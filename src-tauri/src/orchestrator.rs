use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavePlan {
    pub id: String,
    pub project_id: String,
    pub slug: String,
    pub docs_path: Option<String>,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAgent {
    pub id: String,
    pub plan_id: String,
    pub agent_ref: String,
    pub task: String,
    pub wave: i64,
    pub depends_on: Option<String>,
    pub agent_id: Option<String>,
    pub status: String,
    pub guideline_path: Option<String>,
    pub handoff_path: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub retry_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionDoc {
    pub id: String,
    pub plan_id: String,
    pub agent_ref: String,
    pub bug_desc: Option<String>,
    pub root_cause: Option<String>,
    pub fix_required: Option<String>,
    pub test_required: Option<String>,
    pub retry_number: i64,
    pub resolved: bool,
    pub created_at: String,
}

// WAVE ORCHESTRATOR
pub fn create_wave_plan(db: &Connection, project_id: &str, slug: &str) -> Result<WavePlan, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let docs_path = format!("docs/{}/{}", chrono::Local::now().format("%Y-%m-%d"), slug);

    db.execute(
        "INSERT INTO feature_plans (id, project_id, slug, docs_path, status, created_at) VALUES (?1, ?2, ?3, ?4, 'planning', ?5)",
        rusqlite::params![id, project_id, slug, docs_path, now],
    ).map_err(|e| e.to_string())?;

    Ok(WavePlan {
        id, project_id: project_id.to_string(), slug: slug.to_string(),
        docs_path: Some(docs_path), status: "planning".to_string(), created_at: now, completed_at: None,
    })
}

pub fn get_wave_plans(db: &Connection, project_id: Option<&str>) -> Result<Vec<WavePlan>, String> {
    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<WavePlan> {
        Ok(WavePlan {
            id: row.get(0)?,
            project_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            slug: row.get(2)?,
            docs_path: row.get(3)?,
            status: row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "planning".to_string()),
            created_at: row.get(5)?,
            completed_at: row.get(6)?,
        })
    };

    if let Some(pid) = project_id.filter(|s| !s.is_empty()) {
        let mut stmt = db
            .prepare(
                "SELECT id, project_id, slug, docs_path, status, created_at, completed_at \
                 FROM feature_plans WHERE project_id = ?1 ORDER BY created_at DESC, rowid DESC",
            )
            .map_err(|e| e.to_string())?;
        let plans = stmt
            .query_map(rusqlite::params![pid], map_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(plans)
    } else {
        let mut stmt = db
            .prepare(
                "SELECT id, project_id, slug, docs_path, status, created_at, completed_at \
                 FROM feature_plans ORDER BY created_at DESC, rowid DESC",
            )
            .map_err(|e| e.to_string())?;
        let plans = stmt
            .query_map([], map_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(plans)
    }
}

fn resolve_dep_index(
    dep: &str,
    agents: &[PlanAgent],
    ref_to_idx: &std::collections::HashMap<&str, usize>,
) -> Option<usize> {
    if let Some(&idx) = ref_to_idx.get(dep) {
        return Some(idx);
    }
    let suffix = format!("-{dep}");
    let mut matches = agents
        .iter()
        .enumerate()
        .filter(|(_, a)| a.agent_ref.ends_with(&suffix))
        .map(|(idx, _)| idx);
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

/// Validate a wave plan's dependency DAG and group agents into topological
/// execution tiers ordered by `(wave, topological_depth)`.
pub fn validate_wave_plan(agents: &[PlanAgent]) -> Result<Vec<Vec<PlanAgent>>, String> {
    use std::collections::{HashMap, HashSet, VecDeque};

    if agents.is_empty() {
        return Ok(Vec::new());
    }

    let mut ref_to_idx: HashMap<&str, usize> = HashMap::with_capacity(agents.len());
    for (idx, agent) in agents.iter().enumerate() {
        if ref_to_idx.insert(agent.agent_ref.as_str(), idx).is_some() {
            return Err(format!("Duplicate agent_ref '{}' in wave plan", agent.agent_ref));
        }
    }

    let n = agents.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (idx, agent) in agents.iter().enumerate() {
        let Some(raw_deps) = agent.depends_on.as_deref() else {
            continue;
        };
        let mut seen_deps: HashSet<usize> = HashSet::new();
        for token in raw_deps.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let target_idx = resolve_dep_index(token, agents, &ref_to_idx).ok_or_else(|| {
                format!(
                    "Agent '{}' depends on unknown agent '{}'",
                    agent.agent_ref, token
                )
            })?;
            if target_idx == idx {
                return Err(format!(
                    "Self-dependency detected for agent '{}'",
                    agent.agent_ref
                ));
            }
            let target = &agents[target_idx];
            if target.wave > agent.wave {
                return Err(format!(
                    "Agent '{}' in wave {} cannot depend on '{}' in wave {}",
                    agent.agent_ref, agent.wave, target.agent_ref, target.wave
                ));
            }
            if seen_deps.insert(target_idx) {
                adj[target_idx].push(idx);
                in_degree[idx] += 1;
            }
        }
    }

    // Kahn's algorithm for cycle detection + longest-path topological depth
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut depth: Vec<usize> = vec![0; n];
    for (idx, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(idx);
        }
    }

    let mut processed = 0usize;
    while let Some(u) = queue.pop_front() {
        processed += 1;
        for &v in &adj[u] {
            depth[v] = depth[v].max(depth[u] + 1);
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    if processed < n {
        let cycle_refs: Vec<&str> = agents
            .iter()
            .enumerate()
            .filter(|(idx, _)| in_degree[*idx] > 0)
            .map(|(_, a)| a.agent_ref.as_str())
            .collect();
        return Err(format!(
            "Dependency cycle detected involving: {}",
            cycle_refs.join(", ")
        ));
    }

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&idx| (agents[idx].wave, depth[idx], idx));

    let mut tiers: Vec<Vec<PlanAgent>> = Vec::new();
    let mut current_key: Option<(i64, usize)> = None;
    for idx in order {
        let key = (agents[idx].wave, depth[idx]);
        if current_key != Some(key) {
            tiers.push(Vec::new());
            current_key = Some(key);
        }
        tiers.last_mut().unwrap().push(agents[idx].clone());
    }

    Ok(tiers)
}

pub fn add_plan_agent(db: &Connection, plan_id: &str, agent_ref: &str, task: &str, wave: i64, depends_on: Option<&str>, agent_id: Option<&str>) -> Result<PlanAgent, String> {
    let id = Uuid::new_v4().to_string();
    let candidate = PlanAgent {
        id: id.clone(), plan_id: plan_id.to_string(), agent_ref: agent_ref.to_string(), task: task.to_string(),
        wave, depends_on: depends_on.map(String::from), agent_id: agent_id.map(String::from),
        status: "queued".to_string(), guideline_path: None, handoff_path: None,
        started_at: None, completed_at: None, retry_count: 0,
    };

    if depends_on.map(str::trim).is_some_and(|s| !s.is_empty()) {
        let mut all_agents = get_plan_agents(db, plan_id)?;
        all_agents.push(candidate.clone());
        validate_wave_plan(&all_agents)?;
    }

    db.execute(
        "INSERT INTO plan_agents (id, plan_id, agent_ref, task, wave, depends_on, agent_id, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued')",
        rusqlite::params![id, plan_id, agent_ref, task, wave, depends_on, agent_id],
    ).map_err(|e| e.to_string())?;

    Ok(candidate)
}

pub fn get_plan_agents(db: &Connection, plan_id: &str) -> Result<Vec<PlanAgent>, String> {
    let mut stmt = db.prepare(
        "SELECT id, plan_id, agent_ref, task, wave, depends_on, agent_id, status, guideline_path, handoff_path, started_at, completed_at, retry_count FROM plan_agents WHERE plan_id = ?1 ORDER BY wave, agent_ref"
    ).map_err(|e| e.to_string())?;

    let agents = stmt.query_map(rusqlite::params![plan_id], |row| {
        Ok(PlanAgent {
            id: row.get(0)?, plan_id: row.get(1)?, agent_ref: row.get(2)?, task: row.get(3)?,
            wave: row.get(4)?, depends_on: row.get(5)?, agent_id: row.get(6)?, status: row.get(7)?,
            guideline_path: row.get(8)?, handoff_path: row.get(9)?, started_at: row.get(10)?,
            completed_at: row.get(11)?, retry_count: row.get(12)?,
        })
    }).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(agents)
}

pub fn update_plan_agent_status(db: &Connection, agent_id: &str, status: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let completed = if status == "done" || status == "failed" { Some(&now) } else { None };
    let started = if status == "running" { Some(&now) } else { None };

    if let Some(start) = started {
        db.execute("UPDATE plan_agents SET status = ?1, started_at = ?2 WHERE id = ?3", rusqlite::params![status, start, agent_id]).map_err(|e| e.to_string())?;
    } else if let Some(comp) = completed {
        db.execute("UPDATE plan_agents SET status = ?1, completed_at = ?2 WHERE id = ?3", rusqlite::params![status, comp, agent_id]).map_err(|e| e.to_string())?;
    } else {
        db.execute("UPDATE plan_agents SET status = ?1 WHERE id = ?2", rusqlite::params![status, agent_id]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// GUIDELINE GENERATOR
/// The exact section headings a handoff must contain. Single source of truth:
/// the guideline tells the agent to write these, and the monitor validates
/// them. (They previously drifted — the guideline omitted
/// "Interface Contracts Exposed", so a compliant agent could never pass.)
pub const REQUIRED_HANDOFF_SECTIONS: [&str; 6] = [
    "Completed Work",
    "Test Results",
    "Interface Contracts Exposed",
    "Files NOT Modified",
    "Design Decisions",
    "Handoff Instructions",
];

pub fn generate_agent_guideline(agent_ref: &str, task: &str, objective: &str, depends_on: Option<&str>, models: &[&str], files_to_create: &[&str], files_not_touch: &[&str]) -> String {
    let handoff_sections = REQUIRED_HANDOFF_SECTIONS
        .iter()
        .map(|section| format!("  - `## {section}`"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# AGENT {} GUIDELINE\n\n\
## Objective\n{}\n\n\
## Task\n{}\n\n\
## Dependencies\n{}\n\n\
## Model\n{}\n\n\
## Files to Create/Modify\n{}\n\n\
## Files NOT to Touch\n{}\n\n\
## Test Requirements\n- All existing tests must pass\n- Write tests for new code before marking complete\n\n\
## Communication Protocol\n- Use `[ACC:STATUS from={} to=ORCHESTRATOR priority=INFO id=<msgid>] <message>` to report status\n\
- Use `[ACC:BLOCKER from={} to=ORCHESTRATOR priority=HIGH id=<msgid>] <blocker>` if blocked\n\n\
## Handoff\n- Write HANDOFF_{}.md in the repository root when complete\n\
- It must contain these exact section headings, each with a body:\n{}\n\n\
## Budget\n- Track token usage and report at 60%, 80%, 95% thresholds\n\
- Write WIP_CHECKPOINT.md if approaching budget limit\n\n\
---\n*Generated by ACC Wave Orchestrator*",
        agent_ref,
        objective,
        task,
        depends_on.unwrap_or("None \u{2014} first wave agent"),
        models.join(", "),
        files_to_create.iter().map(|s| format!("- `{}`", s)).collect::<Vec<_>>().join("\n"),
        files_not_touch.iter().map(|s| format!("- `{}`", s)).collect::<Vec<_>>().join("\n"),
        agent_ref, agent_ref, agent_ref,
        handoff_sections,
    )
}

// HANDOFF MONITOR
pub fn validate_handoff_schema(content: &str) -> (bool, Vec<String>) {
    let required_sections = REQUIRED_HANDOFF_SECTIONS;
    let mut missing = Vec::new();
    for section in &required_sections {
        if !content.contains(section) {
            missing.push(section.to_string());
        }
    }
    (missing.is_empty(), missing)
}

// CORRECTION LOOP
pub fn create_correction(db: &Connection, plan_id: &str, agent_ref: &str, bug_desc: &str, root_cause: &str, fix_required: &str, test_required: &str, retry_number: i64) -> Result<CorrectionDoc, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT INTO corrections (id, plan_id, agent_ref, bug_desc, root_cause, fix_required, test_required, retry_number, resolved, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        rusqlite::params![id, plan_id, agent_ref, bug_desc, root_cause, fix_required, test_required, retry_number, now],
    ).map_err(|e| e.to_string())?;

    Ok(CorrectionDoc { id, plan_id: plan_id.to_string(), agent_ref: agent_ref.to_string(), bug_desc: Some(bug_desc.to_string()), root_cause: Some(root_cause.to_string()), fix_required: Some(fix_required.to_string()), test_required: Some(test_required.to_string()), retry_number, resolved: false, created_at: now })
}

pub fn get_corrections(db: &Connection, plan_id: &str) -> Result<Vec<CorrectionDoc>, String> {
    let mut stmt = db.prepare(
        "SELECT id, plan_id, agent_ref, bug_desc, root_cause, fix_required, test_required, retry_number, resolved, created_at FROM corrections WHERE plan_id = ?1 ORDER BY retry_number DESC"
    ).map_err(|e| e.to_string())?;
    let corrections = stmt.query_map(rusqlite::params![plan_id], |row| {
        Ok(CorrectionDoc {
            id: row.get(0)?, plan_id: row.get(1)?, agent_ref: row.get(2)?, bug_desc: row.get(3)?,
            root_cause: row.get(4)?, fix_required: row.get(5)?, test_required: row.get(6)?,
            retry_number: row.get(7)?, resolved: row.get::<_, i64>(8)? == 1, created_at: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(corrections)
}

// ============================================================================
// GAP 1: Orchestrator native-vs-external decision logic
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationDecision {
    pub agent_ref: String,
    pub path: String,
    pub mechanism: String,
    pub detection: String,
    pub command: String,
}

pub fn decide_orchestration_path(
    agent_ref: &str,
    task: &str,
    supports_subagents: bool,
    subagent_family: &str,
    wave_command: &str,
    model: &str,
    dir: &str,
) -> OrchestrationDecision {
    if supports_subagents {
        let mechanism = match subagent_family {
            "task-tool" => "task()".to_string(),
            "gemini" => "@agent".to_string(),
            "codex" => "spawn_agent".to_string(),
            "cline" => "native".to_string(),
            "cursor" => "native".to_string(),
            _ => "waveCommand".to_string(),
        };
        let command = match subagent_family {
            "task-tool" => format!("task(subagent_type=\"{}\", description=\"{}\")", agent_ref, task),
            "gemini" => format!("@{} {}", agent_ref, task),
            "codex" => format!("spawn_agent(\"{}\", \"{}\")", agent_ref, task),
            "cline" => format!("cline --task \"{}\" --auto-approve", task),
            _ => wave_command.replace("{prompt}", task).replace("{model}", model).replace("{dir}", dir),
        };
        OrchestrationDecision {
            agent_ref: agent_ref.to_string(),
            path: "native".to_string(),
            mechanism,
            detection: "pty_pattern".to_string(),
            command,
        }
    } else {
        let mut cmd = wave_command.to_string();
        cmd = cmd.replace("{prompt}", task);
        cmd = cmd.replace("{model}", model);
        cmd = cmd.replace("{dir}", dir);
        OrchestrationDecision {
            agent_ref: agent_ref.to_string(),
            path: "external".to_string(),
            mechanism: "waveCommand".to_string(),
            detection: "fs_watch".to_string(),
            command: cmd,
        }
    }
}

// ============================================================================
// GAP 2: Handoff Monitor PTY-based subagent detection
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSpawn {
    pub id: String,
    pub parent_session_id: String,
    pub agent_ref: String,
    pub agent_family: String,
    pub task: String,
    pub detected_at: String,
    pub status: String,
}

pub fn detect_subagent_spawn(
    pty_output: &str,
    detection_pattern: &str,
    parent_session_id: &str,
    agent_ref: &str,
) -> Option<SubagentSpawn> {
    let patterns: Vec<&str> = detection_pattern.split('|').collect();
    let mut best_match: Option<SubagentSpawn> = None;

    for line in pty_output.lines() {
        for pattern in &patterns {
            if line.contains(pattern) {
                let task = line.trim().to_string();
                if task.len() > 1 && task.len() < 500 {
                    best_match = Some(SubagentSpawn {
                        id: uuid::Uuid::new_v4().to_string(),
                        parent_session_id: parent_session_id.to_string(),
                        agent_ref: agent_ref.to_string(),
                        agent_family: detection_pattern.to_string(),
                        task,
                        detected_at: chrono::Utc::now().to_rfc3339(),
                        status: "detected".to_string(),
                    });
                }
            }
        }
    }

    best_match
}

// ============================================================================
// GAP 5: Guideline Generator subagent delegation instructions
// ============================================================================

pub fn generate_orchestrator_guideline(
    agent_ref: &str,
    task: &str,
    objective: &str,
    depends_on: Option<&str>,
    supports_subagents: bool,
    subagent_family: &str,
    sub_agent_refs: &[&str],
    models: &[&str],
    files_to_create: &[&str],
    files_not_touch: &[&str],
) -> String {
    let base = generate_agent_guideline(agent_ref, task, objective, depends_on, models, files_to_create, files_not_touch);

    if !supports_subagents || sub_agent_refs.is_empty() {
        return base;
    }

    let subagent_section = match subagent_family {
        "task-tool" => {
            let mut instructions = format!(
                "## Subagent Delegation (Native — {})\n\n\
                 You are the ORCHESTRATOR for this wave. Delegate tasks to subagents using your native task() mechanism:\n\n\
                 ```\n",
                agent_ref
            );
            for a in sub_agent_refs {
                instructions.push_str(&format!(
                    "task(subagent_type=\"{}\", description=\"<task specific to {}>\")\n", a, a
                ));
            }
            instructions.push_str("```\n\nSubagents assigned:\n");
            for a in sub_agent_refs {
                instructions.push_str(&format!("- **{}** — delegate when ready\n", a));
            }
            instructions.push_str("\nAfter all subagents complete, consolidate their outputs and produce the final HANDOFF.");
            instructions
        }
        "gemini" => {
            let mut instructions = "## Subagent Delegation (Native — Gemini)\n\n\
                Delegate to subagents using @agent_name syntax. Use /agents to list available agents.\n\n".to_string();
            for a in sub_agent_refs {
                instructions.push_str(&format!("- `@{} <task>` — delegate to {}\n", a, a));
            }
            instructions
        }
        "codex" => {
            let mut instructions = "## Subagent Delegation (Native — Codex)\n\n\
                Use spawn_agent to delegate tasks. Each spawned agent runs independently.\n\n".to_string();
            for a in sub_agent_refs {
                instructions.push_str(&format!("- `spawn_agent(\"{}\", \"<task>\")`\n", a));
            }
            instructions
        }
        _ => format!(
            "## Subagent Execution\n\n\
             Subagents ({}) will be spawned externally by ACC as separate sessions.\n\
             Coordinate via HANDOFF files written to the docs/ directory.",
            sub_agent_refs.join(", ")
        ),
    };

    format!("{}\n\n{}", base, subagent_section)
}

pub fn execute_wave(
    db: &Connection,
    _pty: &crate::pty::PtyManager,
    plan_id: &str,
) -> Result<(), String> {
    let agents = get_plan_agents(db, plan_id)?;
    for agent in &agents {
        if agent.status == "queued" {
            update_plan_agent_status(db, &agent.id, "running")?;
        }
    }
    db.execute(
        "UPDATE feature_plans SET status = 'executing' WHERE id = ?1",
        rusqlite::params![plan_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn start_handoff_watcher(
    db: std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>,
    watch_path: std::path::PathBuf,
    project_id: String,
) -> Result<(), String> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        Config::default(),
    ).map_err(|e| format!("watcher create: {e}"))?;
    watcher
        .watch(&watch_path, RecursiveMode::Recursive)
        .map_err(|e| format!("watch: {e}"))?;
    tauri::async_runtime::spawn(async move {
        while let Some(Ok(event)) = rx.recv().await {
            // Iterate changed paths
            for path in &event.paths {
                if !crate::handoff_parser::is_handoff_file(path) {
                    continue;
                }
                let agent_ref = match crate::handoff_parser::agent_ref_from_filename(path) {
                    Some(r) => r,
                    None => continue,
                };
                let envelope = match crate::handoff_parser::parse_handoff_file(path) {
                    Ok(env) => env,
                    Err(_e) => {
                        // Mark agent as failed and create a correction doc.
                        let _ = mark_handoff_failed(&db, &project_id, &agent_ref, &_e).await;
                        continue;
                    }
                };
                let _ = mark_handoff_done(&db, &project_id, &agent_ref, &envelope).await;
            }
        }
    });
    Ok(())
}

async fn mark_handoff_done(
    db: &std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>,
    _project_id: &str,
    agent_ref: &str,
    _envelope: &crate::handoff_parser::HandoffEnvelope,
) -> Result<(), String> {
    let conn = db.lock().await;
    conn.execute(
        "UPDATE plan_agents SET status = 'done', handoff_path = ?1, completed_at = datetime('now') WHERE agent_ref = ?2",
        rusqlite::params!["", agent_ref],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_handoff_failed(
    db: &std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>,
    _project_id: &str,
    agent_ref: &str,
    reason: &str,
) -> Result<(), String> {
    let conn = db.lock().await;
    conn.execute(
        "UPDATE plan_agents SET status = 'failed', completed_at = datetime('now') WHERE agent_ref = ?1",
        rusqlite::params![agent_ref],
    ).map_err(|e| e.to_string())?;
    // Log the failure reason as a correction (for later inspection).
    let _ = conn.execute(
        "INSERT INTO corrections (id, plan_id, agent_ref, bug_desc, root_cause, fix_required, test_required, retry_number, resolved, created_at) VALUES (?1, '', ?2, ?3, '', '', '', 0, 0, datetime('now'))",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_ref, reason],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql"))
            .unwrap();
        for pid in ["proj-1", "proj-a", "proj-b"] {
            conn.execute(
                "INSERT INTO projects (id, path, name, created_at, updated_at) VALUES (?1, ?2, ?1, datetime('now'), datetime('now'))",
                rusqlite::params![pid, format!("/tmp/{pid}")],
            )
            .unwrap();
        }
        conn
    }

    fn make_agent(agent_ref: &str, wave: i64, depends_on: Option<&str>) -> PlanAgent {
        PlanAgent {
            id: format!("id-{agent_ref}"),
            plan_id: "plan-1".to_string(),
            agent_ref: agent_ref.to_string(),
            task: format!("Task {agent_ref}"),
            wave,
            depends_on: depends_on.map(String::from),
            agent_id: None,
            status: "queued".to_string(),
            guideline_path: None,
            handoff_path: None,
            started_at: None,
            completed_at: None,
            retry_count: 0,
        }
    }

    #[test]
    fn test_rejects_cycles_and_invalid_wave_deps() {
        // 1. Self-dependency rejected
        let self_dep = vec![make_agent("A", 1, Some("A"))];
        let err = validate_wave_plan(&self_dep).unwrap_err();
        assert!(err.to_lowercase().contains("self"), "got: {err}");

        // 2. Unknown dependency rejected
        let unknown_dep = vec![make_agent("A", 1, Some("MISSING"))];
        let err = validate_wave_plan(&unknown_dep).unwrap_err();
        assert!(err.to_lowercase().contains("unknown"), "got: {err}");

        // 3. Dependency on higher wave number rejected
        let higher_wave = vec![
            make_agent("A", 1, Some("B")),
            make_agent("B", 2, None),
        ];
        let err = validate_wave_plan(&higher_wave).unwrap_err();
        assert!(
            err.contains("wave 1") && err.contains("wave 2"),
            "expected wave order error, got: {err}"
        );

        // 4. Cycle A -> B -> A rejected
        let cycle = vec![
            make_agent("A", 1, Some("B")),
            make_agent("B", 1, Some("A")),
        ];
        let err = validate_wave_plan(&cycle).unwrap_err();
        assert!(
            err.to_lowercase().contains("cycle"),
            "expected cycle error, got: {err}"
        );

        // 5. Valid multi-wave DAG groups into topological tiers
        let valid = vec![
            make_agent("2-1", 2, Some("1-1, 1-2")),
            make_agent("1-1", 1, None),
            make_agent("1-2", 1, Some("1-1")),
        ];
        let tiers = validate_wave_plan(&valid).expect("valid DAG");
        assert_eq!(tiers.len(), 3);
        assert_eq!(tiers[0].iter().map(|a| a.agent_ref.as_str()).collect::<Vec<_>>(), vec!["1-1"]);
        assert_eq!(tiers[1].iter().map(|a| a.agent_ref.as_str()).collect::<Vec<_>>(), vec!["1-2"]);
        assert_eq!(tiers[2].iter().map(|a| a.agent_ref.as_str()).collect::<Vec<_>>(), vec!["2-1"]);

        // 6. add_plan_agent rejects invalid dependency against DB
        let conn = setup_db();
        let plan = create_wave_plan(&conn, "proj-1", "dag-test").unwrap();
        add_plan_agent(&conn, &plan.id, "2-1", "wave 2", 2, None, None).unwrap();
        let db_err = add_plan_agent(&conn, &plan.id, "1-1", "wave 1", 1, Some("2-1"), None)
            .unwrap_err();
        assert!(db_err.contains("wave 1") && db_err.contains("wave 2"), "got: {db_err}");
    }

    #[test]
    fn test_get_wave_plans() {
        let conn = setup_db();
        let p1 = create_wave_plan(&conn, "proj-a", "first-plan").unwrap();
        let p2 = create_wave_plan(&conn, "proj-b", "second-plan").unwrap();
        let p3 = create_wave_plan(&conn, "proj-a", "third-plan").unwrap();

        let all = get_wave_plans(&conn, None).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, p3.id);
        assert_eq!(all[1].id, p2.id);
        assert_eq!(all[2].id, p1.id);

        let proj_a = get_wave_plans(&conn, Some("proj-a")).unwrap();
        assert_eq!(proj_a.len(), 2);
        assert_eq!(proj_a[0].id, p3.id);
        assert_eq!(proj_a[1].id, p1.id);
    }
}

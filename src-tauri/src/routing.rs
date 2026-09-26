use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSuggestion {
    pub agent_id: String,
    pub model_id: String,
    pub confidence: f64,
    pub reasoning: String,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub model_path: String,
    pub strengths: Option<String>,
    pub agent_id: Option<String>,
    pub alternation_index: Option<i64>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEnvelope {
    pub original_task: String,
    pub completed_by: String,
    pub model_used: String,
    pub output_summary: String,
    pub changed_files: Vec<String>,
    pub diff_preview: String,
    pub handoff_instruction: String,
    pub next_agent: String,
    pub next_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVersionInfo {
    pub agent_id: String,
    pub version: String,
    pub config_snapshot: Option<String>,
    pub checked_at: String,
    pub flags_match: bool,
    pub warning: Option<String>,
}

// TASK ROUTER: Rules-based routing v1 (keyword -> outcome stats -> rank)
pub fn route_task(db: &Connection, task_desc: &str, task_type: &str, project_id: Option<&str>) -> Result<Vec<TaskSuggestion>, String> {
    let keywords: Vec<(&str, &[&str])> = vec![
        ("refactor", &["refactor", "refactoring", "clean", "restructure", "simplify", "reorganize"]),
        ("review", &["review", "audit", "inspect", "check"]),
        ("test", &["test", "testing", "spec", "coverage", "assert"]),
        ("implement", &["implement", "build", "create", "add", "new feature", "develop"]),
        ("debug", &["debug", "fix", "bug", "issue", "error", "crash", "broken"]),
        ("document", &["document", "doc", "readme", "comment", "explain"]),
    ];

    let lower = task_desc.to_lowercase();
    let detected_type = keywords.iter()
        .find(|(_, kw_list)| kw_list.iter().any(|kw| lower.contains(kw)))
        .map(|(t, _)| *t)
        .unwrap_or(task_type);

    // Get outcome stats for this task type
    let query = format!(
        "SELECT agent_id, task_type, total, done, failed, revised, avg_duration_s,
                CASE WHEN total > 0 THEN CAST(done AS REAL) / CAST(total AS REAL) ELSE 0 END as success_rate
         FROM outcome_stats
         WHERE task_type = '{}'
         ORDER BY success_rate DESC, total DESC
         LIMIT 5",
        detected_type
    );

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;
    let suggestions = stmt.query_map([], |row| {
        let success_rate: f64 = row.get(7)?;
        Ok(TaskSuggestion {
            agent_id: row.get(0)?,
            model_id: String::new(),
            confidence: success_rate,
            reasoning: format!("{}% success rate on {} tasks ({} attempts)",
                (success_rate * 100.0).round(), detected_type, row.get::<_, i64>(2)?),
            success_rate,
        })
    }).map_err(|e| e.to_string())?;

    let mut result: Vec<TaskSuggestion> = suggestions.filter_map(|r| r.ok()).collect();

    // Fallback: if no stats, suggest based on task type
    if result.is_empty() {
        let defaults: Vec<(&str, &str)> = vec![
            ("refactor", "claude"),
            ("implement", "claude"),
            ("test", "aider"),
            ("review", "opencode"),
            ("debug", "claude"),
            ("document", "opencode"),
        ];
        if let Some(d) = defaults.iter().find(|(t, _)| *t == detected_type) {
            result.push(TaskSuggestion {
                agent_id: d.1.to_string(),
                model_id: String::new(),
                confidence: 0.5,
                reasoning: "Default suggestion (no outcome history yet)".to_string(),
                success_rate: 0.0,
            });
        }
    }

    let _ = project_id;

    // M2 (spec R20): re-rank by decision-derived confidence when a backend is available.
    let cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let key = std::env::var("OPENROUTER_API_KEY").ok();
    if (key.is_some() || cfg.backend == "local") && result.len() >= 2 {
        let agents: Vec<String> = result.iter().map(|s| s.agent_id.clone()).collect();
        let transport = crate::decision::UreqTransport { timeout_ms: cfg.timeout_ms };
        if let Ok(Some((best, conf))) = crate::decision::choose_agent(
            &cfg,
            &transport,
            key.as_deref(),
            task_desc,
            &agents,
            Some(db),
        ) {
            if let Some(pos) = result.iter().position(|s| s.agent_id == best) {
                let mut chosen = result.remove(pos);
                chosen.confidence = conf;
                chosen.reasoning = format!("decision-layer pick (confidence {conf:.2})");
                result.insert(0, chosen);
            }
        }
    }

    Ok(result)
}

// MODEL ROUTER
pub fn get_models(db: &Connection) -> Result<Vec<ModelEntry>, String> {
    let mut stmt = db.prepare(
        "SELECT id, label, provider, model_path, strengths, agent_id, alternation_index, is_active FROM models ORDER BY label"
    ).map_err(|e| e.to_string())?;

    let models = stmt.query_map([], |row| {
        Ok(ModelEntry {
            id: row.get(0)?,
            label: row.get(1)?,
            provider: row.get(2)?,
            model_path: row.get(3)?,
            strengths: row.get(4)?,
            agent_id: row.get(5)?,
            alternation_index: row.get(6)?,
            is_active: row.get::<_, i64>(7)? == 1,
        })
    }).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    Ok(models)
}

pub fn add_model(db: &Connection, entry: &ModelEntry) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO models (id, label, provider, model_path, strengths, agent_id, alternation_index, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![entry.id, entry.label, entry.provider, entry.model_path, entry.strengths, entry.agent_id, entry.alternation_index, entry.is_active as i64],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn toggle_model(db: &Connection, model_id: &str, active: bool) -> Result<(), String> {
    db.execute(
        "UPDATE models SET is_active = ?1 WHERE id = ?2",
        rusqlite::params![active as i64, model_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// HANDOFF PROTOCOL
pub fn build_handoff_envelope(agent_brief: &HandoffEnvelope) -> String {
    format!(
        "## AGENT HANDOFF\n\n\
         **From agent:** {completed_by} (model: {model_used})\n\
         **To agent:** {next_agent} (suggested model: {next_model})\n\n\
         ### Original Task\n{original_task}\n\n\
         ### What Was Done\n{output_summary}\n\n\
         ### Files Changed\n```\n{changed_files}\n```\n\n\
         ### Key Diff\n```diff\n{diff_preview}\n```\n\n\
         ### Your Instructions\n{handoff_instruction}\n\n\
         ---\n*Handoff generated by Agent Control Center*",
        completed_by = agent_brief.completed_by,
        model_used = agent_brief.model_used,
        next_agent = agent_brief.next_agent,
        next_model = agent_brief.next_model,
        original_task = agent_brief.original_task,
        output_summary = agent_brief.output_summary,
        changed_files = agent_brief.changed_files.join("\n"),
        diff_preview = agent_brief.diff_preview,
        handoff_instruction = agent_brief.handoff_instruction,
    )
}

// VERSION CHECKER
pub fn check_agent_version(agent_id: &str) -> Result<AgentVersionInfo, String> {
    let version = std::process::Command::new(agent_id)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(AgentVersionInfo {
        agent_id: agent_id.to_string(),
        version: version.trim().to_string(),
        config_snapshot: None,
        checked_at: Utc::now().to_rfc3339(),
        flags_match: true,
        warning: None,
    })
}

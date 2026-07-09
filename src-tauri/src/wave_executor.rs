// src-tauri/src/wave_executor.rs
//
// T5: Real Wave Execution (Closes G1)
// Replaces the stub `execute_wave` in orchestrator.rs with a real implementation
// that uses T1-T4 to spawn N agents in N worktrees with guidelines, guards,
// and handoff detection.
//
// Now integrated with:
// - Agent adapters (Feature 1) for CLI abstraction
// - Wave persistence (Feature 2) for crash recovery
// - Agent events (Feature 3) for real-time streaming

use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::agent_adapters::{AdapterRegistry, AgentSession};
use crate::guideline_spawn;
use crate::handoff_parser;
use crate::orchestrator;
use crate::pty::PtyManager;
use crate::wave_persistence::{self, AgentState, WaveState};
use crate::worktree;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecution {
    pub agent_ref: String,
    pub session_id: String,
    pub worktree_path: String,
    pub branch: String,
    pub status: String, // "running" | "done" | "failed" | "killed"
    pub guideline_path: String,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WaveExecutionReport {
    pub plan_id: String,
    pub base_repo: String,
    pub agents: Vec<AgentExecution>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub total_cost_usd: f64,
}

/// Configuration for a wave run.
#[derive(Debug, Clone)]
pub struct WaveRunConfig {
    pub plan_id: String,
    pub base_repo: String,
    pub base_branch: String,
    pub agent_command: String,       // e.g., "mimo" or "claude"
    pub agent_base_args: Vec<String>, // e.g., ["--model", "mimo-v2.5"]
    pub deadline_secs: Option<u64>,
    pub cost_cap_usd: Option<f64>,
}

/// Execute a real wave. Creates worktrees, writes guidelines, spawns agents
/// with guards, and returns a WaveExecutionReport.
pub async fn execute_wave_real(
    db: &Mutex<Connection>,
    pty: &std::sync::Arc<PtyManager>,
    config: WaveRunConfig,
) -> Result<WaveExecutionReport, String> {
    // 1. Read plan agents from DB
    let plan_agents = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        orchestrator::get_plan_agents(&conn, &config.plan_id)?
    };

    // 2. Mark all queued agents as running
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        for agent in &plan_agents {
            if agent.status == "queued" {
                orchestrator::update_plan_agent_status(&conn, &agent.id, "running")?;
            }
        }
        let _ = conn.execute(
            "UPDATE feature_plans SET status = 'executing' WHERE id = ?1",
            rusqlite::params![config.plan_id],
        );
    }

    let mut report = WaveExecutionReport {
        plan_id: config.plan_id.clone(),
        base_repo: config.base_repo.clone(),
        agents: Vec::new(),
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        total_cost_usd: 0.0,
    };

    // 3. For each agent: create worktree + write guideline + spawn
    for agent in &plan_agents {
        let worktree_path = format!(
            ".worktrees/{}-{}",
            config.plan_id, agent.agent_ref
        );
        let branch = format!("agent/{}-{}", config.plan_id, agent.agent_ref);

        // 3a. Create the worktree (from T1)
        worktree::create_worktree(
            &config.base_repo,
            &branch,
            &worktree_path,
            &config.base_branch,
        )?;

        // 3b. Write guideline + build spawn args (from T3)
        let (_guideline_path, spawn_args) = guideline_spawn::prepare_spawn(
            &worktree_path,
            &agent.agent_ref,
            &agent.task,
            &agent.task, // objective = task for now
            agent.depends_on.as_deref(),
            &["mimo-v2.5"],
            &[], // files_to_create: TBD
            &[], // files_not_touch: TBD
            &config.agent_base_args,
        )?;

        // 3c. Spawn the agent with guards (from T2)
        let session_id = pty
            .spawn_process_with_guards(
                agent.agent_ref.clone(),
                worktree_path.clone(),
                config.agent_command.clone(),
                spawn_args,
                HashMap::new(),
                config.deadline_secs,
                config.cost_cap_usd,
            )
            .await?;

        report.agents.push(AgentExecution {
            agent_ref: agent.agent_ref.clone(),
            session_id,
            worktree_path: worktree_path.clone(),
            branch,
            status: "running".to_string(),
            guideline_path: _guideline_path.to_string_lossy().to_string(),
            cost_usd: 0.0,
        });
    }

    // 4. Mark plan as completed (the spawn phase is done; agents run async)
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "UPDATE feature_plans SET status = 'executing' WHERE id = ?1",
            rusqlite::params![config.plan_id],
        );
    }
    report.completed_at = Some(chrono::Utc::now().to_rfc3339());

    Ok(report)
}

/// Finalize a wave: check each agent's handoff file, optionally verify build output,
/// and update agent statuses. If verify_path is provided, runs deployment verification
/// and attaches the report to the wave report.
pub async fn finalize_wave(
    db: &Mutex<Connection>,
    mut report: WaveExecutionReport,
) -> Result<WaveExecutionReport, String> {
    let mut total = 0.0;

    for agent_exec in &mut report.agents {
        let handoff_path = std::path::Path::new(&agent_exec.worktree_path)
            .join(format!("HANDOFF_{}.md", agent_exec.agent_ref));

        match handoff_parser::parse_handoff_file(&handoff_path) {
            Ok(_env) => {
                agent_exec.status = "done".to_string();
                let conn = db.lock().map_err(|e| e.to_string())?;
                let _ = conn.execute(
                    "UPDATE plan_agents SET status = 'done', handoff_path = ?1, completed_at = datetime('now') WHERE agent_ref = ?2",
                    rusqlite::params![handoff_path.to_string_lossy().to_string(), agent_exec.agent_ref],
                );
            }
            Err(_e) => {
                agent_exec.status = "failed".to_string();
                let conn = db.lock().map_err(|e| e.to_string())?;
                let _ = conn.execute(
                    "UPDATE plan_agents SET status = 'failed', completed_at = datetime('now') WHERE agent_ref = ?1",
                    rusqlite::params![agent_exec.agent_ref],
                );
            }
        }
        total += agent_exec.cost_usd;
    }

    report.total_cost_usd = total;
    report.completed_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(report)
}

/// Finalize a wave WITH deployment verification.
/// After agents finish, runs project verification and includes results.
pub async fn finalize_wave_with_verify(
    db: &Mutex<Connection>,
    report: WaveExecutionReport,
    project_path: &str,
) -> Result<serde_json::Value, String> {
    let wave_report = finalize_wave(db, report).await?;
    let verify_report = crate::verification::verify_project(std::path::Path::new(project_path));
    Ok(serde_json::json!({
        "wave": wave_report,
        "verification": verify_report
    }))
}

/// Execute a wave using the adapter registry (Feature 1 integration)
/// This is the modern execution path that uses agent adapters instead of direct CLI calls.
pub async fn execute_wave_with_adapters(
    db: &Mutex<Connection>,
    config: WaveRunConfig,
    registry: &AdapterRegistry,
) -> Result<WaveExecutionReport, String> {
    // 1. Read plan agents from DB
    let plan_agents = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        orchestrator::get_plan_agents(&conn, &config.plan_id)?
    };

    // 2. Check if we can resume from a previous state
    let existing_state = wave_persistence::load_wave_state(&config.plan_id).ok();
    
    if let Some(state) = existing_state {
        // Resume from checkpoint
        return resume_wave_from_state(db, config, registry, state).await;
    }

    // 3. Mark all queued agents as running
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        for agent in &plan_agents {
            if agent.status == "queued" {
                orchestrator::update_plan_agent_status(&conn, &agent.id, "running")?;
            }
        }
        let _ = conn.execute(
            "UPDATE feature_plans SET status = 'executing' WHERE id = ?1",
            rusqlite::params![config.plan_id],
        );
    }

    let mut report = WaveExecutionReport {
        plan_id: config.plan_id.clone(),
        base_repo: config.base_repo.clone(),
        agents: Vec::new(),
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        total_cost_usd: 0.0,
    };

    // 4. Get the adapter for the specified agent command
    let adapter = registry
        .get(&config.agent_command)
        .ok_or_else(|| format!("No adapter found for agent: {}", config.agent_command))?;

    // 5. For each agent: create worktree + write guideline + spawn via adapter
    for agent in &plan_agents {
        let worktree_path = format!(
            ".worktrees/{}-{}",
            config.plan_id, agent.agent_ref
        );
        let branch = format!("agent/{}-{}", config.plan_id, agent.agent_ref);

        // 5a. Create the worktree
        worktree::create_worktree(
            &config.base_repo,
            &branch,
            &worktree_path,
            &config.base_branch,
        )?;

        // 5b. Write guideline
        let (guideline_path, _spawn_args) = guideline_spawn::prepare_spawn(
            &worktree_path,
            &agent.agent_ref,
            &agent.task,
            &agent.task,
            agent.depends_on.as_deref(),
            &["mimo-v2.5"],
            &[],
            &[],
            &config.agent_base_args,
        )?;

        // 5c. Spawn via adapter
        let session = adapter.spawn(&agent.task, &worktree_path)?;

        report.agents.push(AgentExecution {
            agent_ref: agent.agent_ref.clone(),
            session_id: session.id.clone(),
            worktree_path: worktree_path.clone(),
            branch,
            status: "running".to_string(),
            guideline_path: guideline_path.to_string_lossy().to_string(),
            cost_usd: 0.0,
        });
    }

    // 6. Save wave state for crash recovery (Feature 2 integration)
    let wave_state = WaveState {
        wave_id: config.plan_id.clone(),
        agents: report.agents.iter().map(|a| AgentState {
            agent_id: a.agent_ref.clone(),
            worktree: a.worktree_path.clone(),
            status: a.status.clone(),
            session_id: Some(a.session_id.clone()),
            cost_usd: a.cost_usd,
        }).collect(),
        status: "executing".to_string(),
        checkpoint: chrono::Utc::now(),
    };
    wave_persistence::save_wave_state(&wave_state)?;

    // 7. Mark plan as executing
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "UPDATE feature_plans SET status = 'executing' WHERE id = ?1",
            rusqlite::params![config.plan_id],
        );
    }
    report.completed_at = Some(chrono::Utc::now().to_rfc3339());

    Ok(report)
}

/// Resume a wave from a saved state (Feature 2 integration)
async fn resume_wave_from_state(
    db: &Mutex<Connection>,
    config: WaveRunConfig,
    registry: &AdapterRegistry,
    state: WaveState,
) -> Result<WaveExecutionReport, String> {
    let adapter = registry
        .get(&config.agent_command)
        .ok_or_else(|| format!("No adapter found for agent: {}", config.agent_command))?;

    let mut report = WaveExecutionReport {
        plan_id: config.plan_id.clone(),
        base_repo: config.base_repo.clone(),
        agents: Vec::new(),
        started_at: state.checkpoint.to_rfc3339(),
        completed_at: None,
        total_cost_usd: 0.0,
    };

    // Resume agents that were running
    for agent_state in &state.agents {
        if agent_state.status == "running" {
            // Try to resume the agent
            // For now, we just re-spawn it (true resume would require session persistence)
            let session = adapter.spawn("resume", &agent_state.worktree)?;
            
            report.agents.push(AgentExecution {
                agent_ref: agent_state.agent_id.clone(),
                session_id: session.id.clone(),
                worktree_path: agent_state.worktree.clone(),
                branch: format!("agent/{}-{}", config.plan_id, agent_state.agent_id),
                status: "running".to_string(),
                guideline_path: String::new(), // Would need to be persisted
                cost_usd: agent_state.cost_usd,
            });
        }
    }

    report.completed_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main", path])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "config", "user.email", "t@t.com"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "config", "user.name", "T"])
            .output()
            .unwrap();
        std::fs::write(format!("{}/README.md", path), "x").unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "commit", "-m", "init"])
            .output()
            .unwrap();
        dir
    }

    #[test]
    fn test_wave_execution_report_default() {
        let r = WaveExecutionReport::default();
        assert_eq!(r.plan_id, "");
        assert_eq!(r.agents.len(), 0);
        assert_eq!(r.total_cost_usd, 0.0);
        assert!(r.completed_at.is_none());
    }

    #[test]
    fn test_wave_run_config_fields() {
        let cfg = WaveRunConfig {
            plan_id: "plan-1".to_string(),
            base_repo: "/tmp".to_string(),
            base_branch: "main".to_string(),
            agent_command: "mimo".to_string(),
            agent_base_args: vec!["--model".to_string(), "mimo-v2.5".to_string()],
            deadline_secs: Some(300),
            cost_cap_usd: Some(0.50),
        };
        assert_eq!(cfg.plan_id, "plan-1");
        assert_eq!(cfg.deadline_secs, Some(300));
    }

    #[test]
    fn test_agent_execution_initial() {
        let ae = AgentExecution {
            agent_ref: "frontend".to_string(),
            session_id: "sess-1".to_string(),
            worktree_path: ".worktrees/plan-1-frontend".to_string(),
            branch: "agent/plan-1-frontend".to_string(),
            status: "running".to_string(),
            guideline_path: ".worktrees/plan-1-frontend/.acc/GUIDELINE.md".to_string(),
            cost_usd: 0.0,
        };
        assert_eq!(ae.status, "running");
        assert!(ae.guideline_path.contains("GUIDELINE.md"));
    }

    #[test]
    fn test_create_test_repo_works() {
        let repo = create_test_repo();
        let path = repo.path().to_str().unwrap();
        assert!(std::path::Path::new(path).join(".git").exists());
    }
}

// src-tauri/src/wave_supervisor.rs
//
// Agent supervision for wave execution (SPEC-001 §5 DG-5, Wave D Steps 4.1–4.3).
//
// After agents are spawned, the supervisor polls each agent's worktree for a
// valid HANDOFF_<agent_ref>.md:
//   - valid handoff            â†’ agent done
//   - deadline / cost cap      â†’ kill, optional single retry, then correction doc
//   - cancellation requested   â†’ kill and stop
//
// Kill/respawn are abstracted behind `AgentControl` so tests run without PTYs.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::wave_executor::{AgentExecution, WaveExecutionReport};

#[derive(Debug, Clone)]
pub struct SupervisionConfig {
    pub timeout: Duration,
    pub poll_interval: Duration,
    pub max_retries: i64,
    pub cost_cap_usd: Option<f64>,
}

impl Default for SupervisionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            poll_interval: Duration::from_secs(2),
            max_retries: 1,
            cost_cap_usd: None,
        }
    }
}

/// Kill / respawn an agent. Implemented by the adapter runner in production.
pub trait AgentControl: Send + Sync {
    fn kill(&self, agent: &AgentExecution) -> Result<(), String>;
    /// Respawn a failed agent; returns the new session id.
    fn respawn(&self, agent: &AgentExecution) -> Result<String, String>;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SupervisionOutcome {
    pub done: usize,
    pub failed: usize,
    pub retried: usize,
    pub cancelled: bool,
}

enum WaitResult {
    Done,
    Timeout(&'static str),
    Cancelled,
}

fn handoff_path(agent: &AgentExecution) -> std::path::PathBuf {
    Path::new(&agent.worktree_path).join(format!("HANDOFF_{}.md", agent.agent_ref))
}

fn wait_for_handoff(
    agent: &AgentExecution,
    config: &SupervisionConfig,
    cancel_check: &dyn Fn() -> bool,
) -> WaitResult {
    let path = handoff_path(agent);
    let deadline = Instant::now() + config.timeout;

    loop {
        if cancel_check() {
            return WaitResult::Cancelled;
        }

        if let Some(cap) = config.cost_cap_usd {
            if agent.cost_usd >= cap {
                return WaitResult::Timeout("cost cap exceeded");
            }
        }

        if path.exists() && crate::handoff_parser::parse_handoff_file(&path).is_ok() {
            return WaitResult::Done;
        }

        if Instant::now() >= deadline {
            return WaitResult::Timeout("deadline exceeded");
        }

        std::thread::sleep(config.poll_interval);
    }
}

fn record_correction(db: &Mutex<Connection>, plan_id: &str, agent: &AgentExecution, reason: &str) {
    let Ok(conn) = db.lock() else { return };
    let _ = crate::orchestrator::create_correction(
        &conn,
        plan_id,
        &agent.agent_ref,
        reason,
        "Agent did not produce a valid handoff before the deadline",
        "Re-run the task and ensure HANDOFF_<agent>.md is written with all required sections",
        "Validate the handoff with handoff_parser before completion",
        agent.retry_count,
    );
}

pub fn supervise_agents(
    db: &Mutex<Connection>,
    report: &mut WaveExecutionReport,
    config: &SupervisionConfig,
    control: &dyn AgentControl,
    cancel_check: &dyn Fn() -> bool,
) -> Result<SupervisionOutcome, String> {
    let mut outcome = SupervisionOutcome::default();
    let plan_id = report.plan_id.clone();
    let mut cancelled = false;

    for agent in report.agents.iter_mut() {
        if agent.status != "running" {
            continue;
        }

        loop {
            match wait_for_handoff(agent, config, cancel_check) {
                WaitResult::Done => {
                    agent.status = "done".to_string();
                    outcome.done += 1;
                    break;
                }
                WaitResult::Cancelled => {
                    let _ = control.kill(agent);
                    agent.status = "killed".to_string();
                    outcome.failed += 1;
                    outcome.cancelled = true;
                    cancelled = true;
                    break;
                }
                WaitResult::Timeout(reason) => {
                    let _ = control.kill(agent);
                    agent.status = "failed".to_string();

                    let under_cap = config
                        .cost_cap_usd
                        .map(|cap| agent.cost_usd < cap)
                        .unwrap_or(true);
                    let can_retry =
                        under_cap && reason != "cost cap exceeded" && agent.retry_count < config.max_retries;

                    if can_retry {
                        match control.respawn(agent) {
                            Ok(new_session_id) => {
                                agent.retry_count += 1;
                                agent.session_id = new_session_id;
                                agent.status = "running".to_string();
                                outcome.retried += 1;
                                continue;
                            }
                            Err(_) => {
                                outcome.failed += 1;
                                record_correction(db, &plan_id, agent, reason);
                                break;
                            }
                        }
                    }

                    outcome.failed += 1;
                    record_correction(db, &plan_id, agent, reason);
                    break;
                }
            }
        }
    }

    // Cancellation stops the whole wave: kill any sibling agents still running.
    if cancelled {
        for agent in report.agents.iter_mut() {
            if agent.status == "running" {
                let _ = control.kill(agent);
                agent.status = "killed".to_string();
                outcome.failed += 1;
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::TempDir;

    struct StubControl {
        kills: Mutex<Vec<String>>,
        respawns: Mutex<usize>,
        respawn_ok: bool,
    }

    impl StubControl {
        fn new(respawn_ok: bool) -> Self {
            Self {
                kills: Mutex::new(Vec::new()),
                respawns: Mutex::new(0),
                respawn_ok,
            }
        }
    }

    impl AgentControl for StubControl {
        fn kill(&self, agent: &AgentExecution) -> Result<(), String> {
            self.kills.lock().unwrap().push(agent.session_id.clone());
            Ok(())
        }

        fn respawn(&self, _agent: &AgentExecution) -> Result<String, String> {
            *self.respawns.lock().unwrap() += 1;
            if self.respawn_ok {
                Ok("session-new".to_string())
            } else {
                Err("respawn failed".to_string())
            }
        }
    }

    const VALID_HANDOFF: &str = "## Original Task\nDo it\n\n## Completed By\nstub\n\n## Model Used\nstub-1\n\n\
        ## Output Summary\nDone\n\n## Completed Work\nDone\n\n## Test Results\nAll pass\n\n\
        ## Files Changed\n- src/a.rs\n\n## Files NOT Modified\n- package.json\n\n\
        ## Design Decisions\nNone\n\n## Interface Contracts Exposed\nNone\n\n## Handoff Instructions\nNone\n";

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql")).unwrap();
        conn.execute(
            "INSERT INTO projects (id, path, name, created_at, updated_at) VALUES ('p1','/p','P',datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO feature_plans (id, project_id, slug, docs_path, status, created_at) VALUES ('plan-1','p1','s','d','executing',datetime('now'))",
            [],
        )
        .unwrap();
        conn
    }

    fn agent(worktree: &Path) -> AgentExecution {
        AgentExecution {
            agent_ref: "1-1".to_string(),
            session_id: "sess-1".to_string(),
            worktree_path: worktree.to_string_lossy().to_string(),
            branch: "b".to_string(),
            status: "running".to_string(),
            guideline_path: String::new(),
            cost_usd: 0.0,
            retry_count: 0,
        }
    }

    fn report(agent: AgentExecution) -> WaveExecutionReport {
        WaveExecutionReport {
            plan_id: "plan-1".to_string(),
            base_repo: "/p".to_string(),
            agents: vec![agent],
            started_at: "now".to_string(),
            completed_at: None,
            total_cost_usd: 0.0,
        }
    }

    fn fast_config() -> SupervisionConfig {
        SupervisionConfig {
            timeout: Duration::from_millis(60),
            poll_interval: Duration::from_millis(5),
            max_retries: 1,
            cost_cap_usd: None,
        }
    }

    fn no_cancel() -> bool {
        false
    }

    #[test]
    fn test_supervisor_marks_done_when_handoff_valid() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("HANDOFF_1-1.md"), VALID_HANDOFF).unwrap();

        let db = Mutex::new(setup_db());
        let control = StubControl::new(true);
        let mut wave = report(agent(dir.path()));

        let outcome =
            supervise_agents(&db, &mut wave, &fast_config(), &control, &no_cancel).unwrap();

        assert_eq!(outcome.done, 1);
        assert_eq!(wave.agents[0].status, "done");
        assert!(control.kills.lock().unwrap().is_empty());
    }

    #[test]
    fn test_supervisor_ignores_incomplete_handoff_then_fails() {
        let dir = TempDir::new().unwrap();
        // Missing required sections â†’ not a valid handoff.
        std::fs::write(dir.path().join("HANDOFF_1-1.md"), "# HANDOFF\nJust some text\n").unwrap();

        let db = Mutex::new(setup_db());
        let control = StubControl::new(false);
        let mut wave = report(agent(dir.path()));

        let outcome =
            supervise_agents(&db, &mut wave, &fast_config(), &control, &no_cancel).unwrap();

        assert_eq!(outcome.failed, 1);
        assert_eq!(wave.agents[0].status, "failed");
        assert_eq!(control.kills.lock().unwrap().len(), 1);

        let corrections: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM corrections WHERE plan_id = 'plan-1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(corrections, 1, "correction doc must be recorded");
    }

    #[test]
    fn test_failed_agent_gets_one_retry() {
        let dir = TempDir::new().unwrap(); // no handoff ever appears

        let db = Mutex::new(setup_db());
        let control = StubControl::new(true);
        let mut wave = report(agent(dir.path()));

        let outcome =
            supervise_agents(&db, &mut wave, &fast_config(), &control, &no_cancel).unwrap();

        assert_eq!(outcome.retried, 1, "one retry allowed");
        assert_eq!(outcome.failed, 1, "still failed after retry");
        assert_eq!(wave.agents[0].retry_count, 1);
        assert_eq!(*control.respawns.lock().unwrap(), 1);
        assert_eq!(control.kills.lock().unwrap().len(), 2, "killed before each attempt");
    }

    #[test]
    fn test_retry_respects_cost_cap() {
        let dir = TempDir::new().unwrap();

        let db = Mutex::new(setup_db());
        let control = StubControl::new(true);
        let mut wave_agent = agent(dir.path());
        wave_agent.cost_usd = 1.0;
        let mut wave = report(wave_agent);

        let config = SupervisionConfig {
            cost_cap_usd: Some(0.5),
            ..fast_config()
        };

        let outcome = supervise_agents(&db, &mut wave, &config, &control, &no_cancel).unwrap();

        assert_eq!(outcome.retried, 0, "over-cap agents must not be retried");
        assert_eq!(outcome.failed, 1);
        assert_eq!(*control.respawns.lock().unwrap(), 0);
    }

    #[test]
    fn test_cancel_kills_running_agent() {
        let dir = TempDir::new().unwrap();

        let db = Mutex::new(setup_db());
        let control = StubControl::new(true);
        let mut wave = report(agent(dir.path()));

        let cancelled = AtomicBool::new(true);
        let cancel_check = || cancelled.load(Ordering::SeqCst);

        let outcome = supervise_agents(&db, &mut wave, &fast_config(), &control, &cancel_check).unwrap();

        assert!(outcome.cancelled);
        assert_eq!(wave.agents[0].status, "killed");
        assert_eq!(
            *control.kills.lock().unwrap(),
            vec!["sess-1".to_string()]
        );
    }
}

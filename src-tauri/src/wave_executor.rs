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
use std::path::Path;

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
    #[serde(default)]
    pub retry_count: i64,
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
            retry_count: 0,
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
        // Absolute path *inside the base repo*: a bare `.worktrees/...` is
        // resolved by git relative to the repo (`-C`) but by the guideline
        // writer / agent CWD relative to the process, so the agent would run in
        // a stray empty directory with only `.acc/GUIDELINE.md` in it.
        let worktree_path = Path::new(&config.base_repo)
            .join(".worktrees")
            .join(format!("{}-{}", config.plan_id, agent.agent_ref))
            .to_string_lossy()
            .to_string();
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

        // Prompt with a short, constant pointer to the guideline file instead of
        // inlining its content. The guideline carries spec-derived text, and on
        // Windows agents are launched through `cmd /C`, where arbitrary text in
        // an argument is a command-injection vector (and can exceed the
        // command-line length limit for large specs). Reading `.acc/GUIDELINE.md`
        // is the phrasing proven to work in the dogfood probe.
        let task_prompt = if guideline_path.exists() {
            "Implement the task described in .acc/GUIDELINE.md in this worktree, \
             then write the HANDOFF file it names in the repository root."
                .to_string()
        } else {
            agent.task.clone()
        };
        let session = adapter.spawn(&task_prompt, &worktree_path)?;

        report.agents.push(AgentExecution {
            agent_ref: agent.agent_ref.clone(),
            session_id: session.id.clone(),
            worktree_path: worktree_path.clone(),
            branch,
            status: "running".to_string(),
            guideline_path: guideline_path.to_string_lossy().to_string(),
            cost_usd: 0.0,
            retry_count: 0,
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
                retry_count: 0,
            });
        }
    }

    report.completed_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(report)
}

// ---------------------------------------------------------------------------
// Delivery: commit agent work and merge it back (work not merged is not shipped)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MergeRecord {
    pub agent_ref: String,
    pub branch: String,
    pub commit: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MergeReport {
    /// Branches committed and merged into the base branch.
    pub merged: Vec<MergeRecord>,
    /// Branches that could not be merged (conflict, missing worktree, ...).
    pub conflicts: Vec<MergeRecord>,
    /// Agents that produced nothing to deliver (not done / no changes).
    pub skipped: Vec<MergeRecord>,
}

fn git(args: &[&str]) -> Result<(bool, String), String> {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("git {:?} failed to start: {e}", &args[..1.min(args.len())]))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok((output.status.success(), text.trim().to_string()))
}

fn git_or_err(args: &[&str]) -> Result<String, String> {
    let (ok, text) = git(args)?;
    if ok {
        Ok(text)
    } else {
        Err(format!("git {} failed: {text}", args.join(" ")))
    }
}

/// Commit each completed agent's worktree and merge its branch into the base
/// branch, then remove the worktree.
///
/// This is what makes the run a *delivery*: before this, agents wrote only into
/// throwaway worktrees and the base project never received their work.
pub fn merge_completed_agents(
    report: &WaveExecutionReport,
    base_repo: &str,
    base_branch: &str,
) -> MergeReport {
    let mut result = MergeReport::default();

    // Merge into the intended branch regardless of what is currently checked out.
    let (ok, head) = git(&["-C", base_repo, "rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or((false, String::new()));
    if !ok {
        result.conflicts.push(MergeRecord {
            agent_ref: "-".to_string(),
            branch: base_branch.to_string(),
            commit: None,
            detail: format!("cannot read the current branch of {base_repo}"),
        });
        return result;
    }
    if head != base_branch {
        if let Err(error) = git_or_err(&["-C", base_repo, "checkout", base_branch]) {
            result.conflicts.push(MergeRecord {
                agent_ref: "-".to_string(),
                branch: base_branch.to_string(),
                commit: None,
                detail: format!("cannot switch to {base_branch}: {error}"),
            });
            return result;
        }
    }

    for agent in &report.agents {
        let record = |commit: Option<String>, detail: String| MergeRecord {
            agent_ref: agent.agent_ref.clone(),
            branch: agent.branch.clone(),
            commit,
            detail,
        };

        if agent.status != "done" {
            result
                .skipped
                .push(record(None, format!("agent status is {}", agent.status)));
            continue;
        }
        if !Path::new(&agent.worktree_path).exists() {
            result.skipped.push(record(None, "worktree is gone".to_string()));
            continue;
        }

        // 1. Commit whatever the agent left behind (identity supplied inline so
        //    the user's repo does not need git identity configured).
        //
        //    `.acc/` (GUIDELINE.md, TASK.md) is pipeline bookkeeping, not
        //    deliverable: every agent's copy differs, so committing it made every
        //    second agent conflict on add/add. Excluded via pathspec rather than
        //    touching the user's git config.
        const EXCLUDE_BOOKKEEPING: [&str; 2] = [".", ":(exclude).acc"];
        let (_, status) = match git(&[
            "-C",
            &agent.worktree_path,
            "status",
            "--porcelain",
            "--",
            EXCLUDE_BOOKKEEPING[0],
            EXCLUDE_BOOKKEEPING[1],
        ]) {
            Ok(value) => value,
            Err(error) => {
                result.conflicts.push(record(None, error));
                continue;
            }
        };
        let commit = if status.is_empty() {
            let (ok, sha) = git(&["-C", &agent.worktree_path, "rev-parse", "HEAD"]).unwrap_or((false, String::new()));
            if !ok {
                result.skipped.push(record(None, "no changes and no commit".to_string()));
                continue;
            }
            Some(sha)
        } else {
            let message = format!("agent {}: deliver work from {}", agent.agent_ref, agent.branch);
            let staged = git_or_err(&[
                "-C",
                &agent.worktree_path,
                "add",
                "-A",
                "--",
                EXCLUDE_BOOKKEEPING[0],
                EXCLUDE_BOOKKEEPING[1],
            ])
            .and_then(|_| {
                git_or_err(&[
                    "-C",
                    &agent.worktree_path,
                    "-c",
                    "user.name=SourceForge Agent",
                    "-c",
                    "user.email=agent@sourceforge.local",
                    "commit",
                    "-m",
                    &message,
                ])
            });
            match staged {
                Ok(_) => {
                    let (ok, sha) = git(&["-C", &agent.worktree_path, "rev-parse", "HEAD"]).unwrap_or((false, String::new()));
                    if !ok {
                        result.conflicts.push(record(None, "commit created but HEAD unreadable".to_string()));
                        continue;
                    }
                    Some(sha)
                }
                Err(error) => {
                    result.conflicts.push(record(None, format!("commit failed: {error}")));
                    continue;
                }
            }
        };

        // 2. Merge the agent branch into the base branch.
        let short = commit.as_deref().unwrap_or("work").chars().take(8).collect::<String>();
        let merge_message = format!("merge agent {} ({})", agent.agent_ref, short);
        if let Err(error) = git_or_err(&["-C", base_repo, "merge", "--no-ff", "-m", &merge_message, &agent.branch]) {
            // Leave the repo clean for the operator to resolve by hand.
            let _ = git(&["-C", base_repo, "merge", "--abort"]);
            result.conflicts.push(record(commit, format!("merge failed: {error}")));
            continue;
        }

        // 3. Only now is the worktree disposable.
        let _ = git(&["-C", base_repo, "worktree", "remove", "--force", &agent.worktree_path]);
        result.merged.push(record(commit, format!("merged into {base_branch}")));
    }

    result
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
            retry_count: 0,
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

    // ------------------------------------------------------------------
    // Delivery: merging agent work back into the base branch
    // ------------------------------------------------------------------

    fn git(args: &[&str]) {
        let out = std::process::Command::new("git").args(args).output().unwrap();
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Create a worktree on its own branch, write `file` with `content`, and
    /// return the AgentExecution the merge step would receive.
    fn agent_worktree(
        repo: &TempDir,
        agent_ref: &str,
        file: &str,
        content: &str,
    ) -> AgentExecution {
        let repo_path = repo.path().to_str().unwrap();
        let worktree = repo.path().join(format!("wt-{agent_ref}"));
        let branch = format!("agent/{agent_ref}");
        let wt = worktree.to_str().unwrap();
        git(&["-C", repo_path, "worktree", "add", "-b", &branch, wt, "main"]);
        let target = worktree.join(file);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(target, content).unwrap();
        // Per-agent pipeline bookkeeping: must never be delivered (every agent's
        // copy differs, which used to make every second merge conflict add/add).
        let acc = worktree.join(".acc");
        std::fs::create_dir_all(&acc).unwrap();
        std::fs::write(acc.join("GUIDELINE.md"), format!("# AGENT {agent_ref} GUIDELINE\n")).unwrap();
        AgentExecution {
            agent_ref: agent_ref.to_string(),
            session_id: format!("session-{agent_ref}"),
            worktree_path: wt.to_string(),
            branch,
            status: "done".to_string(),
            guideline_path: String::new(),
            cost_usd: 0.0,
            retry_count: 0,
        }
    }

    fn wave_report(agents: Vec<AgentExecution>) -> WaveExecutionReport {
        WaveExecutionReport {
            plan_id: "plan-merge".to_string(),
            base_repo: String::new(),
            agents,
            started_at: "now".to_string(),
            completed_at: None,
            total_cost_usd: 0.0,
        }
    }

    #[test]
    fn test_merge_delivers_agent_work_into_the_base_branch() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let report = wave_report(vec![
            agent_worktree(&repo, "1-1", "src/a.txt", "from a\n"),
            agent_worktree(&repo, "1-2", "src/b.txt", "from b\n"),
        ]);

        let merges = merge_completed_agents(&report, repo_path, "main");

        assert_eq!(merges.merged.len(), 2, "both agents delivered: {:?}", merges);
        assert!(merges.conflicts.is_empty(), "no conflicts: {:?}", merges.conflicts);
        // The work is now in the base project, not just in a worktree.
        assert!(repo.path().join("src/a.txt").exists());
        assert!(repo.path().join("src/b.txt").exists());
        // Worktrees are disposable once their work is merged.
        for agent in &report.agents {
            assert!(!Path::new(&agent.worktree_path).exists(), "worktree removed");
        }
        // Every recorded merge names its commit.
        assert!(merges.merged.iter().all(|m| m.commit.is_some()));
    }

    #[test]
    fn test_merge_reports_conflicts_without_leaving_a_dirty_repo() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        // Same file, different content → the second merge must conflict.
        let report = wave_report(vec![
            agent_worktree(&repo, "1-1", "conflict.txt", "a\n"),
            agent_worktree(&repo, "1-2", "conflict.txt", "b\n"),
        ]);

        let merges = merge_completed_agents(&report, repo_path, "main");

        assert_eq!(merges.merged.len(), 1);
        assert_eq!(merges.conflicts.len(), 1, "conflicts: {:?}", merges.conflicts);
        assert_eq!(merges.conflicts[0].agent_ref, "1-2");
        assert!(merges.conflicts[0].detail.contains("merge failed"));
        // The repo is left clean for the operator (merge aborted, not half-done).
        // Untracked entries are the still-present worktrees of unmerged agents.
        let out = std::process::Command::new("git")
            .args(["-C", repo_path, "status", "--porcelain", "--untracked-files=no"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&out.stdout).trim().is_empty(),
            "no half-merged changes: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let merge_head = std::process::Command::new("git")
            .args(["-C", repo_path, "rev-parse", "-q", "--verify", "MERGE_HEAD"])
            .output()
            .unwrap();
        assert!(!merge_head.status.success(), "no merge left in progress");
    }

    #[test]
    fn test_merge_skips_agents_that_did_not_complete() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let mut failed = agent_worktree(&repo, "1-1", "x.txt", "x\n");
        failed.status = "failed".to_string();
        let report = wave_report(vec![failed]);

        let merges = merge_completed_agents(&report, repo_path, "main");

        assert!(merges.merged.is_empty());
        assert!(merges.conflicts.is_empty());
        assert_eq!(merges.skipped.len(), 1);
        assert!(merges.skipped[0].detail.contains("failed"));
        // Its worktree is left in place for inspection.
        assert!(Path::new(&report.agents[0].worktree_path).exists());
    }
}

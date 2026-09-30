// src-tauri/src/pipeline.rs
//
// Supervised build pipeline (SPEC-001 §5 DG-1).
//
//   parse_spec → resolve_stack → provision → scaffold → seed_plan
//   → execute_waves → finalize_and_verify → deploy → report
//
// The pipeline is synchronous by design: it runs inside
// `tauri::async_runtime::spawn_blocking` from `pipeline_commands.rs`, and the
// two async dependencies (wave execution, compounder LLM call) are bridged
// with `block_on` on a blocking thread. All heavy seams are injectable so the
// E2E test runs hermetically with a mock wave runner, mock deployer, and a
// static LLM.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::deployer::{DeployOutcome, Deployer};
use crate::pipeline_store::{self, StageLogEntry};
use crate::stack_registry::{self, CliStatus};
use crate::wave_executor::{AgentExecution, WaveExecutionReport, WaveRunConfig};

pub const EVENT_NAME: &str = "build-app-progress";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildStage {
    ParseSpec,
    ResolveStack,
    Provision,
    Scaffold,
    SeedPlan,
    ExecuteWaves,
    FinalizeAndVerify,
    Deploy,
    Report,
}

impl BuildStage {
    pub fn order() -> [BuildStage; 9] {
        [
            BuildStage::ParseSpec,
            BuildStage::ResolveStack,
            BuildStage::Provision,
            BuildStage::Scaffold,
            BuildStage::SeedPlan,
            BuildStage::ExecuteWaves,
            BuildStage::FinalizeAndVerify,
            BuildStage::Deploy,
            BuildStage::Report,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BuildStage::ParseSpec => "parse_spec",
            BuildStage::ResolveStack => "resolve_stack",
            BuildStage::Provision => "provision",
            BuildStage::Scaffold => "scaffold",
            BuildStage::SeedPlan => "seed_plan",
            BuildStage::ExecuteWaves => "execute_waves",
            BuildStage::FinalizeAndVerify => "finalize_and_verify",
            BuildStage::Deploy => "deploy",
            BuildStage::Report => "report",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOptions {
    pub run_id: String,
    pub project_id: Option<String>,
    pub spec_path: String,
    pub project_path: String,
    pub stack_id: Option<String>,
    pub agent_command: String,
    pub base_branch: String,
    pub allow_deploy_on_failed_verification: bool,
    pub generate_dockerfile: bool,
    pub agent_timeout_secs: u64,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            run_id: String::new(),
            project_id: None,
            spec_path: String::new(),
            project_path: String::new(),
            stack_id: None,
            agent_command: "opencode".to_string(),
            base_branch: "main".to_string(),
            allow_deploy_on_failed_verification: false,
            generate_dockerfile: true,
            agent_timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRecord {
    pub name: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub run_id: String,
    pub status: String,
    pub stack_id: Option<String>,
    pub stages: Vec<StageRecord>,
    pub plan_id: Option<String>,
    pub wave: Option<WaveExecutionReport>,
    pub verification: Option<serde_json::Value>,
    pub deploy: Option<DeployOutcome>,
    pub compounder_items: usize,
    pub artifacts: serde_json::Value,
    pub error: Option<String>,
}

impl Default for BuildReport {
    fn default() -> Self {
        Self {
            run_id: String::new(),
            status: "pending".to_string(),
            stack_id: None,
            stages: Vec::new(),
            plan_id: None,
            wave: None,
            verification: None,
            deploy: None,
            compounder_items: 0,
            artifacts: serde_json::json!({}),
            error: None,
        }
    }
}

/// Emits pipeline progress to the frontend (or nowhere, in tests).
pub trait EventSink: Send + Sync {
    fn emit(&self, stage: &str, status: &str, message: &str);
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _stage: &str, _status: &str, _message: &str) {}
}

/// Executes a wave plan and returns the wave report. Injectable for tests.
pub trait WaveRunner: Send + Sync {
    fn run(
        &self,
        db: &Mutex<Connection>,
        plan_id: &str,
        base_repo: &str,
    ) -> Result<WaveExecutionReport, String>;

    /// Kill/respawn control for the supervisor. `None` skips supervision
    /// (used by hermetic test runners that write handoffs immediately).
    fn control(&self) -> Option<&dyn crate::wave_supervisor::AgentControl> {
        None
    }
}

/// Production wave runner: adapter-based execution (mock/opencode adapters).
pub struct AdapterWaveRunner {
    pub registry: crate::agent_adapters::AdapterRegistry,
    pub agent_command: String,
    pub base_branch: String,
    pub deadline_secs: Option<u64>,
    pub cost_cap_usd: Option<f64>,
}

impl WaveRunner for AdapterWaveRunner {
    fn run(
        &self,
        db: &Mutex<Connection>,
        plan_id: &str,
        base_repo: &str,
    ) -> Result<WaveExecutionReport, String> {
        let config = WaveRunConfig {
            plan_id: plan_id.to_string(),
            base_repo: base_repo.to_string(),
            base_branch: self.base_branch.clone(),
            agent_command: self.agent_command.clone(),
            agent_base_args: vec![],
            deadline_secs: self.deadline_secs,
            cost_cap_usd: self.cost_cap_usd,
        };
        tauri::async_runtime::block_on(crate::wave_executor::execute_wave_with_adapters(
            db,
            config,
            &self.registry,
        ))
    }

    fn control(&self) -> Option<&dyn crate::wave_supervisor::AgentControl> {
        Some(self)
    }
}

impl crate::wave_supervisor::AgentControl for AdapterWaveRunner {
    fn kill(&self, agent: &AgentExecution) -> Result<(), String> {
        let adapter = self
            .registry
            .get(&self.agent_command)
            .ok_or_else(|| format!("No adapter found for agent: {}", self.agent_command))?;
        let session = crate::agent_adapters::AgentSession {
            id: agent.session_id.clone(),
            agent_id: self.agent_command.clone(),
            worktree: agent.worktree_path.clone(),
            started_at: chrono::Utc::now(),
        };
        adapter.kill(&session)
    }

    fn is_running(&self, agent: &AgentExecution) -> bool {
        match self.registry.get(&self.agent_command) {
            Some(adapter) => adapter.is_running(&crate::agent_adapters::AgentSession {
                id: agent.session_id.clone(),
                agent_id: self.agent_command.clone(),
                worktree: agent.worktree_path.clone(),
                started_at: chrono::Utc::now(),
            }),
            None => true,
        }
    }

    fn cost_usd(&self, agent: &AgentExecution) -> Option<f64> {
        let adapter = self.registry.get(&self.agent_command)?;
        adapter.session_cost(&crate::agent_adapters::AgentSession {
            id: agent.session_id.clone(),
            agent_id: self.agent_command.clone(),
            worktree: agent.worktree_path.clone(),
            started_at: chrono::Utc::now(),
        })
    }

    fn respawn(&self, agent: &AgentExecution) -> Result<String, String> {
        let adapter = self
            .registry
            .get(&self.agent_command)
            .ok_or_else(|| format!("No adapter found for agent: {}", self.agent_command))?;
        // Re-send the real task, not the agent reference: the per-agent
        // guideline written at spawn time holds it. Fall back to the ref only
        // if the guideline is unreadable.
        let guideline = std::path::Path::new(&agent.worktree_path)
            .join(".acc")
            .join("GUIDELINE.md");
        let task = std::fs::read_to_string(&guideline)
            .ok()
            .filter(|content| !content.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "Continue task {}: follow .acc/GUIDELINE.md and write HANDOFF_{}.md",
                    agent.agent_ref, agent.agent_ref
                )
            });
        Ok(adapter.spawn(&task, &agent.worktree_path)?.id)
    }
}

pub struct PipelineAdapters<'a> {
    pub deployer: &'a dyn Deployer,
    pub wave_runner: &'a dyn WaveRunner,
    pub event_sink: &'a dyn EventSink,
    pub llm: &'a crate::compounder_llm::LlmProvider,
    /// None = detect CLIs on this machine.
    pub cli_status: Option<HashMap<String, CliStatus>>,
    pub run_compounder: bool,
    /// Install project dependencies before verification and treat an
    /// un-runnable build/test as a verification failure. Production `true`;
    /// hermetic tests `false` so they stay offline and deterministic.
    pub install_deps: bool,
    /// Commit each agent's work and merge it into the base branch (delivery).
    /// Production `true`; hermetic tests use non-git stand-in worktrees.
    pub deliver: bool,
}

enum StageResult {
    Done(String),
    Skipped(String),
    AwaitingUser(String),
}

#[derive(Default)]
struct PipelineState {
    tasks: Vec<crate::spec_parser::SpecTask>,
    run_id: String,
    project_path: String,
    project_id: String,
    stack_id: Option<String>,
    plan_id: Option<String>,
    wave_report: Option<WaveExecutionReport>,
    verification: Option<crate::verification::VerificationReport>,
    deploy: Option<DeployOutcome>,
    compounder_items: usize,
    merges: Option<crate::wave_executor::MergeReport>,
}

fn lock<'a>(db: &'a Mutex<Connection>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    db.lock().map_err(|e| e.to_string())
}

pub fn run_pipeline(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
) -> Result<BuildReport, String> {
    let mut report = BuildReport {
        run_id: opts.run_id.clone(),
        ..Default::default()
    };
    let mut state = PipelineState {
        run_id: opts.run_id.clone(),
        project_path: opts.project_path.clone(),
        ..Default::default()
    };

    // Resume support: hydrate state from the persisted report (if any), then
    // stages re-run idempotently (cheap stages always, expensive ones guard on
    // already-populated state).
    {
        let conn = lock(db)?;
        let run = pipeline_store::get_run(&conn, &opts.run_id)?;
        if let Some(value) = run.report {
            if let Ok(previous) = serde_json::from_value::<BuildReport>(value) {
                state.stack_id = previous.stack_id;
                state.plan_id = previous.plan_id;
                state.wave_report = previous.wave;
                state.verification = previous
                    .verification
                    .as_ref()
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
                state.deploy = previous.deploy;
                state.compounder_items = previous.compounder_items;
            }
        }
        // Resolve the project before any stage runs: `provision` checks Supabase
        // config and runs *before* `seed_plan`, which used to be the only
        // resolver (so the default Supabase stack could never provision).
        state.project_id = resolve_project_id(&conn, opts)?;
    }

    for stage in BuildStage::order() {
        let name = stage.as_str();

        // Cancellation is checked between stages.
        {
            let conn = lock(db)?;
            if pipeline_store::is_cancelled(&conn, &opts.run_id)? {
                report.status = pipeline_store::STATUS_CANCELLED.to_string();
                pipeline_store::set_report(&conn, &opts.run_id, &serde_json::to_value(&report).unwrap_or_default())?;
                return Ok(report);
            }
        }

        let started_at = Utc::now().to_rfc3339();
        {
            let conn = lock(db)?;
            pipeline_store::update_status(&conn, &opts.run_id, pipeline_store::STATUS_RUNNING, Some(name))?;
            pipeline_store::append_stage_log(&conn, &opts.run_id, &StageLogEntry::new(name, "started", ""))?;
        }
        adapters.event_sink.emit(name, "started", "");

        let outcome = run_stage(stage, db, opts, adapters, &mut state);
        let ended_at = Utc::now().to_rfc3339();

        match outcome {
            Ok(StageResult::Done(message)) => {
                {
                    let conn = lock(db)?;
                    pipeline_store::append_stage_log(&conn, &opts.run_id, &StageLogEntry::new(name, "done", &message))?;
                }
                adapters.event_sink.emit(name, "done", &message);
                report.stages.push(StageRecord {
                    name: name.to_string(),
                    status: "done".to_string(),
                    started_at,
                    ended_at,
                    message,
                });
                // Persist after every stage so a crash mid-run still carries
                // plan_id/stack/verification forward on resume (no duplicate plans).
                finalize_report(db, opts, &mut report, &state)?;
            }
            Ok(StageResult::Skipped(message)) => {
                {
                    let conn = lock(db)?;
                    pipeline_store::append_stage_log(&conn, &opts.run_id, &StageLogEntry::new(name, "skipped", &message))?;
                }
                adapters.event_sink.emit(name, "skipped", &message);
                report.stages.push(StageRecord {
                    name: name.to_string(),
                    status: "skipped".to_string(),
                    started_at,
                    ended_at,
                    message,
                });
            }
            Ok(StageResult::AwaitingUser(message)) => {
                {
                    let conn = lock(db)?;
                    pipeline_store::append_stage_log(&conn, &opts.run_id, &StageLogEntry::new(name, "awaiting_user", &message))?;
                    pipeline_store::update_status(&conn, &opts.run_id, pipeline_store::STATUS_AWAITING_USER, Some(name))?;
                }
                adapters.event_sink.emit(name, "awaiting_user", &message);
                report.status = pipeline_store::STATUS_AWAITING_USER.to_string();
                report.stages.push(StageRecord {
                    name: name.to_string(),
                    status: "awaiting_user".to_string(),
                    started_at,
                    ended_at,
                    message,
                });
                finalize_report(db, opts, &mut report, &state)?;
                return Ok(report);
            }
            Err(error) => {
                {
                    let conn = lock(db)?;
                    pipeline_store::append_stage_log(&conn, &opts.run_id, &StageLogEntry::new(name, "failed", &error))?;
                    pipeline_store::set_error(&conn, &opts.run_id, &error)?;
                    pipeline_store::update_status(&conn, &opts.run_id, pipeline_store::STATUS_FAILED, Some(name))?;
                }
                adapters.event_sink.emit(name, "failed", &error);
                report.status = pipeline_store::STATUS_FAILED.to_string();
                report.error = Some(error.clone());
                report.stages.push(StageRecord {
                    name: name.to_string(),
                    status: "failed".to_string(),
                    started_at,
                    ended_at,
                    message: error,
                });
                finalize_report(db, opts, &mut report, &state)?;
                return Ok(report);
            }
        }
    }

    // Work that could not be merged was not delivered: pause for the operator
    // instead of verifying/deploying a project that is missing agent output.
    if let Some(merges) = &state.merges {
        if !merges.conflicts.is_empty() {
            let detail = merges
                .conflicts
                .iter()
                .map(|record| format!("{}: {}", record.agent_ref, record.detail))
                .collect::<Vec<_>>()
                .join("; ");
            let message = format!(
                "{} agent branch(es) could not be merged: {detail}",
                merges.conflicts.len()
            );
            report.status = pipeline_store::STATUS_AWAITING_USER.to_string();
            report.error = Some(message.clone());
            finalize_report(db, opts, &mut report, &state)?;
            {
                let conn = lock(db)?;
                pipeline_store::set_error(&conn, &opts.run_id, &message)?;
                pipeline_store::update_status(
                    &conn,
                    &opts.run_id,
                    pipeline_store::STATUS_AWAITING_USER,
                    Some("report"),
                )?;
            }
            adapters.event_sink.emit("report", "merge_conflict", &message);
            return Ok(report);
        }
    }

    // A run whose agents never produced a valid handoff is NOT a success, even
    // when the project happens to satisfy verification (e.g. it already passed
    // before the agents ran). Pause instead of overclaiming: the operator can
    // resume (retry) or cancel.
    if let Some(wave) = &state.wave_report {
        let incomplete = incomplete_agents(wave);
        if !incomplete.is_empty() {
            let detail = incomplete
                .iter()
                .map(|(agent, status)| format!("{agent}={status}"))
                .collect::<Vec<_>>()
                .join(", ");
            let message = format!("{} agent(s) did not complete: {detail}", incomplete.len());
            report.status = pipeline_store::STATUS_AWAITING_USER.to_string();
            report.error = Some(message.clone());
            finalize_report(db, opts, &mut report, &state)?;
            {
                let conn = lock(db)?;
                pipeline_store::set_error(&conn, &opts.run_id, &message)?;
                pipeline_store::update_status(
                    &conn,
                    &opts.run_id,
                    pipeline_store::STATUS_AWAITING_USER,
                    Some("report"),
                )?;
            }
            adapters.event_sink.emit("report", "agents_incomplete", &message);
            return Ok(report);
        }
    }

    // A run whose verification failed is NOT a success, even though the only
    // stage it affects (deploy) was skipped rather than failed.
    let verification_failed = state
        .verification
        .as_ref()
        .map(|verification| !verification.passed)
        .unwrap_or(false);

    if verification_failed && !opts.allow_deploy_on_failed_verification {
        report.status = pipeline_store::STATUS_VERIFICATION_FAILED.to_string();
        report.error = Some("verification failed".to_string());
        finalize_report(db, opts, &mut report, &state)?;
        {
            let conn = lock(db)?;
            pipeline_store::update_status(
                &conn,
                &opts.run_id,
                pipeline_store::STATUS_VERIFICATION_FAILED,
                Some("report"),
            )?;
        }
        adapters
            .event_sink
            .emit("report", "verification_failed", "verification failed");
        return Ok(report);
    }

    report.status = pipeline_store::STATUS_SUCCEEDED.to_string();
    finalize_report(db, opts, &mut report, &state)?;
    {
        let conn = lock(db)?;
        pipeline_store::update_status(&conn, &opts.run_id, pipeline_store::STATUS_SUCCEEDED, Some("report"))?;
    }
    adapters.event_sink.emit("report", "done", "pipeline complete");
    Ok(report)
}

fn finalize_report(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    report: &mut BuildReport,
    state: &PipelineState,
) -> Result<(), String> {
    report.stack_id = state.stack_id.clone();
    report.plan_id = state.plan_id.clone();
    report.wave = state.wave_report.clone();
    report.verification = state
        .verification
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap_or_default());
    report.deploy = state.deploy.clone();
    report.compounder_items = state.compounder_items;
    report.artifacts = serde_json::json!({
        "project_path": opts.project_path,
        "spec_path": opts.spec_path,
        // The effective options are persisted so a resume reproduces the run
        // instead of silently falling back to defaults (M12).
        "agent_command": opts.agent_command,
        "base_branch": opts.base_branch,
        "allow_deploy_on_failed_verification": opts.allow_deploy_on_failed_verification,
        "generate_dockerfile": opts.generate_dockerfile,
        "agent_timeout_secs": opts.agent_timeout_secs,
    });
    // Delivery evidence: which agent branches were merged into the base branch.
    if let Some(merges) = &state.merges {
        report.artifacts["merges"] =
            serde_json::to_value(merges).unwrap_or(serde_json::Value::Null);
    }

    let conn = lock(db)?;
    pipeline_store::set_report(&conn, &opts.run_id, &serde_json::to_value(report).unwrap_or_default())
}

fn run_stage(
    stage: BuildStage,
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    match stage {
        BuildStage::ParseSpec => stage_parse_spec(opts, state),
        BuildStage::ResolveStack => stage_resolve_stack(db, opts, state),
        BuildStage::Provision => stage_provision(db, opts, adapters, state),
        BuildStage::Scaffold => stage_scaffold(opts, state),
        BuildStage::SeedPlan => stage_seed_plan(db, opts, state),
        BuildStage::ExecuteWaves => stage_execute_waves(db, opts, adapters, state),
        BuildStage::FinalizeAndVerify => stage_finalize_and_verify(db, opts, adapters, state),
        BuildStage::Deploy => stage_deploy(opts, adapters, state),
        BuildStage::Report => Ok(StageResult::Done("report assembled".to_string())),
    }
}

fn stage_parse_spec(
    opts: &PipelineOptions,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    let content = std::fs::read_to_string(&opts.spec_path)
        .map_err(|e| format!("Cannot read spec '{}': {e}", opts.spec_path))?;
    let tasks = crate::spec_parser::parse_gap_closure_plan(&content);
    if tasks.is_empty() {
        return Err(format!(
            "No tasks parsed from spec '{}' — expected '## Phase N' + '### Step X.Y' headers",
            opts.spec_path
        ));
    }
    let count = tasks.len();
    state.tasks = tasks;
    Ok(StageResult::Done(format!("{count} tasks parsed")))
}

fn stage_resolve_stack(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    let stack_id = match &opts.stack_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            let conn = lock(db)?;
            crate::preferences::get_preferences(&conn)?.preferred_stack
        }
    };

    if stack_registry::StackPreset::get_by_id(&stack_id).is_none() {
        return Err(format!(
            "Unknown stack '{stack_id}'. Valid: {:?}",
            stack_registry::StackPreset::all_ids()
        ));
    }

    state.stack_id = Some(stack_id.clone());
    Ok(StageResult::Done(format!("stack={stack_id}")))
}

fn stage_provision(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    let stack_id = state
        .stack_id
        .clone()
        .ok_or_else(|| "resolve_stack must run before provision".to_string())?;

    let status = adapters
        .cli_status
        .clone()
        .unwrap_or_else(stack_registry::detect_installed_clis);
    let mut missing = stack_registry::missing_clis_for_stack(&stack_id, &status);

    // Supabase MCP decision (§6.2): a cloud config wins; otherwise fall back to
    // a containerized Postgres on this machine (local-first database target).
    // Only a dead Docker daemon pauses the run — everything else is automatic.
    let stack = stack_registry::StackPreset::get_by_id(&stack_id)
        .ok_or_else(|| format!("Unknown stack '{stack_id}'"))?;
    let mut missing_mcp: Vec<String> = Vec::new();
    let mut local_db_note: Option<String> = None;
    if stack.required_mcp.iter().any(|m| m == "supabase") {
        let conn = lock(db)?;
        if !has_supabase_config(&conn, &state.project_id) {
            match crate::database::ensure_local_postgres(
                &conn,
                &state.project_id,
                &opts.project_path,
            ) {
                Ok(target) => {
                    local_db_note = Some(format!(
                        "local postgres up ({})",
                        target.container_name.as_deref().unwrap_or("db")
                    ));
                }
                Err(crate::database::ProvisionError::DaemonDown(_)) => {
                    missing_mcp.push("supabase-mcp".to_string());
                }
                Err(crate::database::ProvisionError::Failed(error)) => return Err(error),
            }
        }
    }

    if missing.is_empty() && missing_mcp.is_empty() {
        let mut message = "all CLIs + MCPs available".to_string();
        if let Some(note) = local_db_note {
            message = format!("{message}; {note}");
        }
        return Ok(StageResult::Done(message));
    }

    let mut delegation = crate::delegation::DelegationReport::new();
    if !missing.is_empty() {
        delegation.add_task(
            "provision-clis",
            &format!("Install missing CLIs: {}", missing.join(", ")),
            "Required by the selected stack",
        );
    }
    if !missing_mcp.is_empty() {
        delegation.add_task(
            "provision-supabase",
            "Connect Supabase MCP (Integrations → Supabase), or start Docker Desktop so the local Postgres can be provisioned instead",
            "Stack requires a Supabase database",
        );
    }

    missing.extend(missing_mcp);
    Ok(StageResult::AwaitingUser(format!(
        "Awaiting user: {}. Delegation tasks: {}",
        missing.join(", "),
        delegation.tasks.len()
    )))
}

fn has_supabase_config(db: &Connection, project_id: &str) -> bool {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM supabase_configs WHERE project_id = ?1)",
        rusqlite::params![project_id],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        == 1
}

fn stage_scaffold(opts: &PipelineOptions, state: &mut PipelineState) -> Result<StageResult, String> {
    let stack_id = state
        .stack_id
        .clone()
        .ok_or_else(|| "resolve_stack must run before scaffold".to_string())?;

    if Path::new(&opts.project_path).join("package.json").exists() {
        return Ok(StageResult::Skipped(
            "project already scaffolded (package.json present)".to_string(),
        ));
    }

    let project_name = Path::new(&opts.project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());

    let report = crate::arch_engine::scaffold_project(&stack_id, &opts.project_path, &project_name)?;
    Ok(StageResult::Done(format!(
        "scaffolded {} files",
        report.files_created.len()
    )))
}

fn stage_seed_plan(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    if state.plan_id.is_some() {
        return Ok(StageResult::Skipped(
            "plan already seeded (resume)".to_string(),
        ));
    }

    let conn = lock(db)?;
    if state.project_id.is_empty() {
        state.project_id = resolve_project_id(&conn, opts)?;
    }

    let slug = Path::new(&opts.spec_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase().replace(' ', "-"))
        .unwrap_or_else(|| "build".to_string());

    let plan = crate::orchestrator::create_wave_plan(&conn, &state.project_id, &slug)?;
    for task in &state.tasks {
        let description = if task.objective.is_empty() {
            task.title.clone()
        } else {
            task.objective.clone()
        };
        crate::orchestrator::add_plan_agent(
            &conn,
            &plan.id,
            &format!("{}-{}", task.phase, task.step),
            &description,
            task.wave,
            task.depends_on.as_deref(),
            None,
        )?;
    }

    let count = state.tasks.len();
    state.plan_id = Some(plan.id.clone());
    Ok(StageResult::Done(format!(
        "plan {} seeded with {count} agents",
        plan.id
    )))
}

fn resolve_project_id(db: &Connection, opts: &PipelineOptions) -> Result<String, String> {
    if let Some(project_id) = &opts.project_id {
        ensure_project_row(db, project_id, &opts.project_path)?;
        return Ok(project_id.clone());
    }

    if let Ok(existing) = db.query_row(
        "SELECT id FROM projects WHERE path = ?1",
        rusqlite::params![opts.project_path],
        |row| row.get::<_, String>(0),
    ) {
        return Ok(existing);
    }

    let id = format!("proj-{}", uuid::Uuid::new_v4());
    ensure_project_row(db, &id, &opts.project_path)?;
    Ok(id)
}

fn ensure_project_row(db: &Connection, project_id: &str, project_path: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR IGNORE INTO projects (id, path, name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![project_id, project_path, project_id, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn stage_execute_waves(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    if state.wave_report.is_some() {
        return Ok(StageResult::Skipped(
            "wave already executed (resume)".to_string(),
        ));
    }

    let plan_id = state
        .plan_id
        .clone()
        .ok_or_else(|| "seed_plan must run before execute_waves".to_string())?;

    let mut report = adapters.wave_runner.run(db, &plan_id, &state.project_path)?;
    let agents = report.agents.len();

    // Production path: supervise handoffs (deadline/retry/correction/cancel).
    // Hermetic test runners return no control and skip this.
    if let Some(control) = adapters.wave_runner.control() {
        let config = crate::wave_supervisor::SupervisionConfig {
            timeout: std::time::Duration::from_secs(opts.agent_timeout_secs.max(1)),
            poll_interval: std::time::Duration::from_secs(2),
            max_retries: 1,
            cost_cap_usd: None,
        };
        let run_id = state.run_id.clone();
        let cancel_check = || {
            db.lock()
                .ok()
                .and_then(|conn| pipeline_store::is_cancelled(&conn, &run_id).ok())
                .unwrap_or(false)
        };
        let outcome = crate::wave_supervisor::supervise_agents(
            db,
            &mut report,
            &config,
            control,
            &cancel_check,
        )?;
        if outcome.cancelled {
            state.wave_report = Some(report);
            return Ok(StageResult::Done(
                "cancellation requested during agent execution".to_string(),
            ));
        }
    }

    state.wave_report = Some(report);
    Ok(StageResult::Done(format!("{agents} agents spawned")))
}

fn stage_finalize_and_verify(
    db: &Mutex<Connection>,
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    if state.verification.is_some() {
        return Ok(StageResult::Skipped(
            "verification already recorded (resume)".to_string(),
        ));
    }

    let project_path = state.project_path.clone();
    let wave_report = state
        .wave_report
        .clone()
        .ok_or_else(|| "execute_waves must run before finalize_and_verify".to_string())?;

    let finalized = tauri::async_runtime::block_on(crate::wave_executor::finalize_wave(
        db,
        wave_report,
    ))?;

    // Feed the compounder: the adapter execution path writes no session events,
    // so derive them from the handoffs (otherwise compounder_items is always 0).
    // Must run before the merge removes the worktrees.
    record_handoff_events(db, &finalized);

    // Deliver: commit each completed agent's work and merge it into the base
    // branch. Without this the agents' work stays stranded in worktrees and the
    // project being verified/deployed contains none of it.
    if adapters.deliver {
        let merges = crate::wave_executor::merge_completed_agents(
            &finalized,
            &project_path,
            &opts.base_branch,
        );
        adapters.event_sink.emit(
            "finalize_and_verify",
            "progress",
            &format!(
                "delivered {} agent branch(es), {} conflict(s)",
                merges.merged.len(),
                merges.conflicts.len()
            ),
        );
        state.merges = Some(merges);
    }

    // Real build/test gate: install the project's dependencies so the build,
    // typecheck and runtime checks actually execute instead of being Skipped.
    if adapters.install_deps {
        let detail = install_project_dependencies(&project_path)?;
        adapters
            .event_sink
            .emit("finalize_and_verify", "progress", &detail);
        // Apply the Prisma schema against the provisioned database (local
        // container or cloud) so the app and its checks run against a real schema.
        let schema_detail = crate::database::apply_prisma_schema(&project_path)?;
        adapters
            .event_sink
            .emit("finalize_and_verify", "progress", &schema_detail);
    }

    let verification = if adapters.install_deps {
        crate::verification::verify_project_with(
            Path::new(&project_path),
            crate::verification::VerifyMode::RequireBuild,
        )
    } else {
        crate::verification::verify_project(Path::new(&project_path))
    };
    let passed = verification.passed;
    state.verification = Some(verification);

    // Auto-compounder (§6.3): best-effort, never fails the stage.
    if adapters.run_compounder {
        let session_ids: Vec<String> = finalized
            .agents
            .iter()
            .map(|agent| agent.session_id.clone())
            .filter(|id| !id.is_empty())
            .collect();
        let mut items = 0usize;
        for session_id in session_ids {
            match compound_session(db, adapters, &session_id, &state.project_id) {
                Ok(count) => items += count,
                Err(error) => {
                    adapters.event_sink.emit(
                        "finalize_and_verify",
                        "warning",
                        &format!("compounder skipped for {session_id}: {error}"),
                    );
                }
            }
        }
        state.compounder_items = items;
    }

    state.wave_report = Some(finalized);
    Ok(StageResult::Done(format!("verification passed={passed}")))
}

/// Record `file_edit` events for each file a completed agent reported in its
/// handoff, so the knowledge compounder has real session activity to work with.
fn record_handoff_events(db: &Mutex<Connection>, report: &WaveExecutionReport) {
    let Ok(conn) = db.lock() else {
        return;
    };
    for agent in &report.agents {
        if agent.status != "done" {
            continue;
        }
        let handoff = Path::new(&agent.worktree_path)
            .join(format!("HANDOFF_{}.md", agent.agent_ref));
        let Ok(envelope) = crate::handoff_parser::parse_handoff_file(&handoff) else {
            continue;
        };
        for file in &envelope.changed_files {
            let _ = conn.execute(
                "INSERT INTO events (id, session_id, timestamp, event_type, target, lines_added, lines_removed)
                 VALUES (?1, ?2, datetime('now'), 'file_edit', ?3, 1, 0)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), agent.session_id, file],
            );
        }
    }
}

fn compound_session(
    db: &Mutex<Connection>,
    adapters: &PipelineAdapters,
    session_id: &str,
    project_id: &str,
) -> Result<usize, String> {
    let prompt = {
        let conn = lock(db)?;
        crate::knowledge::compounder_prepare_prompt(&conn, session_id)?
    };
    let Some(prompt) = prompt else {
        return Ok(0);
    };

    let content = tauri::async_runtime::block_on(crate::compounder_llm::complete(
        adapters.llm,
        &prompt,
    ))?;

    let conn = lock(db)?;
    let items = crate::knowledge::compounder_merge(
        &conn,
        session_id,
        Some(project_id),
        &content,
    )?;
    Ok(items.len())
}

/// Hard cap on the dependency install so a hung registry cannot wedge a run.
const DEPENDENCY_INSTALL_TIMEOUT_SECS: u64 = 900;

/// Install the project's dependencies (`npm ci` when a lock file exists).
///
/// Without this the build/typecheck/runtime checks only had the option to Skip,
/// so "verification passed" could mean "nothing was ever built".
fn install_project_dependencies(project_path: &str) -> Result<String, String> {
    let base = Path::new(project_path);
    if !base.join("package.json").exists() {
        return Ok("no package.json — no dependencies to install".to_string());
    }
    let args: Vec<&str> = if base.join("package-lock.json").exists() {
        vec!["ci", "--no-audit", "--no-fund"]
    } else {
        vec!["install", "--no-audit", "--no-fund"]
    };

    // Bootstrap `.env` from `.env.example` when missing: toolchains run during
    // `npm install` (e.g. Prisma's `postinstall: prisma generate`) resolve the
    // datasource URL and fail hard without it. Never clobber a real `.env`.
    let mut bootstrapped = false;
    let env_example = base.join(".env.example");
    let env_file = base.join(".env");
    if !env_file.exists() && env_example.exists() {
        std::fs::copy(&env_example, &env_file)
            .map_err(|e| format!("cannot bootstrap .env from .env.example: {e}"))?;
        bootstrapped = true;
    }

    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg("npm");
        c
    } else {
        std::process::Command::new("npm")
    };
    cmd.args(&args).current_dir(base);

    run_with_timeout(cmd, DEPENDENCY_INSTALL_TIMEOUT_SECS).map(|_| {
        format!(
            "dependencies installed (npm {}{})",
            args.join(" "),
            if bootstrapped { ", .env bootstrapped" } else { "" }
        )
    })
}

/// Run a command with a wall-clock timeout, sending its output to a temp log so
/// a chatty child cannot deadlock on a full pipe. Failures carry the log tail.
pub(crate) fn run_with_timeout(mut cmd: std::process::Command, secs: u64) -> Result<(), String> {
    let log_path =
        std::env::temp_dir().join(format!("sourceforge-cmd-{}.log", std::process::id()));
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log))
        .stderr(std::process::Stdio::from(log_err))
        .spawn()
        .map_err(|e| format!("cannot start command: {e}"))?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "command failed ({status}); output tail: {}",
                    tail_of(&log_path)
                ));
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "timed out after {secs}s; output tail: {}",
                        tail_of(&log_path)
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

pub(crate) fn tail_of(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let tail: Vec<&str> = text.lines().rev().take(8).collect();
    tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

/// Agents that did not finish with a valid handoff, as `(agent_ref, status)`.
/// A non-empty result means the promised work was not done.
fn incomplete_agents(report: &crate::wave_executor::WaveExecutionReport) -> Vec<(String, String)> {
    report
        .agents
        .iter()
        .filter(|agent| agent.status != "done")
        .map(|agent| (agent.agent_ref.clone(), agent.status.clone()))
        .collect()
}

fn stage_deploy(
    opts: &PipelineOptions,
    adapters: &PipelineAdapters,
    state: &mut PipelineState,
) -> Result<StageResult, String> {
    if state.deploy.is_some() {
        return Ok(StageResult::Skipped(
            "already deployed (resume)".to_string(),
        ));
    }

    let stack_id = state.stack_id.clone().unwrap_or_default();
    let verification_passed = state
        .verification
        .as_ref()
        .map(|v| v.passed)
        .unwrap_or(false);

    if !verification_passed && !opts.allow_deploy_on_failed_verification {
        return Ok(StageResult::Skipped(
            "verification failed — deploy skipped (override with allow_deploy_on_failed_verification)"
                .to_string(),
        ));
    }

    // A wave whose agents never completed would deploy nothing (or whatever
    // happened to be in the worktree), so gate it like a failed verification
    // instead of pushing a half-done project to production.
    if !opts.allow_deploy_on_failed_verification {
        if let Some(merges) = &state.merges {
            if !merges.conflicts.is_empty() {
                return Ok(StageResult::Skipped(format!(
                    "{} agent branch(es) could not be merged — deploy skipped",
                    merges.conflicts.len()
                )));
            }
        }
        if let Some(wave) = &state.wave_report {
            let incomplete = incomplete_agents(wave);
            if !incomplete.is_empty() {
                let detail = incomplete
                    .iter()
                    .map(|(agent, status)| format!("{agent}={status}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Ok(StageResult::Skipped(format!(
                    "{} agent(s) did not complete ({detail}) — deploy skipped",
                    incomplete.len()
                )));
            }
        }
    }

    let outcome = adapters
        .deployer
        .deploy(Path::new(&opts.project_path), &stack_id)?;

    // Dockerfile artifact for non-Vercel hosts (§6.1) — never executed in v1.
    let dockerfile = if opts.generate_dockerfile {
        crate::deployer::write_dockerfile(Path::new(&opts.project_path), &stack_id)?
    } else {
        None
    };

    let url = outcome.url.clone().unwrap_or_else(|| "(no url)".to_string());
    state.deploy = Some(outcome);
    Ok(StageResult::Done(format!(
        "deployed via {} → {url}{}",
        state
            .deploy
            .as_ref()
            .map(|d| d.provider.clone())
            .unwrap_or_default(),
        if dockerfile.is_some() {
            " (+ Dockerfile artifact)"
        } else {
            ""
        }
    )))
}

// Small helper so `stage_seed_plan` can capture the project path once.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployer::MockDeployer;
    use tempfile::TempDir;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql")).unwrap();
        conn.execute_batch(include_str!("../migrations/010_knowledge_graph.sql")).unwrap();
        conn.execute_batch(include_str!("../migrations/014_bagua_semantics.sql")).unwrap();
        conn.execute_batch(include_str!("../migrations/017_build_runs.sql")).unwrap();
        conn
    }

    /// A project that passes every deterministic verification check.
    fn make_verifiable_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        std::fs::write(
            base.join("package.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "pipeline-fixture",
                "scripts": {
                    "dev": "vite",
                    "build": "vite build",
                    "start": "vite preview",
                    "typecheck": "tsc --noEmit",
                    "test": "vitest run"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(base.join("dist")).unwrap();
        std::fs::write(
            base.join("dist").join("index.html"),
            "<html><body><div id=\"root\"></div><script src=\"/assets/index.js\"></script></body></html>",
        )
        .unwrap();
        std::fs::write(
            base.join("vercel.json"),
            r#"{"rewrites":[{"source":"/((?!api/).*)","destination":"/index.html"}]}"#,
        )
        .unwrap();
        std::fs::write(
            base.join("README.md"),
            "# Pipeline Fixture\n\n## Setup\n\nRun `npm install` then `npm run dev` for local development.\n\
             Build with `npm run build` and preview with `npm start`.\n\n\
             ## Environment Variables\n\nCopy `.env.example` to `.env` and fill in the required values.\n\
             Required variables: JWT_SECRET, DATABASE_URL, PORT, CLIENT_URL.\n\n\
             ## Architecture\n\nVite + React application with an API surface, tested with Vitest \
             and deployed to Vercel with SPA rewrites. The build pipeline verifies build output, \
             SPA routing, runtime smoke tests, and API client production configuration.\n\n\
             ## Testing\n\nRun `npm test` for unit tests and `npm run typecheck` for static analysis.\n\
             The verification pipeline checks the production build, deploy config, and runtime behavior.\n",
        )
        .unwrap();
        std::fs::write(
            base.join(".env.example"),
            "JWT_SECRET=change-me\nDATABASE_URL=postgres://localhost/app\nPORT=3001\nCLIENT_URL=http://localhost:5173\n",
        )
        .unwrap();
        std::fs::create_dir_all(base.join("src").join("api")).unwrap();
        std::fs::write(
            base.join("src").join("App.tsx"),
            "import { ErrorBoundary } from 'react-error-boundary';\nexport default function App() { return <div/>; }",
        )
        .unwrap();
        std::fs::write(
            base.join("src").join("api").join("client.ts"),
            "const api = axios.create({ baseURL: import.meta.env.VITE_API_URL || '/api' });",
        )
        .unwrap();

        let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(base).output();
        let _ = std::process::Command::new("git")
            .args(["remote", "add", "origin", "https://example.com/fixture.git"])
            .current_dir(base)
            .output();
        dir
    }

    fn make_spec(dir: &TempDir) -> String {
        let path = dir.path().join("plan.md");
        std::fs::write(
            &path,
            "# Plan\n\n## Phase 1: Build\n\n### Step 1.1: Do the thing\n**Wave:** A · **Depends on:** —\nbody\n\n\
             ### Step 1.2: Do the other thing\n**Wave:** A · **Depends on:** 1.1\nbody\n",
        )
        .unwrap();
        path.to_string_lossy().to_string()
    }

    /// Writes a valid handoff per plan agent into a temp "worktree" and returns
    /// a wave report — hermetic stand-in for real agent execution.
    struct TestWaveRunner;

    impl WaveRunner for TestWaveRunner {
        fn run(
            &self,
            db: &Mutex<Connection>,
            plan_id: &str,
            _base_repo: &str,
        ) -> Result<WaveExecutionReport, String> {
            let agents = {
                let conn = db.lock().map_err(|e| e.to_string())?;
                crate::orchestrator::get_plan_agents(&conn, plan_id)?
            };

            let mut report = WaveExecutionReport {
                plan_id: plan_id.to_string(),
                base_repo: "/fixture".to_string(),
                started_at: Utc::now().to_rfc3339(),
                completed_at: Some(Utc::now().to_rfc3339()),
                ..Default::default()
            };

            for agent in agents {
                let worktree = std::env::temp_dir().join(format!("acc-wave-{plan_id}-{}", agent.agent_ref));
                std::fs::create_dir_all(&worktree).map_err(|e| e.to_string())?;
                let handoff = worktree.join(format!("HANDOFF_{}.md", agent.agent_ref));
                std::fs::write(
                    &handoff,
                    "## Original Task\nDo the thing\n\n## Completed By\nmock\n\n## Model Used\nmock-1\n\n\
                     ## Output Summary\nDone\n\n## Completed Work\nDone\n\n## Test Results\nAll pass\n\n\
                     ## Files Changed\n- src/registry.rs\n\n## Files NOT Modified\n- package.json\n\n\
                     ## Design Decisions\nNone\n\n## Interface Contracts Exposed\nNone\n\n\
                     ## Handoff Instructions\nNone\n",
                )
                .map_err(|e| e.to_string())?;

                let session_id = format!("session-{}-{}", plan_id, agent.agent_ref);
                {
                    // Events are NOT seeded here: the pipeline derives them from
                    // the handoff (record_handoff_events), which is what H7 fixes.
                    let conn = db.lock().map_err(|e| e.to_string())?;
                    conn.execute(
                        "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?1, datetime('now'))",
                        rusqlite::params![session_id],
                    )
                    .map_err(|e| e.to_string())?;
                }

                report.agents.push(AgentExecution {
                    agent_ref: agent.agent_ref.clone(),
                    session_id,
                    worktree_path: worktree.to_string_lossy().to_string(),
                    branch: "test".to_string(),
                    status: "running".to_string(),
                    guideline_path: String::new(),
                    cost_usd: 0.0,
                    retry_count: 0,
                });
            }

            Ok(report)
        }
    }

    fn make_opts(db: &Connection, project: &TempDir, spec: &str) -> PipelineOptions {
        let run = pipeline_store::create_run(
            db,
            None,
            spec,
            &project.path().to_string_lossy(),
            Some("nextjs-prisma-vercel"),
        )
        .unwrap();

        PipelineOptions {
            run_id: run.id,
            project_id: None,
            spec_path: spec.to_string(),
            project_path: project.path().to_string_lossy().to_string(),
            stack_id: Some("nextjs-prisma-vercel".to_string()),
            agent_command: "mock".to_string(),
            base_branch: "main".to_string(),
            allow_deploy_on_failed_verification: false,
            generate_dockerfile: true,
            agent_timeout_secs: 300,
        }
    }

    fn all_clis_installed() -> HashMap<String, CliStatus> {
        let mut map = HashMap::new();
        for cli in ["node", "npm", "vercel", "python3", "pip", "git"] {
            map.insert(cli.to_string(), CliStatus::Installed("1.0".to_string()));
        }
        map
    }

    #[test]
    fn test_stage_order_is_stable() {
        let names: Vec<&str> = BuildStage::order().iter().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "parse_spec",
                "resolve_stack",
                "provision",
                "scaffold",
                "seed_plan",
                "execute_waves",
                "finalize_and_verify",
                "deploy",
                "report"
            ]
        );
    }

    #[test]
    fn test_full_pipeline_with_mock_runner_produces_complete_report() {
        let conn = setup_db();
        let db = Mutex::new(conn);
        let project = make_verifiable_project();
        let spec_dir = TempDir::new().unwrap();
        let spec = make_spec(&spec_dir);

        let opts = {
            let conn = db.lock().unwrap();
            make_opts(&conn, &project, &spec)
        };

        let deployer = MockDeployer {
            url: "https://mock.example.app".to_string(),
            fail: false,
        };
        let llm = crate::compounder_llm::LlmProvider::Static(
            r#"[{"title":"Registry pattern","content":"Use LazyLock for registries","category":"pattern","confidence":0.8}]"#
                .to_string(),
        );
        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &TestWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(all_clis_installed()),
            install_deps: false,
            deliver: false,
            run_compounder: true,
        };

        let report = run_pipeline(&db, &opts, &adapters).unwrap();

        assert_eq!(report.status, pipeline_store::STATUS_SUCCEEDED, "error: {:?}", report.error);
        assert_eq!(report.stages.len(), 9);
        assert!(report.stages.iter().all(|s| s.status == "done" || s.status == "skipped"));
        assert!(report.plan_id.is_some());
        assert_eq!(report.wave.as_ref().unwrap().agents.len(), 2);
        let verification = report.verification.as_ref().unwrap();
        assert_eq!(
            verification["passed"], true,
            "failing checks: {}",
            verification["checks"]
                .as_array()
                .map(|checks| checks
                    .iter()
                    .filter(|c| c["status"] != "Pass")
                    .map(|c| format!("{}={}", c["name"], c["status"]))
                    .collect::<Vec<_>>()
                    .join(", "))
                .unwrap_or_default()
        );
        assert_eq!(
            report.deploy.as_ref().unwrap().url.as_deref(),
            Some("https://mock.example.app")
        );
        // Compounder candidate generation needs >=2 signals per session, which a
        // single handoff cannot provide; the handoff->events recording itself is
        // covered by test_record_handoff_events_persists_events.
        assert!(project.path().join("Dockerfile").exists(), "Dockerfile artifact");

        // Persisted run reflects the report.
        let run = {
            let conn = db.lock().unwrap();
            pipeline_store::get_run(&conn, &opts.run_id).unwrap()
        };
        assert_eq!(run.status, pipeline_store::STATUS_SUCCEEDED);
        assert!(run.report.is_some());
    }

    #[test]
    fn test_record_handoff_events_persists_events() {
        // H7: the adapter path writes no session events; the pipeline must
        // derive them from handoffs so the compounder has real activity.
        let conn = setup_db();
        let db = Mutex::new(conn);
        let dir = TempDir::new().unwrap();
        let worktree = dir.path().to_string_lossy().to_string();

        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO sessions (id, started_at) VALUES ('sess-h7', datetime('now'))",
                [],
            )
            .unwrap();
        }
        std::fs::write(
            dir.path().join("HANDOFF_1-1.md"),
            "## Original Task\nx\n\n## Completed By\nmock\n\n## Model Used\nmock-1\n\n\
             ## Output Summary\nx\n\n## Completed Work\nx\n\n## Test Results\nx\n\n\
             ## Files Changed\n- src/registry.rs\n- src/lib.rs\n\n## Files NOT Modified\n- x\n\n\
             ## Design Decisions\nx\n\n## Interface Contracts Exposed\nx\n\n## Handoff Instructions\nx\n",
        )
        .unwrap();

        let mut report = WaveExecutionReport {
            plan_id: "plan-1".to_string(),
            ..Default::default()
        };
        report.agents.push(AgentExecution {
            agent_ref: "1-1".to_string(),
            session_id: "sess-h7".to_string(),
            worktree_path: worktree,
            branch: "b".to_string(),
            status: "done".to_string(),
            guideline_path: String::new(),
            cost_usd: 0.0,
            retry_count: 0,
        });

        record_handoff_events(&db, &report);

        let count: i64 = {
            let conn = db.lock().unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM events WHERE session_id = 'sess-h7'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 2, "one file_edit event per changed file");
    }

    #[test]
    fn test_missing_cli_pauses_then_resumes() {
        let conn = setup_db();
        let db = Mutex::new(conn);
        let project = make_verifiable_project();
        let spec_dir = TempDir::new().unwrap();
        let spec = make_spec(&spec_dir);

        let opts = {
            let conn = db.lock().unwrap();
            make_opts(&conn, &project, &spec)
        };

        let deployer = MockDeployer { url: "https://x".to_string(), fail: false };
        let llm = crate::compounder_llm::LlmProvider::Static("[]".to_string());

        // First pass: vercel CLI missing → awaiting_user.
        let mut status = HashMap::new();
        status.insert("node".to_string(), CliStatus::Installed("v20".to_string()));
        status.insert("npm".to_string(), CliStatus::Installed("10".to_string()));
        status.insert("git".to_string(), CliStatus::Installed("2".to_string()));
        status.insert("vercel".to_string(), CliStatus::Missing);
        status.insert("python3".to_string(), CliStatus::Missing);
        status.insert("pip".to_string(), CliStatus::Missing);

        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &TestWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(status),
            install_deps: false,
            deliver: false,
            run_compounder: false,
        };

        let first = run_pipeline(&db, &opts, &adapters).unwrap();
        assert_eq!(first.status, pipeline_store::STATUS_AWAITING_USER);
        assert!(first
            .stages
            .iter()
            .any(|s| s.name == "provision" && s.status == "awaiting_user"));

        // Second pass with all CLIs → resumes and finishes.
        let adapters_ok = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &TestWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(all_clis_installed()),
            install_deps: false,
            deliver: false,
            run_compounder: false,
        };

        let second = run_pipeline(&db, &opts, &adapters_ok).unwrap();
        assert_eq!(second.status, pipeline_store::STATUS_SUCCEEDED, "error: {:?}", second.error);

        // Resume re-ran the cheap stages and finished without duplicating the plan.
        let provision = second.stages.iter().find(|s| s.name == "provision").unwrap();
        assert_eq!(provision.status, "done");
        let plan_count: i64 = {
            let conn = db.lock().unwrap();
            conn.query_row("SELECT COUNT(*) FROM feature_plans", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(plan_count, 1, "resume must not create a second plan");
    }

    #[test]
    fn test_hard_failure_marks_run_failed() {
        let conn = setup_db();
        let db = Mutex::new(conn);
        let project = make_verifiable_project();

        let opts = {
            let conn = db.lock().unwrap();
            make_opts(&conn, &project, "/nonexistent/spec.md")
        };

        let deployer = MockDeployer { url: "https://x".to_string(), fail: false };
        let llm = crate::compounder_llm::LlmProvider::Static("[]".to_string());
        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &TestWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(all_clis_installed()),
            install_deps: false,
            deliver: false,
            run_compounder: false,
        };

        let report = run_pipeline(&db, &opts, &adapters).unwrap();
        assert_eq!(report.status, pipeline_store::STATUS_FAILED);
        assert!(report.error.unwrap().contains("Cannot read spec"));
    }

    #[test]
    fn test_cancel_before_start_returns_cancelled() {
        let conn = setup_db();
        let db = Mutex::new(conn);
        let project = make_verifiable_project();
        let spec_dir = TempDir::new().unwrap();
        let spec = make_spec(&spec_dir);

        let opts = {
            let conn = db.lock().unwrap();
            let opts = make_opts(&conn, &project, &spec);
            pipeline_store::cancel_run(&conn, &opts.run_id).unwrap();
            opts
        };

        let deployer = MockDeployer { url: "https://x".to_string(), fail: false };
        let llm = crate::compounder_llm::LlmProvider::Static("[]".to_string());
        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &TestWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(all_clis_installed()),
            install_deps: false,
            deliver: false,
            run_compounder: false,
        };

        let report = run_pipeline(&db, &opts, &adapters).unwrap();
        assert_eq!(report.status, pipeline_store::STATUS_CANCELLED);
    }

    /// A runner whose agents never write a handoff: the agents failed.
    struct HandofflessWaveRunner;

    impl WaveRunner for HandofflessWaveRunner {
        fn run(
            &self,
            db: &Mutex<Connection>,
            plan_id: &str,
            _base_repo: &str,
        ) -> Result<WaveExecutionReport, String> {
            let agents = {
                let conn = db.lock().map_err(|e| e.to_string())?;
                crate::orchestrator::get_plan_agents(&conn, plan_id)?
            };
            let mut report = WaveExecutionReport {
                plan_id: plan_id.to_string(),
                ..Default::default()
            };
            for agent in agents {
                let worktree = std::env::temp_dir()
                    .join(format!("acc-handoffless-{plan_id}-{}", agent.agent_ref));
                std::fs::create_dir_all(&worktree).map_err(|e| e.to_string())?;
                report.agents.push(AgentExecution {
                    agent_ref: agent.agent_ref.clone(),
                    session_id: format!("session-{plan_id}-{}", agent.agent_ref),
                    worktree_path: worktree.to_string_lossy().to_string(),
                    branch: "test".to_string(),
                    status: "running".to_string(),
                    guideline_path: String::new(),
                    cost_usd: 0.0,
                    retry_count: 0,
                });
            }
            Ok(report)
        }
    }

    #[test]
    fn test_incomplete_agents_do_not_report_success() {
        // The project already satisfies verification, so before this gate the
        // pipeline would report `succeeded` even though no agent completed —
        // overclaiming the product's core promise.
        let conn = setup_db();
        let db = Mutex::new(conn);
        let project = make_verifiable_project();
        let spec_dir = TempDir::new().unwrap();
        let spec = make_spec(&spec_dir);
        let opts = {
            let conn = db.lock().unwrap();
            make_opts(&conn, &project, &spec)
        };

        let deployer = MockDeployer {
            url: "https://mock.example.app".to_string(),
            fail: false,
        };
        let llm = crate::compounder_llm::LlmProvider::Static("[]".to_string());
        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &HandofflessWaveRunner,
            event_sink: &NoopEventSink,
            llm: &llm,
            cli_status: Some(all_clis_installed()),
            install_deps: false,
            deliver: false,
            run_compounder: false,
        };

        let report = run_pipeline(&db, &opts, &adapters).unwrap();

        assert_eq!(
            report.status,
            pipeline_store::STATUS_AWAITING_USER,
            "a wave with no completed agents must not report success"
        );
        assert!(report
            .wave
            .as_ref()
            .unwrap()
            .agents
            .iter()
            .all(|agent| agent.status == "failed"));
        let deploy = report
            .stages
            .iter()
            .find(|stage| stage.name == "deploy")
            .unwrap();
        assert_eq!(deploy.status, "skipped", "deploy must not run: {:?}", deploy.message);
        assert!(
            report.error.as_deref().unwrap_or_default().contains("did not complete"),
            "error should explain the incomplete agents: {:?}",
            report.error
        );
        assert!(report.deploy.is_none(), "nothing may be deployed");

        let run = {
            let conn = db.lock().unwrap();
            pipeline_store::get_run(&conn, &opts.run_id).unwrap()
        };
        assert_eq!(run.status, pipeline_store::STATUS_AWAITING_USER);
    }
}

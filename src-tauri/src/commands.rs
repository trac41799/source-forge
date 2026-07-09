use crate::acb;
use crate::backward_channel;
use crate::assets;
use crate::budget;
use crate::control;
use crate::events::{self, EventRecord, SessionSummary};
use crate::integrations;
use crate::intelligence;
use crate::kg_core;
use crate::kg_core::{BaguaEncoding, BaguaRelation, bagua_similarity, classify_relationship, encode_concept, solve_analogy};
use crate::worktree;
use crate::handoff_parser;
use crate::wave_executor;
use crate::spec_parser;
use crate::kg_git;
use crate::kg_queries;
use crate::knowledge;
use crate::orchestrator;
use crate::playbook;
use crate::pty::{AgentProcessInfo, ActiveAgentSnapshot, PtyManager};
use crate::routing;
use crate::scheduler;
use crate::skillbridge::{detect_skillbridge, SkillBridgeInfo};
use chrono::Utc;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::State;
use crate::codebase;
use crate::memory;

pub struct AppState {
    pub pty_manager: Arc<PtyManager>,
    pub db: Mutex<Connection>,
    pub current_project_path: Mutex<Option<String>>,
    pub memory_circuit_breaker: Mutex<memory::CircuitBreaker>,
    pub memory_anti_thrashing: Mutex<HashMap<String, (i64, bool)>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Connection::open(":memory:").unwrap())
    }
}

impl AppState {
    pub fn new(db: Connection) -> Self {
        Self {
            pty_manager: Arc::new(PtyManager::new()),
            db: Mutex::new(db),
            current_project_path: Mutex::new(None),
            memory_circuit_breaker: Mutex::new(memory::CircuitBreaker::new()),
            memory_anti_thrashing: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn spawn_agent(
    agent_id: String,
    project_path: String,
    command: String,
    args: Vec<String>,
    env_vars: HashMap<String, String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    {
        let mut path = state.current_project_path.lock().map_err(|e| e.to_string())?;
        *path = Some(project_path.clone());
    }
    let session_id = state
        .pty_manager
        .spawn_process(agent_id.clone(), project_path, command, args, env_vars)
        .await?;
    let _ = save_state_inner(&state).await;
    Ok(session_id)
}

#[tauri::command]
pub async fn spawn_agent_with_guards(
    agent_id: String,
    project_path: String,
    command: String,
    args: Vec<String>,
    env_vars: HashMap<String, String>,
    deadline_secs: Option<u64>,
    cost_cap_usd: Option<f64>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    {
        let mut path = state.current_project_path.lock().map_err(|e| e.to_string())?;
        *path = Some(project_path.clone());
    }
    let session_id = state
        .pty_manager
        .spawn_process_with_guards(
            agent_id.clone(),
            project_path,
            command,
            args,
            env_vars,
            deadline_secs,
            cost_cap_usd,
        )
        .await?;
    let _ = save_state_inner(&state).await;
    Ok(session_id)
}

#[tauri::command]
pub async fn kill_agent(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pty_manager.kill_process(&agent_id).await?;
    let _ = save_state_inner(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn create_worktree_cmd(
    repo_path: String,
    branch: String,
    worktree_path: String,
    base_branch: String,
) -> Result<String, String> {
    let path = worktree::create_worktree(&repo_path, &branch, &worktree_path, &base_branch)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn remove_worktree_cmd(
    repo_path: String,
    worktree_path: String,
) -> Result<(), String> {
    worktree::remove_worktree(&repo_path, &worktree_path)
}

#[tauri::command]
pub async fn list_worktrees_cmd(
    repo_path: String,
) -> Result<Vec<String>, String> {
    worktree::list_worktrees(&repo_path)
}

#[tauri::command]
pub async fn parse_handoff_file_cmd(
    path: String,
) -> Result<handoff_parser::HandoffEnvelope, String> {
    handoff_parser::parse_handoff_file(std::path::Path::new(&path))
}

#[tauri::command]
pub async fn write_to_agent(
    agent_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pty_manager.write_to_process(&agent_id, &text).await
}

#[tauri::command]
pub async fn list_agents(
    state: State<'_, AppState>,
) -> Result<Vec<AgentProcessInfo>, String> {
    Ok(state.pty_manager.list_processes().await)
}

#[tauri::command]
pub async fn get_agent_output(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let recv = state.pty_manager.get_output_receiver(&agent_id).await?;
    Ok(match recv {
        Some(_) => Some(agent_id),
        None => None,
    })
}

#[tauri::command]
pub async fn log_event(
    state: State<'_, AppState>,
    session_id: String,
    agent_id: String,
    event_type: String,
    target: Option<String>,
    lines_added: Option<i64>,
    lines_removed: Option<i64>,
    exit_code: Option<i64>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    events::log_event(
        &db,
        &session_id,
        &agent_id,
        &event_type,
        target.as_deref(),
        lines_added,
        lines_removed,
        exit_code,
    )
}

#[tauri::command]
pub async fn log_event_with_payload(
    state: State<'_, AppState>,
    session_id: String,
    agent_id: String,
    event_type: String,
    target: Option<String>,
    detail: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    events::log_event_with_payload(
        &db,
        &session_id,
        &agent_id,
        &event_type,
        target.as_deref(),
        &detail,
    )
}

#[tauri::command]
pub async fn get_events(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<EventRecord>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    events::get_session_events(&db, &session_id)
}

#[tauri::command]
pub async fn get_event_detail(
    state: State<'_, AppState>,
    event_id: String,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    events::get_session_event_detail(&db, &event_id)
}

#[tauri::command]
pub async fn get_all_sessions_cmd(
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    events::get_all_sessions(&db)
}

#[tauri::command]
pub async fn check_skillbridge() -> Result<SkillBridgeInfo, String> {
    Ok(detect_skillbridge())
}

// Integrations - Supabase
#[tauri::command]
pub async fn get_supabase_configs(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<integrations::SupabaseConfig>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::get_supabase_configs(&db, &project_id)
}

#[tauri::command]
pub async fn save_supabase_config(
    state: State<'_, AppState>,
    config: integrations::SupabaseConfig,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::save_supabase_config(&db, &config)
}

#[tauri::command]
pub async fn toggle_supabase_feature(
    state: State<'_, AppState>,
    config_id: String,
    feature: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::toggle_supabase_feature(&db, &config_id, &feature, enabled)
}

#[tauri::command]
pub async fn detect_supabase(project_path: String) -> Result<Option<String>, String> {
    Ok(integrations::detect_supabase_project(&project_path))
}

// Integrations - GitHub
#[tauri::command]
pub async fn get_github_configs(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<integrations::GitHubConfig>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::get_github_configs(&db, &project_id)
}

#[tauri::command]
pub async fn save_github_config(
    state: State<'_, AppState>,
    config: integrations::GitHubConfig,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::save_github_config(&db, &config)
}

#[tauri::command]
pub async fn toggle_github_feature(
    state: State<'_, AppState>,
    config_id: String,
    feature: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    integrations::toggle_github_feature(&db, &config_id, &feature, enabled)
}

#[tauri::command]
pub async fn detect_github_repo_cmd(project_path: String) -> Result<Option<(String, String, String)>, String> {
    Ok(integrations::detect_github_repo(&project_path))
}

#[tauri::command]
pub async fn check_repo_visibility_cmd(owner: String, repo: String) -> Result<String, String> {
    integrations::check_repo_visibility(&owner, &repo)
}

#[tauri::command]
pub async fn list_github_issues_cmd(owner: String, repo: String, state: String) -> Result<Vec<integrations::GitHubIssue>, String> {
    integrations::list_github_issues(&owner, &repo, &state)
}

#[tauri::command]
pub async fn check_migration_safety_cmd(project_path: String) -> Result<Vec<String>, String> {
    integrations::check_migration_safety(&project_path)
}

#[tauri::command]
pub async fn check_github_actions_cmd(project_path: String) -> Result<Vec<String>, String> {
    integrations::check_github_actions(&project_path)
}

#[tauri::command]
pub async fn scan_skills(path: String) -> Result<Vec<assets::SkillEntry>, String> {
    Ok(assets::scan_skills_directory(&path))
}

#[tauri::command]
pub async fn read_skill(path: String) -> Result<String, String> {
    assets::read_skill_content(&path)
}

#[tauri::command]
pub async fn scan_memory(project_path: String) -> Result<Vec<assets::MemoryFileEntry>, String> {
    Ok(assets::scan_memory_files(&project_path))
}

#[tauri::command]
pub async fn write_memory(path: String, content: String) -> Result<(), String> {
    assets::write_memory_file(&path, &content)
}

#[tauri::command]
pub async fn list_mcps(agent_config_path: String) -> Result<Vec<assets::MCPEntry>, String> {
    Ok(assets::read_mcp_configs(&agent_config_path))
}

#[tauri::command]
pub async fn toggle_mcp(
    agent_config_path: String,
    mcp_name: String,
    enabled: bool,
) -> Result<(), String> {
    assets::toggle_mcp(&agent_config_path, &mcp_name, enabled)
}

#[tauri::command]
pub async fn store_secret(
    state: State<'_, AppState>,
    key_name: String,
    value: String,
    scope: String,
    agent_id: Option<String>,
    project_id: Option<String>,
) -> Result<assets::VaultEntry, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    assets::store_secret(
        &db,
        &key_name,
        &value,
        &scope,
        agent_id.as_deref(),
        project_id.as_deref(),
    )
}

#[tauri::command]
pub async fn list_secrets(state: State<'_, AppState>) -> Result<Vec<assets::VaultEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    assets::list_secrets(&db)
}

#[tauri::command]
pub async fn get_secret_value(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    assets::get_secret_value(&db, &id)
}

#[tauri::command]
pub async fn list_plugins() -> Result<Vec<String>, String> {
    Ok(assets::list_plugins())
}

#[tauri::command]
pub async fn generate_profile(project_path: String) -> Result<serde_json::Value, String> {
    assets::generate_project_profile(&project_path)
}

// ============================================================================
// Phase 3: Intelligence Layer Commands
// ============================================================================

#[tauri::command]
pub async fn record_outcome_cmd(
    state: State<'_, AppState>,
    session_id: String,
    agent_id: String,
    task_type: String,
    outcome: String,
    duration_s: f64,
) -> Result<intelligence::OutcomeRecord, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::record_outcome(&db, &session_id, &agent_id, &task_type, &outcome, duration_s)
}

#[tauri::command]
pub async fn get_outcome_stats_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
    agent_id: Option<String>,
) -> Result<Vec<intelligence::OutcomeStats>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::get_outcome_stats(&db, project_id.as_deref(), agent_id.as_deref())
}

#[tauri::command]
pub async fn create_failure_analysis_cmd(
    state: State<'_, AppState>,
    session_id: String,
    pty_excerpt: String,
) -> Result<intelligence::FailureAnalysis, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::create_failure_analysis(&db, &session_id, &pty_excerpt)
}

#[tauri::command]
pub async fn get_failure_analyses_cmd(
    state: State<'_, AppState>,
    session_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<intelligence::FailureAnalysis>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::get_failure_analyses(&db, session_id.as_deref(), limit.unwrap_or(20))
}

#[tauri::command]
pub async fn detect_limit_event_cmd(raw_output: String) -> Result<Option<(String, String)>, String> {
    Ok(intelligence::detect_limit_event(&raw_output).map(|(a, b)| (a, b.to_string())))
}

#[tauri::command]
pub async fn record_limit_event_cmd(
    state: State<'_, AppState>,
    session_id: String,
    plan_agent_id: Option<String>,
    event_type: String,
    raw_message: String,
) -> Result<intelligence::LimitEvent, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::record_limit_event(&db, &session_id, plan_agent_id.as_deref(), &event_type, &raw_message)
}

#[tauri::command]
pub async fn resolve_limit_event_cmd(
    state: State<'_, AppState>,
    event_id: String,
    resolution: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::resolve_limit_event(&db, &event_id, &resolution)
}

#[tauri::command]
pub async fn get_unresolved_limits_cmd(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<Vec<intelligence::LimitEvent>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::get_unresolved_limits(&db, session_id.as_deref())
}

#[tauri::command]
pub async fn record_token_usage_cmd(
    state: State<'_, AppState>,
    session_id: String,
    agent_id: Option<String>,
    context: String,
    model: Option<String>,
    tokens_in: i64,
    tokens_out: i64,
) -> Result<intelligence::TokenUsage, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::record_token_usage(&db, &session_id, agent_id.as_deref(), &context, model.as_deref(), tokens_in, tokens_out)
}

#[tauri::command]
pub async fn get_token_usage_stats_cmd(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::get_token_usage_stats(&db, session_id.as_deref())
}

#[tauri::command]
pub async fn run_heartbeat_check_cmd(
    state: State<'_, AppState>,
    session_id: String,
    last_activity_at: String,
    pid_active: bool,
) -> Result<intelligence::HeartbeatResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::run_heartbeat_check(&db, &session_id, &last_activity_at, pid_active)
}

// ============================================================================
// Phase 4: Routing Commands
// ============================================================================

#[tauri::command]
pub async fn route_task_cmd(state: State<'_, AppState>, task_desc: String, task_type: String, project_id: Option<String>) -> Result<Vec<routing::TaskSuggestion>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    routing::route_task(&db, &task_desc, &task_type, project_id.as_deref())
}

#[tauri::command]
pub async fn get_models_cmd(state: State<'_, AppState>) -> Result<Vec<routing::ModelEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    routing::get_models(&db)
}

#[tauri::command]
pub async fn add_model_cmd(state: State<'_, AppState>, entry: routing::ModelEntry) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    routing::add_model(&db, &entry)
}

#[tauri::command]
pub async fn toggle_model_cmd(state: State<'_, AppState>, model_id: String, active: bool) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    routing::toggle_model(&db, &model_id, active)
}

#[tauri::command]
pub async fn build_handoff_cmd(agent_brief: routing::HandoffEnvelope) -> Result<String, String> {
    Ok(routing::build_handoff_envelope(&agent_brief))
}

#[tauri::command]
pub async fn check_agent_version_cmd(agent_id: String) -> Result<routing::AgentVersionInfo, String> {
    routing::check_agent_version(&agent_id)
}

// ============================================================================
// Phase 5: Wave Orchestrator Commands
// ============================================================================

#[tauri::command]
pub async fn create_wave_plan_cmd(state: State<'_, AppState>, project_id: String, slug: String) -> Result<orchestrator::WavePlan, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::create_wave_plan(&db, &project_id, &slug)
}

#[tauri::command]
pub async fn add_plan_agent_cmd(state: State<'_, AppState>, plan_id: String, agent_ref: String, task: String, wave: i64, depends_on: Option<String>, agent_id: Option<String>) -> Result<orchestrator::PlanAgent, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::add_plan_agent(&db, &plan_id, &agent_ref, &task, wave, depends_on.as_deref(), agent_id.as_deref())
}

#[tauri::command]
pub async fn get_plan_agents_cmd(state: State<'_, AppState>, plan_id: String) -> Result<Vec<orchestrator::PlanAgent>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::get_plan_agents(&db, &plan_id)
}

#[tauri::command]
pub async fn update_plan_agent_status_cmd(state: State<'_, AppState>, agent_id: String, status: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::update_plan_agent_status(&db, &agent_id, &status)
}

#[tauri::command]
pub async fn generate_guideline_cmd(agent_ref: String, task: String, objective: String, depends_on: Option<String>, models: Vec<String>, files_to_create: Vec<String>, files_not_touch: Vec<String>) -> Result<String, String> {
    let models_ref: Vec<&str> = models.iter().map(|s| s.as_str()).collect();
    let create_ref: Vec<&str> = files_to_create.iter().map(|s| s.as_str()).collect();
    let notouch_ref: Vec<&str> = files_not_touch.iter().map(|s| s.as_str()).collect();
    Ok(orchestrator::generate_agent_guideline(&agent_ref, &task, &objective, depends_on.as_deref(), &models_ref, &create_ref, &notouch_ref))
}

#[tauri::command]
pub async fn validate_handoff_schema_cmd(content: String) -> Result<(bool, Vec<String>), String> {
    Ok(orchestrator::validate_handoff_schema(&content))
}

#[tauri::command]
pub async fn execute_wave_cmd(
    state: State<'_, AppState>,
    plan_id: String,
    base_repo: String,
    base_branch: String,
    agent_command: String,
    agent_base_args: Vec<String>,
    deadline_secs: Option<u64>,
    cost_cap_usd: Option<f64>,
) -> Result<wave_executor::WaveExecutionReport, String> {
    let config = wave_executor::WaveRunConfig {
        plan_id,
        base_repo,
        base_branch,
        agent_command,
        agent_base_args,
        deadline_secs,
        cost_cap_usd,
    };
    let pty = &state.pty_manager;
    wave_executor::execute_wave_real(&state.db, pty, config).await
}

#[tauri::command]
pub async fn finalize_wave_cmd(
    state: State<'_, AppState>,
    report: wave_executor::WaveExecutionReport,
) -> Result<wave_executor::WaveExecutionReport, String> {
    wave_executor::finalize_wave(&state.db, report).await
}

#[tauri::command]
pub async fn seed_wave_from_spec_cmd(
    state: State<'_, AppState>,
    project_id: String,
    slug: String,
    spec_path: String,
) -> Result<orchestrator::WavePlan, String> {
    use std::fs;
    let content = fs::read_to_string(&spec_path)
        .map_err(|e| format!("Failed to read spec: {}", e))?;
    let tasks = spec_parser::parse_gap_closure_plan(&content);

    if tasks.is_empty() {
        return Err("No tasks parsed from spec".to_string());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let plan = orchestrator::create_wave_plan(&db, &project_id, &slug)?;

    for task in &tasks {
        orchestrator::add_plan_agent(
            &db,
            &plan.id,
            &format!("{}-{}", task.phase, task.step),
            &task.objective,
            task.wave,
            task.depends_on.as_deref(),
            None,
        )?;
    }

    Ok(plan)
}

// ── Deployment Verification Phase (Wave 5) ───────────────────────────

#[tauri::command]
pub async fn verify_project_cmd(
    project_path: String,
) -> Result<crate::verification::VerificationReport, String> {
    use std::path::Path;
    Ok(crate::verification::verify_project(Path::new(&project_path)))
}

#[tauri::command]
pub async fn verify_and_finalize_wave_cmd(
    state: State<'_, AppState>,
    report: wave_executor::WaveExecutionReport,
    project_path: String,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    let mut wave_report = wave_executor::finalize_wave(&state.db, report).await?;
    let verify = crate::verification::verify_project(Path::new(&project_path));
    Ok(serde_json::json!({
        "wave": wave_report,
        "verification": verify
    }))
}

#[tauri::command]
pub async fn generate_deploy_config_cmd(
    project_path: String,
) -> Result<String, String> {
    use std::path::Path;
    let vercel_path = crate::verification::generate_vercel_config(Path::new(&project_path))?;
    Ok(format!("Generated: {}", vercel_path.display()))
}

#[tauri::command]
pub async fn create_correction_cmd(state: State<'_, AppState>, plan_id: String, agent_ref: String, bug_desc: String, root_cause: String, fix_required: String, test_required: String, retry_number: i64) -> Result<orchestrator::CorrectionDoc, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::create_correction(&db, &plan_id, &agent_ref, &bug_desc, &root_cause, &fix_required, &test_required, retry_number)
}

#[tauri::command]
pub async fn get_corrections_cmd(state: State<'_, AppState>, plan_id: String) -> Result<Vec<orchestrator::CorrectionDoc>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::get_corrections(&db, &plan_id)
}

// ============================================================================
// Phase 5+: ACB Commands
// ============================================================================

#[tauri::command]
pub async fn parse_acb_signal_cmd(line: String) -> Result<Option<acb::ACBSignal>, String> {
    Ok(acb::parse_acb_signal(&line))
}

#[tauri::command]
pub async fn record_acb_signal_cmd(state: State<'_, AppState>, signal: acb::ACBSignal) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    acb::record_acb_signal(&db, &signal)
}

#[tauri::command]
pub async fn get_open_signals_cmd(state: State<'_, AppState>, session_id: Option<String>) -> Result<Vec<acb::ACBSignal>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    acb::get_open_signals(&db, session_id.as_deref())
}

#[tauri::command]
pub async fn resolve_signal_cmd(state: State<'_, AppState>, signal_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    acb::resolve_signal(&db, &signal_id)
}

// ============================================================================
// Phase 6: Team Layer Commands
// ============================================================================

#[tauri::command]
pub async fn detect_memory_candidate_cmd(output: String) -> Result<Option<String>, String> {
    Ok(playbook::detect_memory_candidate(&output))
}

#[tauri::command]
pub async fn create_memory_candidate_cmd(state: State<'_, AppState>, session_id: String, project_id: String, content: String, source_pattern: Option<String>) -> Result<playbook::MemoryCandidate, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    playbook::create_memory_candidate(&db, &session_id, &project_id, &content, source_pattern.as_deref())
}

#[tauri::command]
pub async fn get_memory_candidates_cmd(state: State<'_, AppState>, session_id: Option<String>, status: Option<String>) -> Result<Vec<playbook::MemoryCandidate>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    playbook::get_memory_candidates(&db, session_id.as_deref(), status.as_deref())
}

#[tauri::command]
pub async fn build_playbook_manifest_cmd(name: String, project: String, stacks: Vec<String>, include_skills: bool, include_memory: bool, include_presets: bool) -> Result<playbook::PlaybookManifest, String> {
    Ok(playbook::build_playbook_manifest(&name, &project, &stacks, include_skills, include_memory, include_presets))
}

#[tauri::command]
pub async fn build_feature_doc_prompt_cmd(doc_type: String, session_id: String, feature_name: String) -> Result<String, String> {
    Ok(playbook::build_feature_doc_prompt(&doc_type, &session_id, &feature_name))
}

// ============================================================================
// Subagent Orchestration commands
// ============================================================================

#[tauri::command]
pub async fn decide_orchestration_path_cmd(
    agent_ref: String,
    task: String,
    supports_subagents: bool,
    subagent_family: String,
    wave_command: String,
    model: String,
    dir: String,
) -> Result<orchestrator::OrchestrationDecision, String> {
    Ok(orchestrator::decide_orchestration_path(&agent_ref, &task, supports_subagents, &subagent_family, &wave_command, &model, &dir))
}

#[tauri::command]
pub async fn detect_subagent_spawn_cmd(
    pty_output: String,
    detection_pattern: String,
    parent_session_id: String,
    agent_ref: String,
) -> Result<Option<orchestrator::SubagentSpawn>, String> {
    Ok(orchestrator::detect_subagent_spawn(&pty_output, &detection_pattern, &parent_session_id, &agent_ref))
}

#[tauri::command]
pub async fn get_subagent_patterns_cmd() -> Result<Vec<intelligence::SubagentDetection>, String> {
    Ok(intelligence::get_subagent_patterns())
}

#[tauri::command]
pub async fn detect_subagent_activity_cmd(pty_output: String, patterns: Vec<String>) -> Result<Vec<String>, String> {
    let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
    Ok(intelligence::detect_subagent_activity(&pty_output, &pattern_refs))
}

#[tauri::command]
pub async fn record_subagent_token_usage_cmd(
    state: State<'_, AppState>,
    session_id: String,
    parent_agent_id: Option<String>,
    subagent_agent_id: Option<String>,
    context: String,
    model: Option<String>,
    tokens_in: i64,
    tokens_out: i64,
) -> Result<intelligence::TokenUsage, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    intelligence::record_subagent_token_usage(&db, &session_id, parent_agent_id.as_deref(), subagent_agent_id.as_deref(), &context, model.as_deref(), tokens_in, tokens_out)
}

#[tauri::command]
pub async fn generate_orchestrator_guideline_cmd(
    agent_ref: String,
    task: String,
    objective: String,
    depends_on: Option<String>,
    supports_subagents: bool,
    subagent_family: String,
    sub_agent_refs: Vec<String>,
    models: Vec<String>,
    files_to_create: Vec<String>,
    files_not_touch: Vec<String>,
) -> Result<String, String> {
    let sa_refs: Vec<&str> = sub_agent_refs.iter().map(|s| s.as_str()).collect();
    let m_refs: Vec<&str> = models.iter().map(|s| s.as_str()).collect();
    let fc_refs: Vec<&str> = files_to_create.iter().map(|s| s.as_str()).collect();
    let fn_refs: Vec<&str> = files_not_touch.iter().map(|s| s.as_str()).collect();
    Ok(orchestrator::generate_orchestrator_guideline(&agent_ref, &task, &objective, depends_on.as_deref(), supports_subagents, &subagent_family, &sa_refs, &m_refs, &fc_refs, &fn_refs))
}

// ============================================================================
// Phase 9+: Autonomous Scheduler Commands
// ============================================================================

#[tauri::command]
pub async fn create_cron_job_cmd(
    state: State<'_, AppState>,
    input: scheduler::CronJobInput,
) -> Result<scheduler::CronJob, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::create_cron_job(&db, &input)
}

#[tauri::command]
pub async fn get_cron_jobs_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
    enabled_only: Option<bool>,
) -> Result<Vec<scheduler::CronJob>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::get_cron_jobs(&db, project_id.as_deref(), enabled_only.unwrap_or(false))
}

#[tauri::command]
pub async fn update_cron_job_cmd(
    state: State<'_, AppState>,
    id: String,
    input: scheduler::CronJobInput,
) -> Result<scheduler::CronJob, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::update_cron_job(&db, &id, &input)
}

#[tauri::command]
pub async fn delete_cron_job_cmd(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::delete_cron_job(&db, &id)
}

#[tauri::command]
pub async fn toggle_cron_job_cmd(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::toggle_cron_job(&db, &id, enabled)
}

#[tauri::command]
pub async fn evaluate_cron_schedule_cmd(
    cron_expr: String,
    count: Option<i64>,
) -> Result<Vec<String>, String> {
    let count = count.unwrap_or(5);
    Ok(scheduler::compute_next_runs(&cron_expr, Utc::now(), count))
}

#[tauri::command]
pub async fn get_cron_executions_cmd(
    state: State<'_, AppState>,
    job_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<scheduler::CronExecution>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::get_cron_executions(&db, job_id.as_deref(), status.as_deref())
}

#[tauri::command]
pub async fn record_cron_execution_cmd(
    state: State<'_, AppState>,
    job_id: String,
    plan_id: Option<String>,
) -> Result<scheduler::CronExecution, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::record_cron_execution(&db, &job_id, plan_id.as_deref())
}

#[tauri::command]
pub async fn update_cron_execution_cmd(
    state: State<'_, AppState>,
    id: String,
    status: String,
    completed: bool,
    escalation_reason: Option<String>,
    escalation_source: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    scheduler::update_cron_execution(&db, &id, &status, completed, escalation_reason.as_deref(), escalation_source.as_deref())
}

// ============================================================================
// Phase 9++: Token Budget System Commands
// ============================================================================

#[tauri::command]
pub async fn create_budget_cmd(
    state: State<'_, AppState>,
    input: budget::BudgetInput,
) -> Result<budget::AgentBudget, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::create_budget(&db, &input)
}

#[tauri::command]
pub async fn get_budgets_cmd(
    state: State<'_, AppState>,
    session_id: Option<String>,
    plan_agent_id: Option<String>,
    state_filter: Option<String>,
) -> Result<Vec<budget::AgentBudget>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::get_budgets(
        &db,
        session_id.as_deref(),
        plan_agent_id.as_deref(),
        state_filter.as_deref(),
    )
}

#[tauri::command]
pub async fn update_budget_usage_cmd(
    state: State<'_, AppState>,
    budget_id: String,
    tokens_used: i64,
) -> Result<budget::AgentBudget, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::update_budget_usage(&db, &budget_id, tokens_used)
}

#[tauri::command]
pub async fn capture_wip_cmd(
    state: State<'_, AppState>,
    budget_id: String,
    wip_path: String,
) -> Result<budget::AgentBudget, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::capture_wip(&db, &budget_id, &wip_path)
}

#[tauri::command]
pub async fn resume_budget_cmd(
    state: State<'_, AppState>,
    budget_id: String,
    additional_tokens: i64,
) -> Result<budget::AgentBudget, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::resume_budget(&db, &budget_id, additional_tokens)
}

#[tauri::command]
pub async fn create_resumption_plan_cmd(
    state: State<'_, AppState>,
    wave_id: String,
    pending_task_id: Option<String>,
    plan_path: String,
    agents_completed: Vec<String>,
    agents_wipd: Vec<String>,
    agents_pending: Vec<String>,
    estimated_remaining_tokens: Option<i64>,
) -> Result<budget::WaveResumptionPlan, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::create_resumption_plan(
        &db,
        &wave_id,
        pending_task_id.as_deref(),
        &plan_path,
        &agents_completed,
        &agents_wipd,
        &agents_pending,
        estimated_remaining_tokens,
    )
}

#[tauri::command]
pub async fn get_resumption_plan_cmd(
    state: State<'_, AppState>,
    wave_id: String,
) -> Result<Option<budget::WaveResumptionPlan>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::get_resumption_plan(&db, &wave_id)
}

#[tauri::command]
pub async fn get_cost_breakdown_cmd(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<budget::CostBreakdown>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::get_cost_breakdown(&db, &session_id)
}

#[tauri::command]
pub async fn check_budget_thresholds_cmd(
    state: State<'_, AppState>,
) -> Result<Vec<budget::AgentBudget>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    budget::check_budget_thresholds(&db)
}

// ============================================================================
// Phase 9: Knowledge Layer Commands
// ============================================================================

#[tauri::command]
pub async fn create_knowledge_item_cmd(
    state: State<'_, AppState>,
    item: knowledge::KnowledgeItemInput,
) -> Result<knowledge::KnowledgeItem, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::create_knowledge_item(&db, &item)
}

#[tauri::command]
pub async fn get_knowledge_items_cmd(
    state: State<'_, AppState>,
    q: Option<String>,
    stack: Option<String>,
    agent: Option<String>,
    project_id: Option<String>,
    r#type: Option<String>,
    status: Option<String>,
    min_confidence: Option<f64>,
    is_global: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<knowledge::KnowledgeItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let query = knowledge::KnowledgeQuery {
        q,
        stack,
        agent,
        project_id,
        r#type,
        status,
        min_confidence,
        is_global,
        limit,
        offset,
    };
    knowledge::get_knowledge_items(&db, &query)
}

#[tauri::command]
pub async fn update_knowledge_item_cmd(
    state: State<'_, AppState>,
    id: String,
    updates: knowledge::KnowledgeItemUpdate,
) -> Result<knowledge::KnowledgeItem, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::update_knowledge_item(&db, &id, &updates)
}

#[tauri::command]
pub async fn delete_knowledge_item_cmd(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::delete_knowledge_item(&db, &id)
}

#[tauri::command]
pub async fn compound_knowledge_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<Vec<knowledge::KnowledgeItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::compound_knowledge(&db, project_id.as_deref())
}

#[tauri::command]
pub async fn add_knowledge_relation_cmd(
    state: State<'_, AppState>,
    from_id: String,
    to_id: String,
    relation_type: String,
) -> Result<knowledge::KnowledgeRelation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::add_knowledge_relation(&db, &from_id, &to_id, &relation_type)
}

#[tauri::command]
pub async fn get_knowledge_relations_cmd(
    state: State<'_, AppState>,
    from_id: String,
) -> Result<Vec<knowledge::KnowledgeRelation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::get_knowledge_relations(&db, &from_id)
}

#[tauri::command]
pub async fn search_knowledge_cmd(
    state: State<'_, AppState>,
    q: String,
    limit: Option<i64>,
) -> Result<Vec<knowledge::KnowledgeItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::search_knowledge(&db, &q, limit.unwrap_or(50))
}

#[tauri::command]
pub async fn get_knowledge_stats_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::get_knowledge_stats(&db, project_id.as_deref())
}

#[tauri::command]
pub async fn get_compounder_status_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<knowledge::CompounderStatus, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    knowledge::get_compounder_status(&db, project_id.as_deref())
}

// ============================================================================
// Knowledge Graph Commands
// ============================================================================

#[tauri::command]
pub async fn kg_local_search_cmd(
    state: State<'_, AppState>,
    seed_ids: Vec<String>,
    depth: Option<i64>,
) -> Result<kg_queries::SubgraphResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_queries::bfs_subgraph(&db, &seed_ids, depth.unwrap_or(2))
}

#[tauri::command]
pub async fn kg_global_search_cmd(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<kg_queries::CommunitySearchResult>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_queries::global_search(&db, None, limit.unwrap_or(10))
}

#[tauri::command]
pub async fn kg_get_community_cmd(
    state: State<'_, AppState>,
    community_id: String,
    level: i64,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let summary = kg_core::get_community_summary(&db, &community_id, level)?;
    let items = kg_core::get_community_items(&db, &community_id, level)?;
    Ok(serde_json::json!({
        "summary": summary,
        "item_ids": items,
    }))
}

#[tauri::command]
pub async fn kg_get_subgraph_cmd(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
    depth: Option<i64>,
) -> Result<kg_queries::SubgraphResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_queries::bfs_subgraph(&db, &item_ids, depth.unwrap_or(2))
}

#[tauri::command]
pub async fn kg_get_code_knowledge_cmd(
    state: State<'_, AppState>,
    source_file: String,
) -> Result<Vec<kg_core::CodeKnowledgeJoin>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_core::get_knowledge_for_code(&db, &source_file)
}

#[tauri::command]
pub async fn kg_get_contradictions_cmd(
    state: State<'_, AppState>,
    filter: Option<String>,
) -> Result<Vec<kg_core::KnowledgeContradiction>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_core::get_contradictions(&db, filter.as_deref())
}

#[tauri::command]
pub async fn kg_resolve_contradiction_cmd(
    state: State<'_, AppState>,
    id: String,
    resolution: String,
    resolved_by: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_core::resolve_contradiction(&db, &id, &resolution, &resolved_by)
}

#[tauri::command]
pub async fn kg_merge_items_cmd(
    state: State<'_, AppState>,
    item_a_id: String,
    item_b_id: String,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let item_a = crate::knowledge::get_knowledge_items(
        &db,
        &crate::knowledge::KnowledgeQuery {
            q: None,
            stack: None,
            agent: None,
            project_id: None,
            r#type: None,
            status: None,
            min_confidence: None,
            is_global: None,
            limit: Some(1),
            offset: Some(0),
        },
    )?
    .into_iter()
    .find(|i| i.id == item_a_id)
    .ok_or_else(|| "Item A not found".to_string())?;

    let item_b = crate::knowledge::get_knowledge_items(
        &db,
        &crate::knowledge::KnowledgeQuery {
            q: None,
            stack: None,
            agent: None,
            project_id: None,
            r#type: None,
            status: None,
            min_confidence: None,
            is_global: None,
            limit: Some(1),
            offset: Some(0),
        },
    )?
    .into_iter()
    .find(|i| i.id == item_b_id)
    .ok_or_else(|| "Item B not found".to_string())?;

    let merged_content = format!("{}\n---\n{}", item_a.content, item_b.content);
    let merged_confidence = (item_a.confidence + item_b.confidence) / 2.0;
    let merged_count = item_a.confirmation_count + item_b.confirmation_count;

    crate::knowledge::update_knowledge_item(
        &db,
        &item_a_id,
        &crate::knowledge::KnowledgeItemUpdate {
            title: Some(format!("{} (merged with {})", item_a.title, item_b.title)),
            content: Some(merged_content),
            tags: None,
            stack_tags: None,
            agent_tags: None,
            confidence: Some(merged_confidence),
            status: None,
        },
    )?;

    crate::knowledge::delete_knowledge_item(&db, &item_b_id)?;

    Ok(serde_json::json!({
        "merged_item_id": item_a_id,
        "deleted_item_id": item_b_id,
        "new_confidence": merged_confidence,
        "new_confirmation_count": merged_count,
    }))
}

#[tauri::command]
pub async fn kg_run_community_detection_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let items = crate::knowledge::get_knowledge_items(
        &db,
        &crate::knowledge::KnowledgeQuery {
            q: None,
            stack: None,
            agent: None,
            project_id,
            r#type: None,
            status: Some("active".to_string()),
            min_confidence: None,
            is_global: None,
            limit: Some(5000),
            offset: Some(0),
        },
    )?;

    let mut communities: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for item in &items {
        let tags: Vec<&str> = item
            .stack_tags
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let community_key = if tags.is_empty() {
            item.r#type.clone()
        } else {
            tags.join("-")
        };

        communities
            .entry(community_key)
            .or_default()
            .push(item.id.clone());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut community_count = 0usize;

    for (community_key, item_ids) in &communities {
        for (level, items_subset) in [item_ids].iter().enumerate() {
            for item_id in *items_subset {
                if kg_core::assign_item_to_community(
                    &db,
                    item_id,
                    community_key,
                    level as i64,
                ).is_ok() {
                    community_count += 1;
                }
            }
        }
    }

    Ok(serde_json::json!({
        "communities_found": communities.len(),
        "assignments_made": community_count,
        "total_items_processed": items.len(),
    }))
}

#[tauri::command]
pub async fn kg_mine_git_cochanges_cmd(
    state: State<'_, AppState>,
    repo_path: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let count = kg_git::mine_cochange_patterns(&repo_path, project_id.as_deref(), &db)?;
    Ok(serde_json::json!({
        "cochange_pairs_stored": count,
    }))
}

#[tauri::command]
pub async fn kg_get_cochange_warnings_cmd(
    state: State<'_, AppState>,
    file_path: String,
    min_jaccard: Option<f64>,
) -> Result<Vec<kg_git::CochangeWarning>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    kg_git::get_cochange_warnings(&db, &file_path, min_jaccard.unwrap_or(0.3))
}

// ============================================================================
// Phase 10+: Control Sessions & Cost Aggregation
// ============================================================================

#[tauri::command]
pub async fn claim_file_cmd(
    state: State<'_, AppState>,
    project_id: String,
    file_path: String,
    claimed_by_thread_id: String,
) -> Result<control::FileOwnership, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::claim_file(&db, &project_id, &file_path, &claimed_by_thread_id)
}

#[tauri::command]
pub async fn release_file_cmd(
    state: State<'_, AppState>,
    project_id: String,
    file_path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::release_file(&db, &project_id, &file_path)
}

#[tauri::command]
pub async fn release_all_for_thread_cmd(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::release_all_for_thread(&db, &thread_id)
}

#[tauri::command]
pub async fn get_owned_files_cmd(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<Vec<control::FileOwnership>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::get_owned_files(&db, &thread_id)
}

#[tauri::command]
pub async fn get_project_locks_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<control::FileOwnership>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::get_project_locks(&db, &project_id)
}

#[tauri::command]
pub async fn detect_conflicts_cmd(
    state: State<'_, AppState>,
    project_id: String,
    file_paths: Vec<String>,
    requesting_thread: String,
) -> Result<Vec<control::ConflictReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::detect_conflicts(&db, &project_id, &file_paths, &requesting_thread)
}

#[tauri::command]
pub async fn get_cost_summary_cmd(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<control::CostSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    control::get_cost_summary(&db, project_id.as_deref())
}

// ============================================================================
// Backward Channel Commands (Phase 8: Chat → Local LLM)
// ============================================================================

#[tauri::command]
pub async fn get_chat_platform_configs_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<backward_channel::ChatPlatformConfig>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    backward_channel::get_chat_platform_configs(&db, &project_id)
}

#[tauri::command]
pub async fn save_chat_platform_config_cmd(
    state: State<'_, AppState>,
    config: backward_channel::ChatPlatformConfig,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    backward_channel::save_chat_platform_config(&db, &config)
}

#[tauri::command]
pub async fn delete_chat_platform_config_cmd(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    backward_channel::delete_chat_platform_config(&db, &id)
}

#[tauri::command]
pub async fn toggle_chat_platform_config_cmd(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    backward_channel::toggle_chat_platform_config(&db, &id, enabled)
}

#[tauri::command]
pub async fn start_backward_channel_daemon_cmd(
    config_path: Option<String>,
) -> Result<String, String> {
    let path = config_path.unwrap_or_else(|| "./agent_registry.yaml".to_string());
    let child = Command::new("python3")
        .arg("local-daemon/main.py")
        .arg("--config")
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start daemon: {}", e))?;

    let pid = child.id();
    fs::write("/tmp/acc-backward-daemon.pid", pid.to_string())
        .map_err(|e| format!("Failed to write PID file: {}", e))?;

    Ok(format!("Daemon started with PID {}", pid))
}

#[tauri::command]
pub async fn stop_backward_channel_daemon_cmd() -> Result<(), String> {
    let pid_str = fs::read_to_string("/tmp/acc-backward-daemon.pid")
        .map_err(|_| "Daemon not running (no PID file)".to_string())?;
    let pid: u32 = pid_str.trim().parse()
        .map_err(|_| "Invalid PID file".to_string())?;

    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    }
    #[cfg(not(unix))]
    {
        Command::new("taskkill").args(["/PID", &pid.to_string()]).spawn()
            .map_err(|e| format!("Failed to kill: {}", e))?;
    }

    fs::remove_file("/tmp/acc-backward-daemon.pid").ok();
    Ok(())
}

#[tauri::command]
pub async fn get_backward_channel_daemon_status_cmd() -> Result<serde_json::Value, String> {
    let pid_str = match fs::read_to_string("/tmp/acc-backward-daemon.pid") {
        Ok(s) => s,
        Err(_) => return Ok(serde_json::json!({
            "running": false, "pid": null, "uptime_s": null,
            "queue_depth": 0, "active_platforms": [],
            "last_event_at": null, "error": null
        })),
    };

    let pid: i32 = pid_str.trim().parse().unwrap_or(0);

    #[cfg(unix)]
    let running = unsafe { libc::kill(pid, 0) == 0 };
    #[cfg(not(unix))]
    let running = true;

    Ok(serde_json::json!({
        "running": running,
        "pid": if running { Some(pid) } else { None::<i32> },
        "uptime_s": null,
        "queue_depth": 0,
        "active_platforms": [],
        "last_event_at": null,
        "error": if !running { Some("Process not running") } else { None::<&str> }
    }))
}

#[tauri::command]
pub async fn get_backward_channel_daemon_logs_cmd(
    lines: Option<i64>,
) -> Result<Vec<String>, String> {
    let n = lines.unwrap_or(50) as usize;
    match fs::read_to_string("/tmp/acc-backward-daemon.log") {
        Ok(log) => {
            let all: Vec<&str> = log.lines().collect();
            let start = if all.len() > n { all.len() - n } else { 0 };
            Ok(all[start..].iter().map(|s| s.to_string()).collect())
        }
        Err(_) => Ok(vec!["No log file found".to_string()]),
    }
}

#[tauri::command]
pub async fn check_backward_channel_queue_health_cmd() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "provider": "upstash",
        "connected": false,
        "queue_depth": 0,
        "latency_ms": null
    }))
}

#[tauri::command]
pub async fn test_chat_platform_connection_cmd(
    platform: String,
    config: HashMap<String, String>,
) -> Result<bool, String> {
    match platform.as_str() {
        "lark" => Ok(config.contains_key("app_id") && config.contains_key("app_secret")),
        "slack" => Ok(config.contains_key("signing_secret") && config.contains_key("bot_token")),
        "discord" => Ok(config.contains_key("public_key") && config.contains_key("bot_token")),
        "telegram" => Ok(config.contains_key("bot_token")),
        _ => Err(format!("Unknown platform: {}", platform)),
    }
}

// ============================================================================
// Phase 10: Codebase Exploration Commands
// ============================================================================

#[tauri::command]
pub async fn build_repo_map_cmd(
    state: State<'_, AppState>,
    project_id: String,
    project_path: String,
    config: codebase::RepoMapConfig,
) -> Result<Vec<codebase::RepoMapOutput>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    codebase::build_repo_map(&db, &project_id, &project_path, &config)
}

#[tauri::command]
pub async fn get_repo_map_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<codebase::RepoMapOutput>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    codebase::get_repo_map(&db, &project_id)
}

#[tauri::command]
pub async fn search_codebase_cmd(
    state: State<'_, AppState>,
    project_id: String,
    query: String,
    top_k: Option<i64>,
) -> Result<Vec<codebase::SearchResult>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    codebase::search_codebase(&db, &project_id, &query, top_k.unwrap_or(10))
}

#[tauri::command]
pub async fn get_file_signatures_cmd(
    state: State<'_, AppState>,
    project_id: String,
    file_path: String,
    level: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let file_id: String = db.query_row(
        "SELECT id FROM codebase_files WHERE project_id = ?1 AND file_path = ?2",
        rusqlite::params![project_id, file_path],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    codebase::get_file_signature_ladder(&db, &file_id, &level)
}

#[tauri::command]
pub async fn get_coverage_stats_cmd(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<codebase::CodebaseCoverage>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    codebase::get_coverage_stats(&db, &project_id).map(Some)
}

#[tauri::command]
pub async fn invalidate_cache_cmd(
    state: State<'_, AppState>,
    project_id: String,
    file_path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    codebase::invalidate_cache(&db, &project_id, &file_path)
}

// ============================================================================
// Phase 10: Memory Layer Commands
// ============================================================================

#[tauri::command]
pub async fn create_memory_fact_cmd(
    state: State<'_, AppState>,
    input: memory::MemoryFactInput,
) -> Result<memory::MemoryFact, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::create_memory_fact(&db, &input)
}

#[tauri::command]
pub async fn get_memory_fact_cmd(
    state: State<'_, AppState>,
    id: String,
) -> Result<memory::MemoryFact, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::get_memory_fact(&db, &id)
}

#[tauri::command]
pub async fn get_memory_facts_cmd(
    state: State<'_, AppState>,
    agent_id: Option<String>,
    session_id: Option<String>,
    org_id: Option<String>,
    fact_type: Option<String>,
    min_confidence: Option<f64>,
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<memory::MemoryFact>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let query = memory::MemoryQuery {
        agent_id,
        session_id,
        org_id,
        fact_type,
        min_confidence,
        q,
        limit,
        offset,
    };
    memory::get_memory_facts(&db, &query)
}

#[tauri::command]
pub async fn update_memory_fact_cmd(
    state: State<'_, AppState>,
    id: String,
    content: Option<String>,
    confidence: Option<f64>,
    metadata: Option<String>,
) -> Result<memory::MemoryFact, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::update_memory_fact(&db, &id, content.as_deref(), confidence, metadata.as_deref())
}

#[tauri::command]
pub async fn delete_memory_fact_cmd(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::delete_memory_fact(&db, &id)
}

#[tauri::command]
pub async fn memory_hybrid_search_cmd(
    state: State<'_, AppState>,
    agent_id: Option<String>,
    session_id: Option<String>,
    org_id: Option<String>,
    q: String,
    limit: Option<i64>,
) -> Result<Vec<memory::SearchResult>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let query = memory::MemoryQuery {
        agent_id,
        session_id,
        org_id,
        fact_type: None,
        min_confidence: None,
        q: None,
        limit: None,
        offset: None,
    };
    memory::hybrid_search(&db, &query, &q, limit.unwrap_or(10))
}

#[tauri::command]
pub async fn memory_get_context_cmd(
    state: State<'_, AppState>,
    agent_id: String,
    session_id: String,
    query: Option<String>,
    budget: Option<i64>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::get_context(&db, &agent_id, &session_id, query.as_deref(), budget)
}

#[tauri::command]
pub async fn create_checkpoint_cmd(
    state: State<'_, AppState>,
    agent_id: String,
    session_id: String,
    turn_number: i64,
    state_blob: Vec<u8>,
    summary: Option<String>,
    token_count: Option<i64>,
) -> Result<memory::SessionCheckpoint, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::create_session_checkpoint(&db, &agent_id, &session_id, turn_number, &state_blob, summary.as_deref(), token_count)
}

#[tauri::command]
pub async fn get_latest_checkpoint_cmd(
    state: State<'_, AppState>,
    agent_id: String,
    session_id: String,
) -> Result<Option<memory::SessionCheckpoint>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::get_latest_checkpoint(&db, &agent_id, &session_id)
}

#[tauri::command]
pub async fn memory_stats_cmd(
    state: State<'_, AppState>,
    org_id: String,
) -> Result<memory::MemoryStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    memory::get_memory_stats(&db, &org_id)
}

// ============================================================================
// Agent Install Check Commands
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformInstallHints {
    pub windows: String,
    pub macos: String,
    pub linux: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentInstallStatus {
    pub installed: bool,
    pub command: String,
    pub install_hint: Option<String>,
    pub platform_hints: PlatformInstallHints,
}

fn get_install_hints(agent_id: &str, command: &str) -> Option<String> {
    let lower = agent_id.to_lowercase();
    if lower.contains("claude") {
        Some("Run: npm install -g @anthropic-ai/claude-code".into())
    } else if lower.contains("opencode") {
        Some("Run: npm install -g @anomalyco/opencode".into())
    } else if lower.contains("aider") {
        Some("Run: pip install aider-chat".into())
    } else if lower.contains("goose") {
        Some("Run: npm install -g @goose-ai/cli".into())
    } else if lower.contains("gemini") || lower.contains("google") {
        Some("Run: npm install -g @anthropic-ai/claude-code".into())
    } else if lower.contains("codex") || lower.contains("openai") {
        Some("Run: npm install -g @openai/codex".into())
    } else {
        None
    }
}

fn get_platform_hints(agent_id: &str, command: &str) -> PlatformInstallHints {
    let lower = agent_id.to_lowercase();
    if lower.contains("claude") {
        PlatformInstallHints {
            windows: "npm install -g @anthropic-ai/claude-code".into(),
            macos: "brew install claude-code || npm install -g @anthropic-ai/claude-code".into(),
            linux: "npm install -g @anthropic-ai/claude-code".into(),
        }
    } else if lower.contains("opencode") {
        PlatformInstallHints {
            windows: "npm install -g @anomalyco/opencode".into(),
            macos: "brew install opencode || npm install -g @anomalyco/opencode".into(),
            linux: "npm install -g @anomalyco/opencode".into(),
        }
    } else if lower.contains("aider") {
        PlatformInstallHints {
            windows: "pip install aider-chat".into(),
            macos: "brew install aider || pip install aider-chat".into(),
            linux: "pip install aider-chat".into(),
        }
    } else if lower.contains("goose") {
        PlatformInstallHints {
            windows: "npm install -g @goose-ai/cli".into(),
            macos: "npm install -g @goose-ai/cli".into(),
            linux: "npm install -g @goose-ai/cli".into(),
        }
    } else if lower.contains("gemini") || lower.contains("google") {
        PlatformInstallHints {
            windows: "npm install -g @google/generative-ai".into(),
            macos: "npm install -g @google/generative-ai".into(),
            linux: "npm install -g @google/generative-ai".into(),
        }
    } else if lower.contains("codex") || lower.contains("openai") {
        PlatformInstallHints {
            windows: "npm install -g @openai/codex".into(),
            macos: "npm install -g @openai/codex".into(),
            linux: "npm install -g @openai/codex".into(),
        }
    } else {
        PlatformInstallHints {
            windows: format!("Check if '{}' is installed and available in PATH", command),
            macos: format!("Check if '{}' is installed and available in PATH", command),
            linux: format!("Check if '{}' is installed and available in PATH", command),
        }
    }
}

#[tauri::command]
pub async fn check_agent_installed(
    agent_id: String,
    command: String,
) -> Result<AgentInstallStatus, String> {
    let (check_cmd, check_arg) = if cfg!(target_os = "windows") {
        ("where", command.clone())
    } else {
        ("which", command.clone())
    };

    let installed = std::process::Command::new(check_cmd)
        .arg(&check_arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let install_hint = if installed {
        None
    } else {
        get_install_hints(&agent_id, &command)
    };

    let platform_hints = get_platform_hints(&agent_id, &command);

    Ok(AgentInstallStatus {
        installed,
        command,
        install_hint,
        platform_hints,
    })
}

// ============================================================================
// GA-Bagua Semantic KG MCP Commands
// ============================================================================

#[tauri::command]
pub async fn detect_bagua_mcp_cmd() -> Result<assets::McpInstallStatus, String> {
    Ok(assets::detect_bagua_mcp())
}

#[tauri::command]
pub async fn test_bagua_mcp_connection_cmd() -> Result<assets::McpConnectionTest, String> {
    Ok(assets::test_bagua_mcp_connection())
}

// ============================================================================
// Crash Recovery: State Persistence
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub active_agents: Vec<ActiveAgentSnapshot>,
    pub last_project_path: Option<String>,
    pub saved_at: String,
}

pub(crate) async fn save_state_inner(state: &AppState) -> Result<(), String> {
    let agents = state.pty_manager.snapshot_active_agents().await;
    let project_path = state
        .current_project_path
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let saved_at = chrono::Utc::now().to_rfc3339();
    let agents_json = serde_json::to_string(&agents).map_err(|e| e.to_string())?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE app_state_snapshot SET active_agents = ?1, last_project_path = ?2, saved_at = ?3 WHERE id = 1",
        rusqlite::params![agents_json, project_path, saved_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn save_app_state(
    state: State<'_, AppState>,
) -> Result<(), String> {
    save_state_inner(&state).await
}

#[tauri::command]
pub async fn load_app_state(
    state: State<'_, AppState>,
) -> Result<AppStateSnapshot, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (agents_json, project_path, saved_at): (String, Option<String>, String) = db
        .query_row(
            "SELECT active_agents, last_project_path, saved_at FROM app_state_snapshot WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let active_agents: Vec<ActiveAgentSnapshot> =
        serde_json::from_str(&agents_json).unwrap_or_default();

    Ok(AppStateSnapshot {
        active_agents,
        last_project_path: project_path,
        saved_at,
    })
}

#[tauri::command]
pub async fn set_project_path(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut current = state.current_project_path.lock().map_err(|e| e.to_string())?;
        *current = Some(path);
    }
    let _ = save_state_inner(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn clear_app_state(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE app_state_snapshot SET active_agents = '[]', last_project_path = NULL, saved_at = ?1 WHERE id = 1",
        rusqlite::params![chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// Bagua Semantic KG Commands
// ============================================================================

#[tauri::command]
pub async fn kg_encode_concept(coeffs: Vec<f64>) -> Result<BaguaEncoding, String> {
    let arr: [f64; 8] = coeffs.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    encode_concept(&arr)
}

#[tauri::command]
pub async fn kg_classify_relation(coeffs_a: Vec<f64>, coeffs_b: Vec<f64>) -> Result<BaguaRelation, String> {
    let a: [f64; 8] = coeffs_a.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    let b: [f64; 8] = coeffs_b.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    classify_relationship(&a, &b)
}

#[tauri::command]
pub async fn kg_bagua_similarity(coeffs_a: Vec<f64>, coeffs_b: Vec<f64>) -> Result<f64, String> {
    let a: [f64; 8] = coeffs_a.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    let b: [f64; 8] = coeffs_b.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    Ok(bagua_similarity(&a, &b))
}

#[tauri::command]
pub async fn kg_solve_analogy(coeffs_a: Vec<f64>, coeffs_b: Vec<f64>, coeffs_c: Vec<f64>) -> Result<Vec<f64>, String> {
    let a: [f64; 8] = coeffs_a.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    let b: [f64; 8] = coeffs_b.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    let c: [f64; 8] = coeffs_c.try_into().map_err(|_| "Need exactly 8 coefficients".to_string())?;
    let result = solve_analogy(&a, &b, &c)
        .ok_or_else(|| "Analogy could not be solved — no valid WuXing cycle found between the concepts".to_string())?;
    Ok(result.to_vec())
}
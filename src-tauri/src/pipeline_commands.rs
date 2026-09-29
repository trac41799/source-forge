// src-tauri/src/pipeline_commands.rs
//
// Tauri command surface for the supervised build pipeline (Step 3.7).
//
// The pipeline itself is synchronous and runs on a blocking thread
// (`spawn_blocking`) so the async runtime is never blocked; the shared DB is
// an `Arc<Mutex<Connection>>` clone. Progress is emitted as
// `build-app-progress` events; the frontend can also poll
// `get_build_app_status_cmd`.

use tauri::{AppHandle, Emitter, State};

use crate::commands::AppState;
use crate::pipeline::{
    BuildReport, EventSink, PipelineAdapters, PipelineOptions, EVENT_NAME,
};
use crate::pipeline_store::{self, BuildRun};

/// Emits pipeline progress to the frontend.
pub struct TauriEventSink {
    pub app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit(&self, stage: &str, status: &str, message: &str) {
        let payload = serde_json::json!({
            "stage": stage,
            "status": status,
            "message": message,
        });
        let _ = self.app.emit(EVENT_NAME, payload);
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BuildAppOptions {
    pub project_id: Option<String>,
    pub spec_path: String,
    pub project_path: String,
    pub stack_id: Option<String>,
    pub agent_command: Option<String>,
    pub base_branch: Option<String>,
    pub allow_deploy_on_failed_verification: Option<bool>,
    pub generate_dockerfile: Option<bool>,
    pub agent_timeout_secs: Option<u64>,
}

impl BuildAppOptions {
    fn into_pipeline_options(self, run_id: &str) -> PipelineOptions {
        PipelineOptions {
            run_id: run_id.to_string(),
            project_id: self.project_id,
            spec_path: self.spec_path,
            project_path: self.project_path,
            stack_id: self.stack_id,
            agent_command: self.agent_command.unwrap_or_else(|| "opencode".to_string()),
            base_branch: self.base_branch.unwrap_or_else(|| "main".to_string()),
            allow_deploy_on_failed_verification: self
                .allow_deploy_on_failed_verification
                .unwrap_or(false),
            generate_dockerfile: self.generate_dockerfile.unwrap_or(true),
            agent_timeout_secs: self.agent_timeout_secs.unwrap_or(300),
        }
    }
}

async fn run_on_blocking_thread(
    app: AppHandle,
    db: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    opts: PipelineOptions,
) -> Result<BuildReport, String> {
    let report = tauri::async_runtime::spawn_blocking(move || {
        let deployer = crate::deployer::VercelDeployer;
        let registry = crate::agent_adapters::AdapterRegistry::new();
        let wave_runner = crate::pipeline::AdapterWaveRunner {
            registry,
            agent_command: opts.agent_command.clone(),
            base_branch: opts.base_branch.clone(),
            deadline_secs: None,
            cost_cap_usd: None,
        };
        let sink = TauriEventSink { app };
        let llm = crate::compounder_llm::LlmProvider::OpenRouter;

        let adapters = PipelineAdapters {
            deployer: &deployer,
            wave_runner: &wave_runner,
            event_sink: &sink,
            llm: &llm,
            cli_status: None,
            run_compounder: true,
        };

        crate::pipeline::run_pipeline(&db, &opts, &adapters)
    })
    .await
    .map_err(|e| format!("pipeline task failed: {e}"))?;

    report
}

#[tauri::command]
pub async fn build_app_cmd(
    state: State<'_, AppState>,
    app: AppHandle,
    options: BuildAppOptions,
) -> Result<BuildReport, String> {
    // Resume a non-terminal run for the same project path, or create a new one.
    let run: BuildRun = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        match pipeline_store::claim_startable_run(&db, &options.project_path)? {
            Some(existing) => existing,
            None => pipeline_store::create_run(
                &db,
                options.project_id.as_deref(),
                &options.spec_path,
                &options.project_path,
                options.stack_id.as_deref(),
            )?,
        }
    };

    let opts = options.into_pipeline_options(&run.id);
    run_on_blocking_thread(app, state.db.clone(), opts).await
}

#[tauri::command]
pub async fn resume_build_app_cmd(
    state: State<'_, AppState>,
    app: AppHandle,
    run_id: String,
) -> Result<BuildReport, String> {
    let run = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        pipeline_store::get_run(&db, &run_id)?
    };

    if pipeline_store::TERMINAL_STATUSES.contains(&run.status.as_str()) {
        return Err(format!("Run {run_id} is already {}", run.status));
    }

    // Reuse the options the run was started with (persisted in the report's
    // artifacts) so a resume behaves the same instead of silently reverting to
    // defaults — e.g. losing a pinned agent or a raised timeout (M12).
    let artifacts = run
        .report
        .as_ref()
        .and_then(|report| report.get("artifacts"))
        .cloned()
        .unwrap_or_default();
    let str_opt = |key: &str, fallback: &str| {
        artifacts
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or(fallback)
            .to_string()
    };

    let opts = PipelineOptions {
        run_id: run.id.clone(),
        project_id: run.project_id.clone(),
        spec_path: run.spec_path.clone(),
        project_path: run.project_path.clone(),
        stack_id: run.stack_id.clone(),
        agent_command: str_opt("agent_command", "opencode"),
        base_branch: str_opt("base_branch", "main"),
        allow_deploy_on_failed_verification: artifacts
            .get("allow_deploy_on_failed_verification")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        generate_dockerfile: artifacts
            .get("generate_dockerfile")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        agent_timeout_secs: artifacts
            .get("agent_timeout_secs")
            .and_then(|value| value.as_u64())
            .unwrap_or(300),
    };

    run_on_blocking_thread(app, state.db.clone(), opts).await
}

#[tauri::command]
pub async fn cancel_build_app_cmd(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pipeline_store::cancel_run(&db, &run_id)
}

#[tauri::command]
pub async fn get_build_app_status_cmd(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<BuildRun, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pipeline_store::get_run(&db, &run_id)
}

/// The run currently in flight (or paused awaiting the user) for a project path.
///
/// `build_app_cmd` only returns when the pipeline finishes, so the UI cannot
/// learn the `run_id` in time to offer Cancel/Resume. This exposes the active
/// run so a long build can be cancelled or retried from the app.
#[tauri::command]
pub async fn get_active_build_run_cmd(
    state: State<'_, AppState>,
    project_path: String,
) -> Result<Option<BuildRun>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pipeline_store::find_resumable_run(&db, &project_path)
}

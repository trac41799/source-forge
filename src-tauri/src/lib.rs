pub mod acb;
mod agent_adapters;
mod agent_events;
mod assets;
pub mod budget;
mod commands;
pub mod control;
pub mod db;
pub mod events;
mod integrations;
pub mod intelligence;
pub mod knowledge;
mod knowledge_commands;
mod compounder_llm;
mod pipeline_store;
mod deployer;
mod pipeline;
mod pipeline_commands;
mod wave_supervisor;
pub mod orchestrator;
mod playbook;
mod pty;
pub mod routing;
mod scheduler;
mod skillbridge;
mod backward_channel;
mod memory;
mod kg_core;
mod kg_extraction;
mod kg_queries;
mod kg_git;
mod kg_code;
mod codebase;
mod worktree;
mod guideline_spawn;
mod pty_guards;
mod handoff_parser;
mod wave_executor;
mod spec_parser;
mod verification;
mod wave_persistence;
mod integration_tests;
mod stack_registry;
mod preferences;
mod provisioner;
mod arch_engine;
mod delegation;
#[allow(dead_code)] // M0: consumed by tests; wired into commands in M1
mod decision;

use commands::AppState;
use log::info;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let conn = db::init_db(app).expect("Failed to initialize database");
            let app_state = AppState::new(conn);
            let pty_arc_for_cron = app_state.pty_manager.clone();
            app.manage(app_state);
            events::set_app_handle(app.handle().clone());
            info!("SourceForge starting...");

            // === W5.B: Cron Engine auto-start ===
            let app_handle_for_cron = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Manager;
                let app_data_dir = match app_handle_for_cron.path().app_data_dir() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[cron auto-start] app_data_dir error: {e}");
                        return;
                    }
                };
                let db_path = app_data_dir.join("acc.db");
                match crate::scheduler::start_cron_engine(db_path, pty_arc_for_cron).await {
                    Ok(_engine) => {
                        info!("Cron engine auto-started");
                    }
                    Err(e) => eprintln!("[cron] failed to start: {e}"),
                }
            });
            info!("Cron engine auto-start dispatched");

            let window = app.get_webview_window("main").unwrap();
            window.set_title("SourceForge").unwrap();
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app_handle = _window.app_handle().clone();
                std::thread::spawn(move || {
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        handle.block_on(async {
                            let state = app_handle.state::<AppState>();
                            let _ = crate::commands::save_state_inner(&state).await;
                        });
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::spawn_agent,
            commands::spawn_agent_with_guards,
            commands::kill_agent,
            commands::write_to_agent,
            commands::list_agents,
            commands::get_agent_output,
            commands::create_worktree_cmd,
            commands::remove_worktree_cmd,
            commands::list_worktrees_cmd,
            commands::parse_handoff_file_cmd,
            commands::execute_wave_cmd,
            commands::finalize_wave_cmd,
            commands::seed_wave_from_spec_cmd,
            commands::verify_project_cmd,
            commands::verify_and_finalize_wave_cmd,
            commands::generate_deploy_config_cmd,
            commands::list_available_stacks_cmd,
            commands::detect_installed_clis_cmd,
            commands::recommend_stack_cmd,
            commands::missing_clis_for_stack_cmd,
            commands::get_preferences_cmd,
            commands::set_preferences_cmd,
            commands::get_decision_config_cmd,
            commands::set_decision_config_cmd,
            commands::decision_health_cmd,
            commands::list_decision_reviews_cmd,
            commands::resolve_decision_review_cmd,
            commands::provision_stack_cmd,
            commands::check_cli_cmd,
            commands::install_cli_cmd,
            commands::scaffold_project_cmd,
            commands::create_delegation_report_cmd,
            commands::log_event,
            commands::log_event_with_payload,
            commands::get_events,
            commands::get_event_detail,
            commands::get_all_sessions_cmd,
            commands::check_skillbridge,
            // Assets (Phase 2)
            commands::scan_skills,
            commands::read_skill,
            commands::scan_memory,
            commands::write_memory,
            commands::list_mcps,
            commands::toggle_mcp,
            commands::store_secret,
            commands::list_secrets,
            commands::get_secret_value,
            commands::list_plugins,
            commands::generate_profile,
            // Integrations (Phase 7)
            commands::get_supabase_configs,
            commands::save_supabase_config,
            commands::toggle_supabase_feature,
            commands::detect_supabase,
            commands::get_github_configs,
            commands::save_github_config,
            commands::toggle_github_feature,
            commands::detect_github_repo_cmd,
            commands::check_repo_visibility_cmd,
            commands::list_github_issues_cmd,
            commands::check_migration_safety_cmd,
            commands::check_github_actions_cmd,
            // Intelligence (Phase 3)
            commands::record_outcome_cmd,
            commands::get_outcome_stats_cmd,
            commands::create_failure_analysis_cmd,
            commands::get_failure_analyses_cmd,
            commands::detect_limit_event_cmd,
            commands::record_limit_event_cmd,
            commands::resolve_limit_event_cmd,
            commands::get_unresolved_limits_cmd,
            commands::record_token_usage_cmd,
            commands::get_token_usage_stats_cmd,
            commands::run_heartbeat_check_cmd,
            // Phase 4: Routing
            commands::route_task_cmd,
            commands::get_models_cmd,
            commands::add_model_cmd,
            commands::toggle_model_cmd,
            commands::build_handoff_cmd,
            commands::check_agent_version_cmd,
            // Phase 5: Wave Orchestrator
            commands::create_wave_plan_cmd,
            commands::add_plan_agent_cmd,
            commands::get_plan_agents_cmd,
            commands::update_plan_agent_status_cmd,
            commands::generate_guideline_cmd,
            commands::validate_handoff_schema_cmd,
            commands::create_correction_cmd,
            commands::get_corrections_cmd,
            // Phase 5+: ACB
            commands::parse_acb_signal_cmd,
            commands::record_acb_signal_cmd,
            commands::get_open_signals_cmd,
            commands::resolve_signal_cmd,
            // Phase 6: Team Layer
            commands::detect_memory_candidate_cmd,
            commands::create_memory_candidate_cmd,
            commands::get_memory_candidates_cmd,
            commands::build_playbook_manifest_cmd,
            commands::build_feature_doc_prompt_cmd,
            // Subagent Orchestration
            commands::decide_orchestration_path_cmd,
            commands::detect_subagent_spawn_cmd,
            commands::get_subagent_patterns_cmd,
            commands::detect_subagent_activity_cmd,
            commands::record_subagent_token_usage_cmd,
            commands::generate_orchestrator_guideline_cmd,
            // Phase 9+: Autonomous Scheduler
            commands::create_cron_job_cmd,
            commands::get_cron_jobs_cmd,
            commands::update_cron_job_cmd,
            commands::delete_cron_job_cmd,
            commands::toggle_cron_job_cmd,
            commands::evaluate_cron_schedule_cmd,
            commands::get_cron_executions_cmd,
            commands::record_cron_execution_cmd,
            commands::update_cron_execution_cmd,
            // Phase 9: Knowledge Layer
            commands::create_knowledge_item_cmd,
            commands::get_knowledge_items_cmd,
            commands::update_knowledge_item_cmd,
            commands::delete_knowledge_item_cmd,
            commands::compound_knowledge_cmd,
            commands::add_knowledge_relation_cmd,
            commands::get_knowledge_relations_cmd,
            commands::search_knowledge_cmd,
            commands::get_knowledge_stats_cmd,
            commands::get_compounder_status_cmd,
            knowledge_commands::run_compounder_cmd,
            knowledge_commands::get_preflight_warnings_cmd,
            pipeline_commands::build_app_cmd,
            pipeline_commands::resume_build_app_cmd,
            pipeline_commands::cancel_build_app_cmd,
            pipeline_commands::get_build_app_status_cmd,
            // Knowledge Graph Commands
            commands::kg_local_search_cmd,
            commands::kg_global_search_cmd,
            commands::kg_get_community_cmd,
            commands::kg_get_subgraph_cmd,
            commands::kg_get_code_knowledge_cmd,
            commands::kg_get_contradictions_cmd,
            commands::kg_resolve_contradiction_cmd,
            commands::kg_merge_items_cmd,
            commands::kg_run_community_detection_cmd,
            commands::kg_mine_git_cochanges_cmd,
            commands::kg_get_cochange_warnings_cmd,
            // Bagua Semantic KG Commands
            commands::kg_encode_concept,
            commands::kg_classify_relation,
            commands::kg_bagua_similarity,
            commands::kg_solve_analogy,
            // Phase 9++: Token Budget System
            commands::create_budget_cmd,
            commands::get_budgets_cmd,
            commands::update_budget_usage_cmd,
            commands::capture_wip_cmd,
            commands::resume_budget_cmd,
            commands::create_resumption_plan_cmd,
            commands::get_resumption_plan_cmd,
            commands::get_cost_breakdown_cmd,
            commands::check_budget_thresholds_cmd,
            // Phase 10+: Control Sessions & Cost Aggregation
            commands::claim_file_cmd,
            commands::release_file_cmd,
            commands::release_all_for_thread_cmd,
            commands::get_owned_files_cmd,
            commands::get_project_locks_cmd,
            commands::detect_conflicts_cmd,
            commands::get_cost_summary_cmd,
            // Backward Channel (Phase 8: Chat → Local LLM)
            commands::get_chat_platform_configs_cmd,
            commands::save_chat_platform_config_cmd,
            commands::delete_chat_platform_config_cmd,
            commands::toggle_chat_platform_config_cmd,
            commands::start_backward_channel_daemon_cmd,
            commands::stop_backward_channel_daemon_cmd,
            commands::get_backward_channel_daemon_status_cmd,
            commands::get_backward_channel_daemon_logs_cmd,
            commands::check_backward_channel_queue_health_cmd,
            commands::test_chat_platform_connection_cmd,
            // Phase 10: Codebase Exploration
            commands::build_repo_map_cmd,
            commands::get_repo_map_cmd,
            commands::search_codebase_cmd,
            commands::get_file_signatures_cmd,
            commands::get_coverage_stats_cmd,
            commands::invalidate_cache_cmd,
            // Phase 10: Memory Layer
            commands::create_memory_fact_cmd,
            commands::get_memory_fact_cmd,
            commands::get_memory_facts_cmd,
            commands::update_memory_fact_cmd,
            commands::delete_memory_fact_cmd,
            commands::memory_hybrid_search_cmd,
            commands::memory_get_context_cmd,
            commands::create_checkpoint_cmd,
            commands::get_latest_checkpoint_cmd,
            commands::memory_stats_cmd,
            // Agent install check
            commands::check_agent_installed,
            // State persistence (crash recovery)
            commands::save_app_state,
            commands::load_app_state,
            commands::set_project_path,
            commands::clear_app_state,
            // GA-Bagua Semantic KG MCP
            commands::detect_bagua_mcp_cmd,
            commands::test_bagua_mcp_connection_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
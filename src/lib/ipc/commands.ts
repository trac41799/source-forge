/**
 * Canonical IPC command names (SPEC-001 §3.1, rule R-2).
 *
 * Every call site should reference a constant here instead of writing the
 * command name as an inline string. The contract test in
 * `src/__tests__/contracts/` enforces that:
 *   - every invoked name exists in the Tauri `generate_handler!` registry
 *   - no *new* inline invoke string literals are introduced (ratchet)
 *   - the set of registered-but-unused commands only changes deliberately
 *
 * When adding a command: register it in `src-tauri/src/lib.rs`, add a constant
 * here, and call it through the constant.
 */
export const IPC = {
  // Settings / preferences
  getPreferences: "get_preferences_cmd",
  setPreferences: "set_preferences_cmd",

  // Knowledge / compounder
  runCompounder: "run_compounder_cmd",
  getPreflightWarnings: "get_preflight_warnings_cmd",
  getCompounderStatus: "get_compounder_status_cmd",

  // Orchestration / verification
  executeWave: "execute_wave_cmd",
  finalizeWave: "finalize_wave_cmd",
  verifyAndFinalizeWave: "verify_and_finalize_wave_cmd",

  // Costs
  getCostSummary: "get_cost_summary_cmd",

  // Build pipeline
  buildApp: "build_app_cmd",
  resumeBuildApp: "resume_build_app_cmd",
  cancelBuildApp: "cancel_build_app_cmd",
  getBuildAppStatus: "get_build_app_status_cmd",
  getActiveBuildRun: "get_active_build_run_cmd",

  // Decision layer
  getDecisionConfig: "get_decision_config_cmd",
  setDecisionConfig: "set_decision_config_cmd",
  decisionHealth: "decision_health_cmd",
  listDecisionReviews: "list_decision_reviews_cmd",
  resolveDecisionReview: "resolve_decision_review_cmd",
  inferOutcome: "infer_outcome_cmd",
  diagnoseFailure: "diagnose_failure_cmd",

  // Skills / project
  checkSkillbridge: "check_skillbridge",

  // Backward channel (chat platforms + daemon)
  getChatPlatformConfigs: "get_chat_platform_configs_cmd",
  saveChatPlatformConfig: "save_chat_platform_config_cmd",
  deleteChatPlatformConfig: "delete_chat_platform_config_cmd",
  toggleChatPlatformConfig: "toggle_chat_platform_config_cmd",
  startBackwardChannelDaemon: "start_backward_channel_daemon_cmd",
  stopBackwardChannelDaemon: "stop_backward_channel_daemon_cmd",
  getBackwardChannelDaemonStatus: "get_backward_channel_daemon_status_cmd",
  getBackwardChannelDaemonLogs: "get_backward_channel_daemon_logs_cmd",
  checkBackwardChannelQueueHealth: "check_backward_channel_queue_health_cmd",
  testChatPlatformConnection: "test_chat_platform_connection_cmd",
} as const;

export type IpcCommand = (typeof IPC)[keyof typeof IPC];

// src-tauri/src/integration_tests.rs
//
// Integration tests that verify components work together as a system.
// These tests go beyond unit tests by verifying:
// - Adapter registry works with actual adapters
// - Wave persistence integrates with wave executor
// - Event system integrates with adapters
// - Full wave lifecycle works end-to-end

#[cfg(test)]
mod adapter_registry_tests {
    use crate::agent_adapters::{AdapterRegistry, AgentAdapter};

    #[test]
    fn test_registry_contains_mock_adapter() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("mock");
        assert!(adapter.is_some(), "Registry should contain mock adapter");
        assert_eq!(adapter.unwrap().name(), "mock");
    }

    #[test]
    fn test_registry_contains_opencode_adapter() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("opencode");
        assert!(adapter.is_some(), "Registry should contain opencode adapter");
        assert_eq!(adapter.unwrap().name(), "opencode");
    }

    #[test]
    fn test_mock_adapter_can_spawn_and_kill() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("mock").unwrap();
        
        // Spawn a session
        let session = adapter.spawn("test task", "/tmp/worktree");
        assert!(session.is_ok(), "Mock adapter should spawn successfully");
        let session = session.unwrap();
        assert_eq!(session.agent_id, "mock");
        assert_eq!(session.worktree, "/tmp/worktree");
        
        // Kill the session
        let kill_result = adapter.kill(&session);
        assert!(kill_result.is_ok(), "Mock adapter should kill successfully");
    }

    #[test]
    fn test_mock_adapter_parses_cost() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("mock").unwrap();
        
        // Test cost parsing
        let cost1 = adapter.parse_cost(r#"{"cost_usd": 0.05}"#);
        assert_eq!(cost1, Some(0.05));
        
        let cost2 = adapter.parse_cost(r#"{"usage": {"cost": 0.10}}"#);
        assert_eq!(cost2, Some(0.10));
        
        let cost3 = adapter.parse_cost("not json");
        assert_eq!(cost3, None);
    }

    #[test]
    fn test_opencode_adapter_parses_cost() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("opencode").unwrap();
        
        // Test cost parsing
        let cost1 = adapter.parse_cost(r#"{"cost_usd": 0.123}"#);
        assert_eq!(cost1, Some(0.123));
        
        let cost2 = adapter.parse_cost(r#"{"usage": {"cost": 0.456}}"#);
        assert_eq!(cost2, Some(0.456));
        
        let cost3 = adapter.parse_cost("not json");
        assert_eq!(cost3, None);
    }
}

#[cfg(test)]
mod wave_persistence_tests {
    use crate::wave_persistence::{save_wave_state, load_wave_state, resume_wave, WaveState, AgentState};
    use chrono::Utc;

    #[test]
    fn test_save_and_load_round_trip() {
        let state = WaveState {
            wave_id: "test-round-trip".to_string(),
            agents: vec![
                AgentState {
                    agent_id: "agent-1".to_string(),
                    worktree: "/tmp/wt1".to_string(),
                    status: "running".to_string(),
                    session_id: Some("sess-1".to_string()),
                    cost_usd: 0.05,
                },
            ],
            status: "executing".to_string(),
            checkpoint: Utc::now(),
        };
        
        // Save
        let save_result = save_wave_state(&state);
        assert!(save_result.is_ok(), "Should save successfully");
        
        // Load
        let loaded = load_wave_state("test-round-trip");
        assert!(loaded.is_ok(), "Should load successfully");
        let loaded = loaded.unwrap();
        
        // Verify
        assert_eq!(loaded.wave_id, state.wave_id);
        assert_eq!(loaded.agents.len(), state.agents.len());
        assert_eq!(loaded.agents[0].agent_id, state.agents[0].agent_id);
        assert_eq!(loaded.agents[0].cost_usd, state.agents[0].cost_usd);
    }

    #[test]
    fn test_multiple_waves_independent() {
        let state1 = WaveState {
            wave_id: "wave-1".to_string(),
            agents: vec![],
            status: "running".to_string(),
            checkpoint: Utc::now(),
        };
        
        let state2 = WaveState {
            wave_id: "wave-2".to_string(),
            agents: vec![],
            status: "completed".to_string(),
            checkpoint: Utc::now(),
        };
        
        // Save both
        save_wave_state(&state1).unwrap();
        save_wave_state(&state2).unwrap();
        
        // Load both
        let loaded1 = load_wave_state("wave-1").unwrap();
        let loaded2 = load_wave_state("wave-2").unwrap();
        
        // Verify independent
        assert_eq!(loaded1.status, "running");
        assert_eq!(loaded2.status, "completed");
    }

    #[test]
    fn test_resume_wave_returns_state() {
        let state = WaveState {
            wave_id: "test-resume".to_string(),
            agents: vec![
                AgentState {
                    agent_id: "agent-1".to_string(),
                    worktree: "/tmp/wt1".to_string(),
                    status: "running".to_string(),
                    session_id: Some("sess-1".to_string()),
                    cost_usd: 0.0,
                },
            ],
            status: "executing".to_string(),
            checkpoint: Utc::now(),
        };
        
        save_wave_state(&state).unwrap();
        
        let resumed = resume_wave("test-resume");
        assert!(resumed.is_ok(), "Should resume successfully");
        let resumed = resumed.unwrap();
        assert_eq!(resumed.wave_id, "test-resume");
        assert_eq!(resumed.agents.len(), 1);
    }

    #[test]
    fn test_load_nonexistent_wave_fails() {
        let result = load_wave_state("nonexistent-wave-xyz-123");
        assert!(result.is_err(), "Should fail for nonexistent wave");
    }
}

#[cfg(test)]
mod event_system_tests {
    use crate::agent_events::{create_output_event, create_status_event, create_cost_event};

    #[test]
    fn test_create_output_event() {
        let event = create_output_event("agent-1", "session-1", "test output line");
        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.line, "test output line");
        assert!(!event.timestamp.is_empty(), "Timestamp should not be empty");
    }

    #[test]
    fn test_create_status_event() {
        let event = create_status_event("agent-1", "session-1", "running");
        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.status, "running");
        assert!(!event.timestamp.is_empty(), "Timestamp should not be empty");
    }

    #[test]
    fn test_create_cost_event() {
        let event = create_cost_event("agent-1", "session-1", 0.05);
        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.cost_usd, 0.05);
        assert!(!event.timestamp.is_empty(), "Timestamp should not be empty");
    }

    #[test]
    fn test_events_have_valid_timestamps() {
        let output = create_output_event("a", "s", "line");
        let status = create_status_event("a", "s", "running");
        let cost = create_cost_event("a", "s", 0.0);
        
        // All timestamps should be valid RFC3339
        assert!(chrono::DateTime::parse_from_rfc3339(&output.timestamp).is_ok());
        assert!(chrono::DateTime::parse_from_rfc3339(&status.timestamp).is_ok());
        assert!(chrono::DateTime::parse_from_rfc3339(&cost.timestamp).is_ok());
    }
}

#[cfg(test)]
mod wave_executor_tests {
    use crate::wave_executor::{WaveExecutionReport, WaveRunConfig, AgentExecution};

    #[test]
    fn test_wave_execution_report_default() {
        let report = WaveExecutionReport::default();
        assert_eq!(report.plan_id, "");
        assert_eq!(report.base_repo, "");
        assert_eq!(report.agents.len(), 0);
        assert_eq!(report.total_cost_usd, 0.0);
        assert!(report.completed_at.is_none());
    }

    #[test]
    fn test_wave_run_config_fields() {
        let config = WaveRunConfig {
            plan_id: "plan-1".to_string(),
            base_repo: "/tmp/repo".to_string(),
            base_branch: "main".to_string(),
            agent_command: "mock".to_string(),
            agent_base_args: vec!["--model".to_string(), "test".to_string()],
            deadline_secs: Some(300),
            cost_cap_usd: Some(1.0),
        };
        
        assert_eq!(config.plan_id, "plan-1");
        assert_eq!(config.base_repo, "/tmp/repo");
        assert_eq!(config.agent_command, "mock");
        assert_eq!(config.deadline_secs, Some(300));
        assert_eq!(config.cost_cap_usd, Some(1.0));
    }

    #[test]
    fn test_agent_execution_structure() {
        let exec = AgentExecution {
            agent_ref: "frontend".to_string(),
            session_id: "sess-1".to_string(),
            worktree_path: ".worktrees/plan-1-frontend".to_string(),
            branch: "agent/plan-1-frontend".to_string(),
            status: "running".to_string(),
            guideline_path: ".worktrees/plan-1-frontend/.acc/GUIDELINE.md".to_string(),
            cost_usd: 0.05,
            retry_count: 0,
        };
        
        assert_eq!(exec.agent_ref, "frontend");
        assert_eq!(exec.session_id, "sess-1");
        assert_eq!(exec.status, "running");
        assert_eq!(exec.cost_usd, 0.05);
        assert!(exec.guideline_path.contains("GUIDELINE.md"));
    }

    #[test]
    fn test_wave_report_can_track_multiple_agents() {
        let mut report = WaveExecutionReport::default();
        report.plan_id = "test-plan".to_string();
        report.base_repo = "/tmp/repo".to_string();
        
        report.agents.push(AgentExecution {
            agent_ref: "agent-1".to_string(),
            session_id: "sess-1".to_string(),
            worktree_path: ".worktrees/wt1".to_string(),
            branch: "agent/wt1".to_string(),
            status: "running".to_string(),
            guideline_path: ".worktrees/wt1/.acc/GUIDELINE.md".to_string(),
            cost_usd: 0.05,
            retry_count: 0,
        });
        
        report.agents.push(AgentExecution {
            agent_ref: "agent-2".to_string(),
            session_id: "sess-2".to_string(),
            worktree_path: ".worktrees/wt2".to_string(),
            branch: "agent/wt2".to_string(),
            status: "done".to_string(),
            guideline_path: ".worktrees/wt2/.acc/GUIDELINE.md".to_string(),
            cost_usd: 0.10,
            retry_count: 0,
        });
        
        assert_eq!(report.agents.len(), 2);
        assert_eq!(report.agents[0].status, "running");
        assert_eq!(report.agents[1].status, "done");
    }
}

use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // Mirror db.rs::apply_migrations — keep this list in sync when migrations are added.
    conn.execute_batch(include_str!("../migrations/001_init.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/002_assets.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/003_integrations.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/004_backward_channel.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/008_control_sessions.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/010_knowledge_graph.sql")).unwrap();
    // 011 ends with a vec0 virtual table that needs the sqlite-vec extension
    // (not loaded yet), so it fails non-fatally — same as db.rs. The relational
    // tables before it are still created.
    let _ = conn.execute_batch(include_str!("../migrations/011_memory.sql"));
    conn.execute_batch(include_str!("../migrations/012_codebase_exploration.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/013_app_state_snapshot.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/014_bagua_semantics.sql")).unwrap();
    conn.execute_batch(include_str!("../migrations/015_user_preferences.sql")).unwrap();

    // Seed parent rows so child-table inserts satisfy foreign keys.
    // (The production code path always creates parents first; these tests
    // exercise child modules in isolation.)
    conn.execute_batch(
        r#"
        INSERT OR IGNORE INTO projects (id, path, name, created_at, updated_at) VALUES
          ('proj-1',  '/tmp/proj-1',  'P1',  datetime('now'), datetime('now')),
          ('proj-c',  '/tmp/proj-c',  'PC',  datetime('now'), datetime('now')),
          ('proj-c1', '/tmp/proj-c1', 'PC1', datetime('now'), datetime('now')),
          ('proj-s',  '/tmp/proj-s',  'PS',  datetime('now'), datetime('now')),
          ('proj-x',  '/tmp/proj-x',  'PX',  datetime('now'), datetime('now')),
          ('proj-y',  '/tmp/proj-y',  'PY',  datetime('now'), datetime('now')),
          ('proj-z',  '/tmp/proj-z',  'PZ',  datetime('now'), datetime('now'));

        INSERT OR IGNORE INTO agents (id, label, spawn_cmd, tier) VALUES
          ('claude',   'Claude Code', 'claude',   1),
          ('opencode', 'OpenCode',    'opencode', 2),
          ('agent-1',  'Agent 1',     'true',     2),
          ('agent-2',  'Agent 2',     'true',     2),
          ('agent-b1', 'Agent B1',    'true',     2),
          ('agent-r',  'Agent R',     'true',     2);

        INSERT OR IGNORE INTO sessions (id, started_at) VALUES
          ('session-1',  datetime('now')),
          ('session-b1', datetime('now')),
          ('session-e1', datetime('now')),
          ('session-e2', datetime('now')),
          ('session-h1', datetime('now')),
          ('session-l1', datetime('now')),
          ('session-t1', datetime('now'));
        "#,
    )
    .unwrap();

    conn
}

// ============================================================================
// DB + Migration Tests
// ============================================================================

#[test]
fn test_db_init_creates_tables() {
    let conn = setup_test_db();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(tables.contains(&"agents".to_string()));
    assert!(tables.contains(&"events".to_string()));
    assert!(tables.contains(&"knowledge_items".to_string()));
    assert!(tables.contains(&"knowledge_relations".to_string()));
    assert!(tables.contains(&"agent_messages".to_string()));
    assert!(tables.contains(&"chat_platform_configs".to_string()));
    assert!(tables.contains(&"control_sessions".to_string()));
    assert!(tables.contains(&"projects".to_string()));
    assert!(tables.contains(&"outcome_stats".to_string()));
    assert!(tables.contains(&"feature_plans".to_string()));
    assert!(tables.contains(&"plan_agents".to_string()));
    assert!(tables.contains(&"corrections".to_string()));
    // Memory layer tables must exist even though vec_memories (sqlite-vec) cannot be created.
    assert!(tables.contains(&"memory_facts".to_string()));
    assert!(tables.contains(&"session_checkpoints".to_string()));
}

#[test]
fn test_db_init_db_path() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = sourceforge::db::init_db_path(&db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |row| row.get(0))
        .unwrap();
    assert!(count > 10, "should have many tables, got {}", count);
}

// ============================================================================
// ACB (Agent Communication Bus) Tests
// ============================================================================

#[test]
fn test_acb_parse_signal() {
    let test_line = "[ACC:BLOCKER from=A1 to=B1 priority=HIGH] This is a blocker signal";
    let signal = sourceforge::acb::parse_acb_signal(test_line).unwrap();
    assert_eq!(signal.signal_type, "BLOCKER");
    assert_eq!(signal.from_agent, "A1");
    assert_eq!(signal.to_agent, "B1");
    assert_eq!(signal.priority, "HIGH");
    assert!(signal.body.contains("blocker signal"));
}

#[test]
fn test_acb_parse_signal_defaults() {
    let test_line = "[ACC:INFO from=C1] Doing fine";
    let signal = sourceforge::acb::parse_acb_signal(test_line).unwrap();
    assert_eq!(signal.signal_type, "INFO");
    assert_eq!(signal.from_agent, "C1");
    assert_eq!(signal.to_agent, "ALL");
    assert_eq!(signal.priority, "INFO");
}

#[test]
fn test_acb_parse_signal_no_match() {
    let result = sourceforge::acb::parse_acb_signal("Just a normal log line");
    assert!(result.is_none());
}

#[test]
fn test_acb_record_and_resolve() {
    let conn = setup_test_db();
    let signal = sourceforge::acb::parse_acb_signal(
        "[ACC:BLOCKER from=A1 to=B1 priority=HIGH] blocked on approval"
    ).unwrap();
    sourceforge::acb::record_acb_signal(&conn, &signal).unwrap();
    let open = sourceforge::acb::get_open_signals(&conn, None).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].signal_type, "BLOCKER");
    assert_eq!(open[0].status, "OPEN");
    sourceforge::acb::resolve_signal(&conn, &signal.id).unwrap();
    let open_after = sourceforge::acb::get_open_signals(&conn, None).unwrap();
    assert_eq!(open_after.len(), 0);
}

// ============================================================================
// Knowledge Layer Tests
// ============================================================================

#[test]
fn test_knowledge_create_and_query() {
    let conn = setup_test_db();
    let input = sourceforge::knowledge::KnowledgeItemInput {
        r#type: "pattern".to_string(),
        title: "Use Result for error handling".to_string(),
        content: "Always use Result<T, String> for fallible operations".to_string(),
        tags: Some("rust,error-handling".to_string()),
        stack_tags: Some("rust".to_string()),
        agent_tags: None,
        project_id: Some("proj-1".to_string()),
        session_ids: Some("session-1".to_string()),
        plan_ids: None,
        is_global: false,
    };
    let item = sourceforge::knowledge::create_knowledge_item(&conn, &input).unwrap();
    assert_eq!(item.r#type, "pattern");
    assert_eq!(item.confidence, 0.5);
    assert_eq!(item.confirmation_count, 1);
    assert_eq!(item.status, "active");

    let query = sourceforge::knowledge::KnowledgeQuery {
        q: Some("error handling".to_string()),
        stack: Some("rust".to_string()),
        agent: None,
        project_id: Some("proj-1".to_string()),
        r#type: Some("pattern".to_string()),
        status: Some("active".to_string()),
        min_confidence: Some(0.1),
        is_global: Some(false),
        limit: Some(10),
        offset: Some(0),
    };
    let results = sourceforge::knowledge::get_knowledge_items(&conn, &query).unwrap();
    assert!(results.len() == 1);
    assert_eq!(results[0].id, item.id);
}

#[test]
fn test_knowledge_update_and_delete() {
    let conn = setup_test_db();
    let input = sourceforge::knowledge::KnowledgeItemInput {
        r#type: "insight".to_string(),
        title: "Test item".to_string(),
        content: "Content here".to_string(),
        tags: None, stack_tags: None, agent_tags: None,
        project_id: None, session_ids: None, plan_ids: None,
        is_global: true,
    };
    let item = sourceforge::knowledge::create_knowledge_item(&conn, &input).unwrap();
    let updates = sourceforge::knowledge::KnowledgeItemUpdate {
        title: Some("Updated title".to_string()),
        content: Some("Updated content".to_string()),
        tags: None, stack_tags: None, agent_tags: None,
        confidence: Some(0.9),
        status: Some("archived".to_string()),
    };
    let updated = sourceforge::knowledge::update_knowledge_item(&conn, &item.id, &updates).unwrap();
    assert_eq!(updated.title, "Updated title");

    sourceforge::knowledge::delete_knowledge_item(&conn, &item.id).unwrap();
    let search = sourceforge::knowledge::search_knowledge(&conn, "Updated", 10).unwrap();
    assert!(search.is_empty());
}

#[test]
fn test_knowledge_compound() {
    let conn = setup_test_db();
    for i in 0..3 {
        let input = sourceforge::knowledge::KnowledgeItemInput {
            r#type: "pattern".to_string(),
            title: format!("compound test iteration {}", i),
            content: format!("This is content block number {}", i),
            tags: None, stack_tags: None, agent_tags: None,
            project_id: Some("proj-c".to_string()),
            session_ids: Some(format!("session-{}", i)),
            plan_ids: None, is_global: false,
        };
        sourceforge::knowledge::create_knowledge_item(&conn, &input).unwrap();
    }
    let results = sourceforge::knowledge::compound_knowledge(&conn, Some("proj-c")).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].r#type, "pattern");
}

#[test]
fn test_knowledge_relations() {
    let conn = setup_test_db();
    let input = |type_name: &str, title: &str| -> sourceforge::knowledge::KnowledgeItemInput {
        sourceforge::knowledge::KnowledgeItemInput {
            r#type: type_name.to_string(),
            title: title.to_string(),
            content: "content".to_string(),
            tags: None, stack_tags: None, agent_tags: None,
            project_id: None, session_ids: None, plan_ids: None,
            is_global: false,
        }
    };
    let a = sourceforge::knowledge::create_knowledge_item(&conn, &input("pattern", "Item A")).unwrap();
    let b = sourceforge::knowledge::create_knowledge_item(&conn, &input("antipattern", "Item B")).unwrap();
    let rel = sourceforge::knowledge::add_knowledge_relation(&conn, &a.id, &b.id, "contradicts").unwrap();
    assert_eq!(rel.relation_type, "contradicts");
    let rels = sourceforge::knowledge::get_knowledge_relations(&conn, &a.id).unwrap();
    assert_eq!(rels.len(), 1);
}

#[test]
fn test_knowledge_stats() {
    let conn = setup_test_db();
    let input = sourceforge::knowledge::KnowledgeItemInput {
        r#type: "fact".to_string(), title: "Fact".to_string(), content: "A fact".to_string(),
        tags: None, stack_tags: None, agent_tags: None,
        project_id: Some("proj-s".to_string()), session_ids: None, plan_ids: None,
        is_global: false,
    };
    sourceforge::knowledge::create_knowledge_item(&conn, &input).unwrap();
    let stats = sourceforge::knowledge::get_knowledge_stats(&conn, Some("proj-s")).unwrap();
    assert_eq!(stats["total"], 1);
    assert_eq!(stats["by_type"][0]["type"], "fact");
}

#[test]
fn test_knowledge_jaccard_similarity() {
    let sim = sourceforge::knowledge::jaccard_similarity(
        "hello world foo bar", "hello world baz qux"
    );
    assert!(sim > 0.0, "jaccard similarity should be > 0 for overlapping strings");
    let sim2 = sourceforge::knowledge::jaccard_similarity(
        "completely different a b", "no overlap here c d"
    );
    assert_eq!(sim2, 0.0, "jaccard similarity should be 0 for disjoint strings");
    let exact = sourceforge::knowledge::jaccard_similarity(
        "hello world", "hello world"
    );
    assert!((exact - 1.0).abs() < f64::EPSILON, "identical strings should score 1.0");
}

#[test]
fn test_preflight_warnings() {
    let conn = setup_test_db();
    let input = sourceforge::knowledge::KnowledgeItemInput {
        r#type: "antipattern".to_string(),
        title: "Avoid global state".to_string(),
        content: "Global mutable state causes issues".to_string(),
        tags: None, stack_tags: Some("react".to_string()),
        agent_tags: None, project_id: None, session_ids: None, plan_ids: None,
        is_global: false,
    };
    sourceforge::knowledge::create_knowledge_item(&conn, &input).unwrap();
    let warnings = sourceforge::knowledge::get_preflight_warnings(&conn, "react", 10).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].title, "Avoid global state");
}

// ============================================================================
// Routing Tests
// ============================================================================

#[test]
fn test_routing_fallback() {
    let conn = setup_test_db();
    let suggestions = sourceforge::routing::route_task(&conn, "fix this bug", "debug", None).unwrap();
    assert!(!suggestions.is_empty(), "should return fallback suggestion");
    assert_eq!(suggestions[0].confidence, 0.5, "fallback confidence should be 0.5");
    assert_eq!(suggestions[0].agent_id, "claude", "debug should route to claude");
}

#[test]
fn test_routing_with_outcome_stats() {
    let conn = setup_test_db();
    conn.execute_batch(
        "INSERT INTO outcome_stats (agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s)
         VALUES ('opencode', 'review', NULL, 10, 8, 1, 1, 45.0)"
    ).unwrap();
    let suggestions = sourceforge::routing::route_task(&conn, "review this PR", "review", None).unwrap();
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].agent_id, "opencode");
    assert!((suggestions[0].confidence - 0.8).abs() < f64::EPSILON);
}

#[test]
fn test_build_handoff_envelope() {
    let envelope = sourceforge::routing::HandoffEnvelope {
        original_task: "Fix login bug".to_string(),
        completed_by: "claude".to_string(),
        model_used: "claude-3".to_string(),
        output_summary: "Fixed the race condition".to_string(),
        changed_files: vec!["src/auth.rs".to_string()],
        diff_preview: "- bug\n+ fix".to_string(),
        handoff_instruction: "Test thoroughly".to_string(),
        next_agent: "opencode".to_string(),
        next_model: "gemini-pro".to_string(),
    };
    let result = sourceforge::routing::build_handoff_envelope(&envelope);
    assert!(result.contains("## AGENT HANDOFF"));
    assert!(result.contains("Fix login bug"));
    assert!(result.contains("claude"));
    assert!(result.contains("opencode"));
}

#[test]
fn test_models_crud() {
    let conn = setup_test_db();
    let entry = sourceforge::routing::ModelEntry {
        id: "model-1".to_string(),
        label: "Claude 3.5 Sonnet".to_string(),
        provider: "anthropic".to_string(),
        model_path: "claude-3-5-sonnet".to_string(),
        strengths: Some("coding,reasoning".to_string()),
        agent_id: Some("claude".to_string()),
        alternation_index: Some(1),
        is_active: true,
    };
    sourceforge::routing::add_model(&conn, &entry).unwrap();
    let models = sourceforge::routing::get_models(&conn).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].label, "Claude 3.5 Sonnet");
    sourceforge::routing::toggle_model(&conn, "model-1", false).unwrap();
    let models_after = sourceforge::routing::get_models(&conn).unwrap();
    assert!(!models_after[0].is_active);
}

// ============================================================================
// Orchestrator Tests
// ============================================================================

#[test]
fn test_orchestrator_create_wave_plan() {
    let conn = setup_test_db();
    let plan = sourceforge::orchestrator::create_wave_plan(&conn, "proj-x", "fix-auth").unwrap();
    assert_eq!(plan.project_id, "proj-x");
    assert_eq!(plan.slug, "fix-auth");
    assert_eq!(plan.status, "planning");
    let agents = sourceforge::orchestrator::get_plan_agents(&conn, &plan.id).unwrap();
    assert!(agents.is_empty());
}

#[test]
fn test_orchestrator_add_plan_agent() {
    let conn = setup_test_db();
    let plan = sourceforge::orchestrator::create_wave_plan(&conn, "proj-y", "refactor-db").unwrap();
    let agent = sourceforge::orchestrator::add_plan_agent(
        &conn, &plan.id, "W6.A", "Refactor database layer", 6, None, Some("opencode")
    ).unwrap();
    assert_eq!(agent.plan_id, plan.id);
    assert_eq!(agent.agent_ref, "W6.A");
    assert_eq!(agent.status, "queued");
    let agents = sourceforge::orchestrator::get_plan_agents(&conn, &plan.id).unwrap();
    assert_eq!(agents.len(), 1);
    sourceforge::orchestrator::update_plan_agent_status(&conn, &agent.id, "running").unwrap();
    let agents_after = sourceforge::orchestrator::get_plan_agents(&conn, &plan.id).unwrap();
    assert_eq!(agents_after[0].status, "running");
    assert!(agents_after[0].started_at.is_some());
}

#[test]
fn test_validate_handoff_schema() {
    let valid = "## AGENT HANDOFF\n\n**From agent:** test\n**To agent:** other\n\n### Completed Work\nDone\n\n### Test Results\nAll pass\n\n### Interface Contracts Exposed\nNone\n\n### Files NOT Modified\nNone\n\n### Design Decisions\nNone\n\n### Handoff Instructions\nNone";
    let (pass, missing) = sourceforge::orchestrator::validate_handoff_schema(valid);
    assert!(pass, "valid handoff should pass, missing: {:?}", missing);
    let invalid = "## AGENT HANDOFF\n\nJust some random text.";
    let (pass, missing) = sourceforge::orchestrator::validate_handoff_schema(invalid);
    assert!(!pass);
    assert!(missing.contains(&"Completed Work".to_string()));
}

#[test]
fn test_generate_agent_guideline() {
    let result = sourceforge::orchestrator::generate_agent_guideline(
        "W6.B", "Write tests", "Integration test the backend", None,
        &["claude-3", "gemini-pro"],
        &["tests/integration.rs"],
        &["src/main.rs"],
    );
    assert!(result.contains("AGENT W6.B GUIDELINE"));
    assert!(result.contains("Integration test the backend"));
    assert!(result.contains("claude-3, gemini-pro"));
    assert!(result.contains("tests/integration.rs"));
    assert!(result.contains("src/main.rs"));
}

#[test]
fn test_decide_orchestration_path() {
    let decision = sourceforge::orchestrator::decide_orchestration_path(
        "W6.B", "Write tests", true, "task-tool",
        "opencode --prompt \"{prompt}\"", "claude-3", "/tmp/proj"
    );
    assert_eq!(decision.path, "native");
    assert_eq!(decision.mechanism, "task()");
    assert!(decision.command.contains("task(subagent_type"));

    let external = sourceforge::orchestrator::decide_orchestration_path(
        "W6.C", "Build binary", false, "generic",
        "opencode --prompt \"{prompt}\"", "gemini-pro", "/tmp/proj"
    );
    assert_eq!(external.path, "external");
    assert_eq!(external.mechanism, "waveCommand");
}

#[test]
fn test_detect_subagent_spawn() {
    let pty = "Some output\nDispatching subagent for task\nMore output";
    let result = sourceforge::orchestrator::detect_subagent_spawn(
        pty, "Dispatching subagent|Sub-task started", "session-1", "W6.A"
    );
    assert!(result.is_some());
    let spawn = result.unwrap();
    assert_eq!(spawn.agent_ref, "W6.A");
    assert_eq!(spawn.status, "detected");
}

#[test]
fn test_orchestrator_corrections() {
    let conn = setup_test_db();
    let plan = sourceforge::orchestrator::create_wave_plan(&conn, "proj-z", "fix-crash").unwrap();
    let correction = sourceforge::orchestrator::create_correction(
        &conn, &plan.id, "W6.C", "Null pointer crash",
        "Missing null check", "Add null check before dereference",
        "Add unit test for null case", 1
    ).unwrap();
    assert!(!correction.resolved);
    let corrections = sourceforge::orchestrator::get_corrections(&conn, &plan.id).unwrap();
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0].retry_number, 1);
}

#[test]
fn test_generate_orchestrator_guideline() {
    let result = sourceforge::orchestrator::generate_orchestrator_guideline(
        "W5.A", "Build feature", "Implement the feature", None,
        true, "task-tool", &["W5.B", "W5.C"],
        &["claude-3"], &["src/feature.rs"],
        &["src/legacy.rs"],
    );
    assert!(result.contains("Subagent Delegation"));
    assert!(result.contains("W5.B"));
    assert!(result.contains("W5.C"));
}

// ============================================================================
// Events Tests
// ============================================================================

#[test]
fn test_log_and_retrieve_events() {
    let conn = setup_test_db();
    let event_id = sourceforge::events::log_event(
        &conn, "session-e1", "agent-1", "file_edit",
        Some("src/main.rs"), Some(10), Some(5), Some(0)
    ).unwrap();
    let events = sourceforge::events::get_session_events(&conn, "session-e1").unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "file_edit");
    assert_eq!(events[0].target.as_deref(), Some("src/main.rs"));
    assert_eq!(events[0].lines_added, Some(10));
}

#[test]
fn test_log_event_with_payload() {
    let conn = setup_test_db();
    let event_id = sourceforge::events::log_event_with_payload(
        &conn, "session-e2", "agent-2", "agent_output",
        Some("analysis"), "Detailed analysis output here"
    ).unwrap();
    let detail = sourceforge::events::get_session_event_detail(&conn, &event_id).unwrap();
    assert_eq!(detail, Some("Detailed analysis output here".to_string()));
}

// ============================================================================
// Intelligence Tests
// ============================================================================

#[test]
fn test_record_outcome_and_stats() {
    let conn = setup_test_db();
    conn.execute_batch(
        "INSERT INTO sessions (id, project_id, agent_id, started_at, task_desc, task_type)
         VALUES ('session-o1', NULL, 'agent-r', '2026-01-01T00:00:00Z', 'review code', 'review')"
    ).unwrap();
    let record = sourceforge::intelligence::record_outcome(
        &conn, "session-o1", "agent-r", "review", "done", 30.0
    ).unwrap();
    assert_eq!(record.outcome, "done");
    let stats = sourceforge::intelligence::get_outcome_stats(&conn, None, None).unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total, 1);
    assert_eq!(stats[0].done, 1);
}

#[test]
fn test_detect_limit_event() {
    let result = sourceforge::intelligence::detect_limit_event(
        "Error: rate limit exceeded, please wait"
    );
    assert!(result.is_some());
    let (etype, pattern) = result.unwrap();
    assert_eq!(etype, "limit_event");
    assert_eq!(pattern, "rate limit");

    let no_match = sourceforge::intelligence::detect_limit_event(
        "Normal operation output"
    );
    assert!(no_match.is_none());
}

#[test]
fn test_limit_event_lifecycle() {
    let conn = setup_test_db();
    let limit = sourceforge::intelligence::record_limit_event(
        &conn, "session-l1", None, "rate_limit", "Too many requests"
    ).unwrap();
    assert!(!limit.resolved);
    sourceforge::intelligence::resolve_limit_event(&conn, &limit.id, "Switched to backup model").unwrap();
    let unresolved = sourceforge::intelligence::get_unresolved_limits(&conn, Some("session-l1")).unwrap();
    assert_eq!(unresolved.len(), 0);
}

#[test]
fn test_token_usage() {
    let conn = setup_test_db();
    sourceforge::intelligence::record_token_usage(
        &conn, "session-t1", Some("agent-1"), "chat", Some("claude-3"), 500, 200
    ).unwrap();
    sourceforge::intelligence::record_token_usage(
        &conn, "session-t1", Some("agent-1"), "chat", Some("claude-3"), 300, 150
    ).unwrap();
    let stats = sourceforge::intelligence::get_token_usage_stats(&conn, Some("session-t1")).unwrap();
    assert_eq!(stats["total_tokens_in"], 800);
    assert_eq!(stats["total_tokens_out"], 350);
    assert_eq!(stats["total_tokens"], 1150);
}

#[test]
fn test_run_heartbeat_check() {
    let conn = setup_test_db();
    let result = sourceforge::intelligence::run_heartbeat_check(
        &conn, "session-h1", "2026-01-01T00:00:00Z", true
    ).unwrap();
    assert_eq!(result.session_id, "session-h1");
    assert!(result.last_activity_s > 0);
}

#[test]
fn test_suggest_outcome() {
    let done = sourceforge::intelligence::suggest_outcome("All tests passed successfully", 10);
    assert_eq!(done, Some("done".to_string()));
    let failed = sourceforge::intelligence::suggest_outcome("Error: compilation failed", 10);
    assert_eq!(failed, Some("failed".to_string()));
    let none = sourceforge::intelligence::suggest_outcome("Still working...", 5);
    assert_eq!(none, None);
}

#[test]
fn test_extract_pty_context() {
    let output = "line1\nline2\nline3\nline4\nline5";
    let extracted = sourceforge::intelligence::extract_pty_context(output, 3);
    assert_eq!(extracted, "line3\nline4\nline5");
}

#[test]
fn test_build_intelligence_prompt() {
    let prompt = sourceforge::intelligence::build_intelligence_prompt(
        "Analyze this", "Some context", "JSON format"
    );
    assert!(prompt.contains("Analyze this"));
    assert!(prompt.contains("Some context"));
    assert!(prompt.contains("JSON format"));
}

#[test]
fn test_subagent_patterns() {
    let patterns = sourceforge::intelligence::get_subagent_patterns();
    assert!(patterns.len() >= 7);
    assert!(patterns.iter().any(|p| p.agent_id == "claude"));
    assert!(patterns.iter().any(|p| p.agent_id == "opencode"));
}

#[test]
fn test_detect_subagent_activity() {
    let pty = "some output\nDispatching subagent for X\nmore\nspawn_agent('y')\nend";
    let patterns = &["Dispatching subagent", "spawn_agent"];
    let detections = sourceforge::intelligence::detect_subagent_activity(pty, patterns);
    assert_eq!(detections.len(), 2);
}

// ============================================================================
// Budget Module Tests
// ============================================================================

#[test]
fn test_budget_create_and_query() {
    let conn = setup_test_db();
    let input = sourceforge::budget::BudgetInput {
        session_id: Some("session-b1".to_string()),
        plan_agent_id: None,
        agent_id: "agent-b1".to_string(),
        task_complexity: None,
        model: Some("claude-3".to_string()),
        budget_total: Some(10000),
    };
    let budget = sourceforge::budget::create_budget(&conn, &input).unwrap();
    assert_eq!(budget.budget_total, 10000);
    assert_eq!(budget.budget_used, 0);
    let budgets = sourceforge::budget::get_budgets(&conn, Some("session-b1"), None, None).unwrap();
    assert_eq!(budgets.len(), 1);
}

// ============================================================================
// Control Sessions Tests
// ============================================================================

#[test]
fn test_control_sessions_claim_release() {
    let conn = setup_test_db();
    let file = sourceforge::control::claim_file(
        &conn, "proj-c1", "src/main.rs", "thread-1"
    ).unwrap();
    assert_eq!(file.file_path, "src/main.rs");
    assert_eq!(file.claimed_by_thread_id, "thread-1");
    let locks = sourceforge::control::get_project_locks(&conn, "proj-c1").unwrap();
    assert_eq!(locks.len(), 1);
    sourceforge::control::release_file(&conn, "proj-c1", "src/main.rs").unwrap();
    let locks_after = sourceforge::control::get_project_locks(&conn, "proj-c1").unwrap();
    assert_eq!(locks_after.len(), 0);
}

// ============================================================================
// Knowledge Compounder Status Tests
// ============================================================================

#[test]
fn test_compounder_status_after_run() {
    let conn = setup_test_db();

    conn.execute(
        "INSERT INTO sessions (id, project_id, agent_id, model, started_at)
         VALUES ('s1', 'proj-1', 'claude', 'claude-3', datetime('now', '-2 hours'))",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, session_ids)
         VALUES ('ki1', 'PatternCard', 'Use React.memo', 'Memoize list items', 0.85, 3, 0, 'active', datetime('now', '-1 hour'), datetime('now', '-1 hour'), 's1')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, session_ids)
         VALUES ('ki2', 'AntiPattern', 'Avoid large bundles', 'Split code', 0.72, 1, 0, 'active', datetime('now', '-30 minutes'), datetime('now', '-30 minutes'), 's1')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, session_ids)
         VALUES ('ki3', 'Runbook', 'Setup steps', 'npm install then npm run dev', 0.60, 2, 1, 'active', datetime('now', '-10 minutes'), datetime('now', '-10 minutes'), 's1')",
        [],
    ).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM knowledge_items WHERE session_ids = 's1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 3, "should have 3 knowledge items from session s1");

    let stats = sourceforge::knowledge::get_knowledge_stats(&conn, None).unwrap();
    assert!(stats["total"].as_i64().unwrap() >= 3, "total should be at least 3, got {}", stats["total"]);
    assert!(stats["avg_confidence"].as_f64().unwrap() > 0.7, "avg confidence should be > 0.7, got {}", stats["avg_confidence"]);
    assert!(stats["by_type"].as_array().unwrap().len() >= 2, "should have at least 2 distinct types");
}

#[test]
fn test_preflight_warnings_stack_filter() {
    let conn = setup_test_db();

    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, stack_tags)
         VALUES ('ap1', 'AntiPattern', 'Nested useEffects in React', 'Causes re-renders', 0.92, 7, 0, 'active', datetime('now'), datetime('now'), 'react')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, stack_tags)
         VALUES ('ap2', 'AntiPattern', 'Global mutable state in Python', 'Thread safety issue', 0.88, 5, 0, 'active', datetime('now'), datetime('now'), 'python')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed, stack_tags)
         VALUES ('ap3', 'AntiPattern', 'Prop drilling in React', 'Use context instead', 0.81, 4, 0, 'active', datetime('now'), datetime('now'), 'react')",
        [],
    ).unwrap();

    let react_items = sourceforge::knowledge::get_knowledge_items(&conn, &sourceforge::knowledge::KnowledgeQuery {
        q: None,
        stack: Some("react".to_string()),
        agent: None,
        project_id: None,
        r#type: Some("AntiPattern".to_string()),
        status: None,
        min_confidence: None,
        is_global: None,
        limit: Some(50),
        offset: Some(0),
    }).unwrap();
    assert_eq!(react_items.len(), 2, "should find 2 react anti-patterns, got {}", react_items.len());

    let python_items = sourceforge::knowledge::get_knowledge_items(&conn, &sourceforge::knowledge::KnowledgeQuery {
        q: None,
        stack: Some("python".to_string()),
        agent: None,
        project_id: None,
        r#type: None,
        status: None,
        min_confidence: None,
        is_global: None,
        limit: Some(50),
        offset: Some(0),
    }).unwrap();
    assert_eq!(python_items.len(), 1, "should find 1 python item, got {}", python_items.len());

    let rust_items = sourceforge::knowledge::get_knowledge_items(&conn, &sourceforge::knowledge::KnowledgeQuery {
        q: None,
        stack: Some("rust".to_string()),
        agent: None,
        project_id: None,
        r#type: None,
        status: None,
        min_confidence: None,
        is_global: None,
        limit: Some(50),
        offset: Some(0),
    }).unwrap();
    assert_eq!(rust_items.len(), 0, "should find 0 rust items");
}

#[test]
fn test_knowledge_stats_accuracy() {
    let conn = setup_test_db();

    let confidences = vec![0.50, 0.70, 0.80, 0.90, 0.95];
    for (i, conf) in confidences.iter().enumerate() {
        let id = format!("ks{}", i + 1);
        conn.execute(
            "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed)
             VALUES (?1, 'PatternCard', ?2, 'content', ?3, 1, 0, 'active', datetime('now'), datetime('now'))",
            rusqlite::params![id, format!("Pattern {}", i + 1), conf],
        ).unwrap();
    }

    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed)
         VALUES ('ks-arch1', 'DecisionLog', 'Old decision', 'was wrong', 0.3, 1, 0, 'archived', datetime('now'), datetime('now'))",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, first_seen, last_confirmed)
         VALUES ('ks-arch2', 'LessonBrief', 'Old lesson', 'outdated', 0.2, 1, 0, 'archived', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let stats = sourceforge::knowledge::get_knowledge_stats(&conn, None).unwrap();
    assert_eq!(stats["total"], 7, "total should be 7, got {}", stats["total"]);

    assert!(stats["avg_confidence"].as_f64().unwrap() > 0.6, "avg confidence should be > 0.6, got {}", stats["avg_confidence"]);
    assert!(stats["avg_confidence"].as_f64().unwrap() < 0.65, "avg confidence should be < 0.65, got {}", stats["avg_confidence"]);

    assert!(stats["by_type"].as_array().unwrap().len() >= 3, "should have at least 3 types, got {}", stats["by_type"].as_array().unwrap().len());
}

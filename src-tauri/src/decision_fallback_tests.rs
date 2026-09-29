//! R51: the offline-fallback matrix as an integration test.
//!
//! Rows verified here use the **injectable transport seam** — a transport that
//! always errors stands in for "backend unavailable". Rows whose consumers build
//! their own transport/config from the environment (outcome, budget, failure
//! confidence, handoff, route_task, daemon router) are covered by those modules'
//! own unit tests; the full matrix lives in
//! `docs/specs/001-decision-layer/implementation-details/decision-contract.md`.

use crate::decision::{DecisionConfig, DecisionError, DecisionTransport};

/// A transport that always fails — the backend is unreachable.
struct OfflineTransport;

impl DecisionTransport for OfflineTransport {
    fn post(
        &self,
        _url: &str,
        _key: Option<&str>,
        _body: &serde_json::Value,
    ) -> Result<serde_json::Value, DecisionError> {
        Err(DecisionError::Transport("offline".into()))
    }
}

fn db_with_schema() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/016_decision_usage.sql"))
        .unwrap();
    conn.execute_batch(include_str!("../migrations/019_decision_reviews_unique.sql"))
        .unwrap();
    conn.execute_batch(include_str!("../migrations/020_decision_usage_hash.sql"))
        .unwrap();
    conn.execute_batch(
        "CREATE TABLE knowledge_relations (
            from_id TEXT, to_id TEXT, relation_type TEXT, created_at TEXT,
            trigram_tag TEXT, hexagram_tag TEXT, wuxing_cycle TEXT,
            bagua_confidence REAL, relation_multivector TEXT,
            PRIMARY KEY (from_id, to_id, relation_type)
        );",
    )
    .unwrap();
    conn
}

fn knowledge_item(id: &str, ty: &str, title: &str, content: &str) -> crate::knowledge::KnowledgeItem {
    crate::knowledge::KnowledgeItem {
        id: id.into(),
        r#type: ty.into(),
        title: title.into(),
        content: content.into(),
        tags: None,
        stack_tags: None,
        agent_tags: None,
        project_id: None,
        session_ids: None,
        plan_ids: None,
        confidence: 1.0,
        confirmation_count: 1,
        is_global: false,
        first_seen: "2026-01-01T00:00:00Z".into(),
        last_confirmed: "2026-01-01T00:00:00Z".into(),
        status: "active".into(),
        pending_task_data: None,
    }
}

/// Core: every primitive surfaces the outage as `Err`, so consumers can fall back.
#[test]
fn core_returns_unavailable_when_backend_down() {
    let cfg = DecisionConfig::default();
    let t = OfflineTransport;
    assert!(crate::decision::judge(
        &cfg, &t, Some("k"), &serde_json::json!("s"), "?", "q", None
    )
    .is_err());
    assert!(crate::decision::choose(
        &cfg,
        &t,
        Some("k"),
        &serde_json::json!("s"),
        "?",
        &crate::decision::criteria(&["a", "b"]),
        "q",
        None
    )
    .is_err());
    assert!(crate::decision::judge_batch(
        &cfg,
        &t,
        Some("k"),
        &serde_json::json!("s"),
        &[("q".to_string(), "?".to_string())],
        None
    )
    .is_err());
}

/// deployment verify → deterministic checks only (no semantic checks added).
#[test]
fn deployment_verify_falls_back_to_deterministic_only() {
    let conn = db_with_schema();
    let cfg = DecisionConfig::default();
    let checks = crate::verification::semantic_checks_with(
        &cfg,
        &OfflineTransport,
        Some("k"),
        &conn,
        Some("readme"),
        "diff --git a/x b/x",
    );
    assert!(checks.is_empty(), "no semantic checks when the backend is down");
}

/// KG typing → the LLM-emitted type is preserved and nothing is dropped.
#[test]
fn kg_typing_falls_back_to_llm_types() {
    let conn = db_with_schema();
    let cfg = DecisionConfig::default();
    let entities = vec![crate::kg_extraction::ExtractionEntity {
        name: "a".into(),
        r#type: "file".into(),
        description: "d".into(),
        confidence: 1.0,
    }];
    let (types, keep) = crate::kg_extraction::plan_entities(
        &cfg,
        &OfflineTransport,
        Some("k"),
        &conn,
        &entities,
        true,
    );
    assert_eq!(types, vec!["file".to_string()], "LLM type preserved");
    assert_eq!(keep, vec![true], "entity not dropped");
}

/// contradiction/merge → `jaccard_similarity` fallback records the relation.
#[test]
fn contradiction_falls_back_to_jaccard() {
    let conn = db_with_schema();
    let new_item = knowledge_item(
        "n1",
        "antipattern",
        "avoid unwrap",
        "do not use unwrap in production code",
    );
    let existing = vec![knowledge_item(
        "e1",
        "pattern",
        "avoid unwrap",
        "never use unwrap in production code",
    )];
    let recorded = crate::knowledge::detect_and_record_contradictions(
        &conn,
        &[new_item],
        &existing,
        &OfflineTransport,
        Some("k"),
    )
    .unwrap();
    assert_eq!(recorded, 1, "jaccard fallback recorded the contradiction");
}

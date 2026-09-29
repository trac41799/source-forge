use crate::intelligence::{self, OpenRouterRequest, Priority};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionEntity {
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRelation {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub evidence: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<ExtractionEntity>,
    pub relationships: Vec<ExtractionRelation>,
}

pub fn build_extraction_prompt(
    session_events: &str,
    code_diffs: &str,
) -> String {
    format!(
        "Extract entities and relationships from this coding session.\n\
         Output JSON:\n\
         {{\n  \"entities\": [\n    {{\"name\": string, \"type\": \"file|function|pattern|error|decision|library\",\n\
         \"description\": string, \"confidence\": 0.0-1.0}}\n  ],\n\
         \"relationships\": [\n    {{\"source\": \"entity_name\", \"target\": \"entity_name\",\n\
         \"type\": \"caused_by|fixed_by|extends|requires|contradicts|similar_to\",\n\
         \"evidence\": string, \"confidence\": 0.0-1.0}}\n  ]\n}}\n\n\
         Session Events:\n{}\n\nCode Diffs:\n{}\n\n\
         Return only the JSON. No prose, no markdown fence.",
        session_events, code_diffs
    )
}

pub fn parse_extraction_response(content: &str) -> ExtractionResult {
    let trimmed = content.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    };

    if let Ok(result) = serde_json::from_str::<ExtractionResult>(&json_str) {
        return result;
    }
    if let Some(start) = json_str.find('{') {
        if let Some(end) = json_str.rfind('}') {
            if end > start {
                if let Ok(result) =
                    serde_json::from_str::<ExtractionResult>(&json_str[start..=end])
                {
                    return result;
                }
            }
        }
    }
    ExtractionResult {
        entities: vec![],
        relationships: vec![],
    }
}

pub async fn run_llm_extraction(
    session_events: &str,
    code_diffs: &str,
    api_key: &str,
) -> Result<ExtractionResult, String> {
    let prompt = build_extraction_prompt(session_events, code_diffs);
    let request = OpenRouterRequest {
        prompt,
        model: None,
        priority: Priority::Normal,
        max_tokens: Some(4096),
        temperature: Some(0.3),
    };
    let resp = intelligence::invoke_with_backoff(request, api_key, 3)
        .await
        .map_err(|e| e.to_string())?;
    Ok(parse_extraction_response(&resp.content))
}

/// Synchronous variant for a sync Tauri command holding the DB guard (#3).
pub fn run_llm_extraction_blocking(
    session_events: &str,
    code_diffs: &str,
    api_key: &str,
) -> Result<ExtractionResult, String> {
    let prompt = build_extraction_prompt(session_events, code_diffs);
    let request = OpenRouterRequest {
        prompt,
        model: None,
        priority: Priority::Normal,
        max_tokens: Some(4096),
        temperature: Some(0.3),
    };
    let resp = intelligence::invoke_openrouter_blocking(request, api_key, 3)?;
    Ok(parse_extraction_response(&resp.content))
}

/// R-4: decide every entity type **and** the real-entity gate with two batched
/// requests (instead of two per entity). Returns `(types, keep)`.
pub(crate) fn plan_entities(
    cfg: &crate::decision::DecisionConfig,
    transport: &dyn crate::decision::DecisionTransport,
    key: Option<&str>,
    db: &Connection,
    entities: &[ExtractionEntity],
    backend_ready: bool,
) -> (Vec<String>, Vec<bool>) {
    let mut types: Vec<String> = entities.iter().map(|e| e.r#type.clone()).collect();
    let mut keep = vec![true; entities.len()];
    if entities.is_empty() || !backend_ready {
        return (types, keep);
    }
    let descriptors: Vec<String> = entities
        .iter()
        .map(|e| format!("{}: {}", e.name, e.description))
        .collect();
    let state = serde_json::json!(descriptors);

    let crit = crate::decision::criteria(crate::decision::KG_ENTITY_TYPES);
    let type_qs: Vec<(String, String, std::collections::BTreeMap<String, String>)> = descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| {
            (
                format!("entity_type_{i}"),
                format!("Classify entity #{i}: {d}"),
                crit.clone(),
            )
        })
        .collect();
    if let Ok(answered) =
        crate::decision::choose_batch(cfg, transport, key, &state, &type_qs, Some(db))
    {
        for i in 0..entities.len() {
            if let Some((t, _)) = answered.get(&format!("entity_type_{i}")) {
                types[i] = t.clone();
            }
        }
    }

    let gate_qs: Vec<(String, String)> = descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| {
            (
                format!("real_{i}"),
                format!(
                    "Is entity #{i} a real, reusable code entity worth storing as knowledge? {d}"
                ),
            )
        })
        .collect();
    if let Ok(ps) = crate::decision::judge_batch(cfg, transport, key, &state, &gate_qs, Some(db)) {
        for i in 0..entities.len() {
            if let Some(p) = ps.get(&format!("real_{i}")) {
                if *p < cfg.review_threshold {
                    keep[i] = false;
                }
            }
        }
    }
    (types, keep)
}

/// R-4: decide every relation type with one batched request.
fn plan_relations(
    cfg: &crate::decision::DecisionConfig,
    transport: &dyn crate::decision::DecisionTransport,
    key: Option<&str>,
    db: &Connection,
    rels: &[ExtractionRelation],
    backend_ready: bool,
) -> Vec<String> {
    let mut rtypes: Vec<String> = rels.iter().map(|r| r.relation_type.clone()).collect();
    if rels.is_empty() || !backend_ready {
        return rtypes;
    }
    let descriptors: Vec<String> = rels
        .iter()
        .map(|r| format!("{} → {}: {}", r.source, r.target, r.evidence))
        .collect();
    let state = serde_json::json!(descriptors);
    let crit = crate::decision::criteria(crate::decision::KG_RELATION_TYPES);
    let qs: Vec<(String, String, std::collections::BTreeMap<String, String>)> = descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| {
            (
                format!("rel_type_{i}"),
                format!("Classify relation #{i}: {d}"),
                crit.clone(),
            )
        })
        .collect();
    if let Ok(answered) = crate::decision::choose_batch(cfg, transport, key, &state, &qs, Some(db)) {
        for i in 0..rels.len() {
            if let Some((t, _)) = answered.get(&format!("rel_type_{i}")) {
                rtypes[i] = t.clone();
            }
        }
    }
    rtypes
}

pub fn persist_extraction(
    db: &Connection,
    result: &ExtractionResult,
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(usize, usize), String> {
    use crate::knowledge::{
        create_knowledge_item, add_knowledge_relation, KnowledgeItemInput,
    };

    let mut entity_count = 0usize;
    let mut rel_count = 0usize;

    // M1 (spec R12): type entities/relations via the decision layer; the LLM
    // types remain the fallback when the backend is unavailable (ADR 0002).
    let dec_cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let dec_transport = crate::decision::UreqTransport { timeout_ms: dec_cfg.timeout_ms };
    let dec_key = std::env::var("OPENROUTER_API_KEY").ok();
    let backend_ready = dec_key.is_some() || dec_cfg.backend == "local";

    let (entity_types, keep) = plan_entities(
        &dec_cfg,
        &dec_transport,
        dec_key.as_deref(),
        db,
        &result.entities,
        backend_ready,
    );

    for (i, entity) in result.entities.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        let item = KnowledgeItemInput {
            r#type: entity_types[i].clone(),
            title: entity.name.clone(),
            content: entity.description.clone(),
            tags: None,
            stack_tags: None,
            agent_tags: None,
            project_id: project_id.map(String::from),
            session_ids: session_id.map(String::from),
            plan_ids: None,
            is_global: false,
        };
        if create_knowledge_item(db, &item).is_ok() {
            entity_count += 1;
        }
    }

    let rel_types = plan_relations(
        &dec_cfg,
        &dec_transport,
        dec_key.as_deref(),
        db,
        &result.relationships,
        backend_ready,
    );

    for (i, rel) in result.relationships.iter().enumerate() {
        let from_items = crate::knowledge::search_knowledge(
            db,
            &rel.source,
            5,
        ).unwrap_or_default();
        let to_items = crate::knowledge::search_knowledge(
            db,
            &rel.target,
            5,
        ).unwrap_or_default();

        if let (Some(from), Some(to)) = (from_items.first(), to_items.first()) {
            if add_knowledge_relation(db, &from.id, &to.id, &rel_types[i]).is_ok() {
                rel_count += 1;
            }
        }
    }

    Ok((entity_count, rel_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn persist_empty_extraction_is_noop() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
            .unwrap();
        let result = ExtractionResult {
            entities: vec![],
            relationships: vec![],
        };
        let (e, r) = persist_extraction(&conn, &result, Some("s1"), None).unwrap();
        assert_eq!((e, r), (0, 0));
    }

    struct CountingTransport {
        calls: std::sync::atomic::AtomicU32,
        payload: serde_json::Value,
    }
    impl crate::decision::DecisionTransport for CountingTransport {
        fn post(
            &self,
            _url: &str,
            _key: Option<&str>,
            _body: &serde_json::Value,
        ) -> Result<serde_json::Value, crate::decision::DecisionError> {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.payload.clone())
        }
    }

    /// R-4: N entities cost exactly 2 requests (types + gate), not 2·N.
    #[test]
    fn plan_entities_batches_into_two_requests() {
        use std::sync::atomic::Ordering;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
            .unwrap();
        let cfg = crate::decision::DecisionConfig::default();
        let t = CountingTransport {
            calls: std::sync::atomic::AtomicU32::new(0),
            payload: serde_json::json!({
                "model": "m",
                "answers": {
                    "entity_type_0": {"type":"choice","choice":"file",
                        "probabilities":{"file":0.9,"error":0.1},"confidence":0.9},
                    "entity_type_1": {"type":"choice","choice":"error",
                        "probabilities":{"file":0.2,"error":0.8},"confidence":0.8},
                    "real_0": {"type":"noul","noul":0.9},
                    "real_1": {"type":"noul","noul":0.1}
                },
                "usage": {"input_tokens": 4, "cost": 0.0}
            }),
        };
        let entities = vec![
            ExtractionEntity { name: "a".into(), r#type: "file".into(), description: "A".into(), confidence: 1.0 },
            ExtractionEntity { name: "b".into(), r#type: "file".into(), description: "B".into(), confidence: 1.0 },
        ];
        let (types, keep) = plan_entities(&cfg, &t, Some("k"), &conn, &entities, true);
        assert_eq!(types, vec!["file".to_string(), "error".to_string()]);
        assert_eq!(keep, vec![true, false], "gate below review threshold drops #1");
        assert_eq!(
            t.calls.load(Ordering::SeqCst),
            2,
            "one type request + one gate request for both entities"
        );
    }

    /// R-4: N relations cost one request, not N.
    #[test]
    fn plan_relations_batches_into_one_request() {
        use std::sync::atomic::Ordering;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
            .unwrap();
        let cfg = crate::decision::DecisionConfig::default();
        let t = CountingTransport {
            calls: std::sync::atomic::AtomicU32::new(0),
            payload: serde_json::json!({
                "model": "m",
                "answers": {
                    "rel_type_0": {"type":"choice","choice":"caused_by",
                        "probabilities":{"caused_by":0.9,"fixed_by":0.1},"confidence":0.9},
                    "rel_type_1": {"type":"choice","choice":"requires",
                        "probabilities":{"requires":0.8,"extends":0.2},"confidence":0.8}
                },
                "usage": {"input_tokens": 4, "cost": 0.0}
            }),
        };
        let rels = vec![
            ExtractionRelation { source: "a".into(), target: "b".into(), relation_type: "requires".into(), evidence: "e".into(), confidence: 1.0 },
            ExtractionRelation { source: "c".into(), target: "d".into(), relation_type: "extends".into(), evidence: "e".into(), confidence: 1.0 },
        ];
        let types = plan_relations(&cfg, &t, Some("k"), &conn, &rels, true);
        assert_eq!(types, vec!["caused_by".to_string(), "requires".to_string()]);
        assert_eq!(t.calls.load(Ordering::SeqCst), 1, "one request for both relations");
    }
}

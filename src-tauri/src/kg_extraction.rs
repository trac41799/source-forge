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

    for entity in &result.entities {
        let describe = format!("{}: {}", entity.name, entity.description);

        let mut etype = entity.r#type.clone();
        if let Ok(Some((t, _))) =
            crate::decision::choose_entity_type(&dec_cfg, &dec_transport, dec_key.as_deref(), &describe, Some(db))
        {
            etype = t;
        }

        // Gate: skip entities the model judges not to be real reusable knowledge.
        if backend_ready {
            if let Ok(Some(p)) = crate::decision::judge(
                &dec_cfg,
                &dec_transport,
                dec_key.as_deref(),
                &serde_json::json!(describe),
                "Is this a real, reusable code entity worth storing as knowledge?",
                "real",
                Some(db),
            ) {
                if p < dec_cfg.review_threshold {
                    continue;
                }
            }
        }

        let item = KnowledgeItemInput {
            r#type: etype,
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

    for rel in &result.relationships {
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

        let mut rtype = rel.relation_type.clone();
        if let Ok(Some((t, _))) = crate::decision::choose_relation_type(
            &dec_cfg,
            &dec_transport,
            dec_key.as_deref(),
            &format!("{} → {}: {}", rel.source, rel.target, rel.evidence),
            Some(db),
        ) {
            rtype = t;
        }

        if let (Some(from), Some(to)) = (from_items.first(), to_items.first()) {
            if add_knowledge_relation(db, &from.id, &to.id, &rtype).is_ok() {
                rel_count += 1;
            }
        }
    }

    Ok((entity_count, rel_count))
}

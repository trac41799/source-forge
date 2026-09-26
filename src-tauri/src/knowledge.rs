use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: String,
    pub r#type: String,
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub stack_tags: Option<String>,
    pub agent_tags: Option<String>,
    pub project_id: Option<String>,
    pub session_ids: Option<String>,
    pub plan_ids: Option<String>,
    pub confidence: f64,
    pub confirmation_count: i64,
    pub is_global: bool,
    pub first_seen: String,
    pub last_confirmed: String,
    pub status: String,
    pub pending_task_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRelation {
    pub from_id: String,
    pub to_id: String,
    pub relation_type: String,
    pub created_at: String,
    pub trigram_tag: Option<String>,
    pub hexagram_tag: Option<String>,
    pub wuxing_cycle: Option<String>,
    pub bagua_confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQuery {
    pub q: Option<String>,
    pub stack: Option<String>,
    pub agent: Option<String>,
    pub project_id: Option<String>,
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub min_confidence: Option<f64>,
    pub is_global: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItemInput {
    pub r#type: String,
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub stack_tags: Option<String>,
    pub agent_tags: Option<String>,
    pub project_id: Option<String>,
    pub session_ids: Option<String>,
    pub plan_ids: Option<String>,
    pub is_global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItemUpdate {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<String>,
    pub stack_tags: Option<String>,
    pub agent_tags: Option<String>,
    pub confidence: Option<f64>,
    pub status: Option<String>,
}

fn row_to_knowledge_item(row: &rusqlite::Row) -> rusqlite::Result<KnowledgeItem> {
    Ok(KnowledgeItem {
        id: row.get(0)?,
        r#type: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        tags: row.get(4)?,
        stack_tags: row.get(5)?,
        agent_tags: row.get(6)?,
        project_id: row.get(7)?,
        session_ids: row.get(8)?,
        plan_ids: row.get(9)?,
        confidence: row.get(10)?,
        confirmation_count: row.get(11)?,
        is_global: row.get::<_, i64>(12)? == 1,
        first_seen: row.get(13)?,
        last_confirmed: row.get(14)?,
        status: row.get(15)?,
        pending_task_data: row.get(16)?,
    })
}

pub fn create_knowledge_item(
    db: &Connection,
    item: &KnowledgeItemInput,
) -> Result<KnowledgeItem, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO knowledge_items (id, type, title, content, tags, stack_tags, agent_tags, project_id, session_ids, plan_ids, confidence, confirmation_count, is_global, first_seen, last_confirmed, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0.5, 1, ?11, ?12, ?12, 'active')",
        rusqlite::params![
            id,
            item.r#type,
            item.title,
            item.content,
            item.tags,
            item.stack_tags,
            item.agent_tags,
            item.project_id,
            item.session_ids,
            item.plan_ids,
            item.is_global as i64,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(KnowledgeItem {
        id,
        r#type: item.r#type.clone(),
        title: item.title.clone(),
        content: item.content.clone(),
        tags: item.tags.clone(),
        stack_tags: item.stack_tags.clone(),
        agent_tags: item.agent_tags.clone(),
        project_id: item.project_id.clone(),
        session_ids: item.session_ids.clone(),
        plan_ids: item.plan_ids.clone(),
        confidence: 0.5,
        confirmation_count: 1,
        is_global: item.is_global,
        first_seen: now.clone(),
        last_confirmed: now,
        status: "active".to_string(),
        pending_task_data: None,
    })
}

pub fn get_knowledge_items(
    db: &Connection,
    query: &KnowledgeQuery,
) -> Result<Vec<KnowledgeItem>, String> {
    let mut conditions: Vec<String> = Vec::new();

    if let Some(ref stack) = query.stack {
        conditions.push(format!("stack_tags LIKE '%{}%'", stack));
    }
    if let Some(ref agent) = query.agent {
        conditions.push(format!("agent_tags LIKE '%{}%'", agent));
    }
    if let Some(ref pid) = query.project_id {
        conditions.push(format!("project_id = '{}'", pid));
    }
    if let Some(ref t) = query.r#type {
        conditions.push(format!("type = '{}'", t));
    }
    if let Some(ref s) = query.status {
        conditions.push(format!("status = '{}'", s));
    }
    if let Some(min_conf) = query.min_confidence {
        conditions.push(format!("confidence >= {}", min_conf));
    }
    if let Some(global) = query.is_global {
        conditions.push(format!("is_global = {}", global as i64));
    }
    if let Some(ref q) = query.q {
        conditions.push(format!(
            "(title LIKE '%{}%' OR content LIKE '%{}%' OR tags LIKE '%{}%')",
            q, q, q
        ));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);

    let sql = format!(
        "SELECT id, type, title, content, tags, stack_tags, agent_tags, project_id, session_ids, plan_ids, confidence, confirmation_count, is_global, first_seen, last_confirmed, status, pending_task_data FROM knowledge_items {} ORDER BY last_confirmed DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let items = stmt
        .query_map([], row_to_knowledge_item)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(items)
}

pub fn update_knowledge_item(
    db: &Connection,
    id: &str,
    updates: &KnowledgeItemUpdate,
) -> Result<KnowledgeItem, String> {
    let now = Utc::now().to_rfc3339();
    let mut sets: Vec<String> = Vec::new();

    if updates.title.is_some() {
        sets.push(format!("title = '{}'", updates.title.as_ref().unwrap()));
    }
    if updates.content.is_some() {
        sets.push(format!(
            "content = '{}'",
            updates.content.as_ref().unwrap()
        ));
    }
    if updates.tags.is_some() {
        sets.push(format!("tags = '{}'", updates.tags.as_ref().unwrap()));
    }
    if updates.stack_tags.is_some() {
        sets.push(format!(
            "stack_tags = '{}'",
            updates.stack_tags.as_ref().unwrap()
        ));
    }
    if updates.agent_tags.is_some() {
        sets.push(format!(
            "agent_tags = '{}'",
            updates.agent_tags.as_ref().unwrap()
        ));
    }
    if let Some(c) = updates.confidence {
        sets.push(format!("confidence = {}", c));
    }
    if let Some(ref s) = updates.status {
        sets.push(format!("status = '{}'", s));
    }
    sets.push(format!("last_confirmed = '{}'", now));

    if !sets.is_empty() {
        let sql = format!(
            "UPDATE knowledge_items SET {} WHERE id = '{}'",
            sets.join(", "),
            id
        );
        db.execute(&sql, []).map_err(|e| e.to_string())?;
    }

    db.query_row(
        "SELECT id, type, title, content, tags, stack_tags, agent_tags, project_id, session_ids, plan_ids, confidence, confirmation_count, is_global, first_seen, last_confirmed, status, pending_task_data FROM knowledge_items WHERE id = ?1",
        rusqlite::params![id],
        row_to_knowledge_item,
    )
    .map_err(|e| e.to_string())
}

pub fn delete_knowledge_item(db: &Connection, id: &str) -> Result<(), String> {
    db.execute(
        "DELETE FROM knowledge_relations WHERE from_id = ?1 OR to_id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;

    db.execute(
        "DELETE FROM knowledge_items WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn compound_knowledge(
    db: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<KnowledgeItem>, String> {
    // M4 (spec R41): decision-derived same-insight confidence when available.
    let dec_cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let dec_transport = crate::decision::UreqTransport { timeout_ms: dec_cfg.timeout_ms };
    let dec_key = std::env::var("OPENROUTER_API_KEY").ok();
    let dec_ready = dec_key.is_some() || dec_cfg.backend == "local";

    let where_clause = if let Some(pid) = project_id {
        format!("WHERE project_id = '{}'", pid)
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT id, type, title, content, tags, stack_tags, agent_tags, project_id, session_ids, plan_ids, confidence, confirmation_count, is_global, first_seen, last_confirmed, status, pending_task_data FROM knowledge_items {} ORDER BY type, title",
        where_clause
    );

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let items: Vec<KnowledgeItem> = stmt
        .query_map([], row_to_knowledge_item)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut merged = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let current = &items[i];
        let mut combined_content = current.content.clone();
        let mut total_confirmations = current.confirmation_count;
        let mut weighted_confidence = current.confidence * current.confirmation_count as f64;
        let mut j = i + 1;

        while j < items.len() {
            let next = &items[j];
            if next.r#type == current.r#type {
                let kw_current: Vec<&str> = current
                    .title
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .collect();
                let kw_next: Vec<&str> = next
                    .title
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .collect();
                let shared = kw_current.iter().filter(|k| kw_next.contains(k)).count();

                if shared > 0 {
                    combined_content = format!("{}\n---\n{}", combined_content, next.content);
                    total_confirmations += next.confirmation_count;
                    weighted_confidence += next.confidence * next.confirmation_count as f64;

                    let _ = db.execute(
                        "DELETE FROM knowledge_items WHERE id = ?1",
                        rusqlite::params![next.id],
                    );
                    j += 1;
                    continue;
                }
            }
            break;
        }

        let mut new_confidence = if total_confirmations > 0 {
            let base = weighted_confidence / total_confirmations as f64;
            let recency = Utc::now()
                .signed_duration_since(
                    current
                        .last_confirmed
                        .parse::<chrono::DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                )
                .num_seconds() as f64;
            let recency_factor = 1.0 + (1.0 / (1.0 + (recency / 3600.0).exp()));
            (base * 0.7 + current.confidence * 0.3 * recency_factor).min(1.0)
        } else {
            current.confidence
        };

        // M4 (spec R41): blend decision-derived same-insight confidence.
        if j > i + 1 && dec_ready {
            if let Ok(Some(p)) = crate::decision::judge(
                &dec_cfg,
                &dec_transport,
                dec_key.as_deref(),
                &serde_json::json!({ "a": current.content, "b": combined_content }),
                "Do these knowledge items express the same insight?",
                "same",
                Some(db),
            ) {
                new_confidence = (new_confidence * 0.5 + p * 0.5).min(1.0);
            }
        }

        let now = Utc::now().to_rfc3339();
        if j > i + 1 {
            db.execute(
                "UPDATE knowledge_items SET content = ?1, confidence = ?2, confirmation_count = ?3, last_confirmed = ?4 WHERE id = ?5",
                rusqlite::params![combined_content, new_confidence, total_confirmations, now, current.id],
            )
            .map_err(|e| e.to_string())?;
        }

        let mut item = current.clone();
        if j > i + 1 {
            item.content = combined_content;
            item.confidence = new_confidence;
            item.confirmation_count = total_confirmations;
            item.last_confirmed = now;
        }
        merged.push(item);
        i = j;
    }

    Ok(merged)
}

pub fn add_knowledge_relation(
    db: &Connection,
    from_id: &str,
    to_id: &str,
    relation_type: &str,
) -> Result<KnowledgeRelation, String> {
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT OR REPLACE INTO knowledge_relations (from_id, to_id, relation_type, created_at, trigram_tag, hexagram_tag, wuxing_cycle, bagua_confidence, relation_multivector) VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, NULL, NULL)",
        rusqlite::params![from_id, to_id, relation_type, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(KnowledgeRelation {
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
        relation_type: relation_type.to_string(),
        created_at: now,
        trigram_tag: None,
        hexagram_tag: None,
        wuxing_cycle: None,
        bagua_confidence: None,
    })
}

pub fn get_knowledge_relations(
    db: &Connection,
    from_id: &str,
) -> Result<Vec<KnowledgeRelation>, String> {
    let mut stmt = db
        .prepare(
            "SELECT from_id, to_id, relation_type, created_at, trigram_tag, hexagram_tag, wuxing_cycle, bagua_confidence FROM knowledge_relations WHERE from_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let relations = stmt
        .query_map(rusqlite::params![from_id], |row| {
            Ok(KnowledgeRelation {
                from_id: row.get(0)?,
                to_id: row.get(1)?,
                relation_type: row.get(2)?,
                created_at: row.get(3)?,
                trigram_tag: row.get(4)?,
                hexagram_tag: row.get(5)?,
                wuxing_cycle: row.get(6)?,
                bagua_confidence: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(relations)
}

pub fn search_knowledge(
    db: &Connection,
    q: &str,
    limit: i64,
) -> Result<Vec<KnowledgeItem>, String> {
    let query = KnowledgeQuery {
        q: Some(q.to_string()),
        stack: None,
        agent: None,
        project_id: None,
        r#type: None,
        status: None,
        min_confidence: None,
        is_global: None,
        limit: Some(limit),
        offset: Some(0),
    };
    get_knowledge_items(db, &query)
}

pub fn get_knowledge_stats(
    db: &Connection,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let where_clause = if let Some(pid) = project_id {
        format!("WHERE project_id = '{}'", pid)
    } else {
        String::new()
    };

    let total: i64 = db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM knowledge_items {}",
                where_clause
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let avg_conf: Option<f64> = db
        .query_row(
            &format!(
                "SELECT AVG(confidence) FROM knowledge_items {}",
                where_clause
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = db
        .prepare(&format!(
            "SELECT type, COUNT(*) as cnt FROM knowledge_items {} GROUP BY type",
            where_clause
        ))
        .map_err(|e| e.to_string())?;

    let by_type: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "type": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt2 = db
        .prepare(&format!(
            "SELECT status, COUNT(*) as cnt FROM knowledge_items {} GROUP BY status",
            where_clause
        ))
        .map_err(|e| e.to_string())?;

    let by_status: Vec<serde_json::Value> = stmt2
        .query_map([], |row| {
            Ok(serde_json::json!({
                "status": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let query = if where_clause.is_empty() {
        "SELECT stack_tags, COUNT(*) as cnt FROM knowledge_items WHERE stack_tags IS NOT NULL GROUP BY stack_tags".to_string()
    } else {
        format!(
            "SELECT stack_tags, COUNT(*) as cnt FROM knowledge_items {} AND stack_tags IS NOT NULL GROUP BY stack_tags",
            where_clause
        )
    };
    let mut stmt3 = db.prepare(&query).map_err(|e| e.to_string())?;

    let by_stack: Vec<serde_json::Value> = stmt3
        .query_map([], |row| {
            Ok(serde_json::json!({
                "stack": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "total": total,
        "avg_confidence": avg_conf.unwrap_or(0.0),
        "by_type": by_type,
        "by_status": by_status,
        "by_stack": by_stack,
    }))
}

pub fn get_compounder_status(
    db: &Connection,
    project_id: Option<&str>,
) -> Result<CompounderStatus, String> {
    let total_items: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM knowledge_items",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let last_run: Option<String> = db
        .query_row(
            "SELECT last_confirmed FROM knowledge_items ORDER BY last_confirmed DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let items_since_last_run: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM knowledge_items WHERE last_confirmed >= datetime('now', '-1 day')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let health = if let Some(ref lr) = last_run {
        let recent: bool = db
            .query_row(
                "SELECT COUNT(*) FROM knowledge_items WHERE last_confirmed >= datetime('now', '-1 day') AND last_confirmed = ?1",
                [lr],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) > 0;
        if recent { "ok".to_string() } else { "stale".to_string() }
    } else {
        "stale".to_string()
    };

    let total_runs: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT session_ids) FROM knowledge_items WHERE session_ids IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let sessions_processed: i64 = total_runs;

    let knowledge_items_created: i64 = total_items;

    let confidence_avg: f64 = db
        .query_row(
            "SELECT COALESCE(AVG(confidence), 0) FROM knowledge_items",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let contradictions_found: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM knowledge_relations WHERE relation_type = 'contradicts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(CompounderStatus {
        last_run,
        items_since_last_run,
        total_items,
        health,
        total_runs,
        flywheel: FlywheelStats {
            sessions_processed,
            knowledge_items_created,
            confidence_avg,
            contradictions_found,
        },
    })
}

// === W5.A: Knowledge Compounder ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompounderCandidate {
    pub raw_text: String,
    pub category: String,
    pub evidence_count: i64,
    pub source_event_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompounderLlmItem {
    title: String,
    content: String,
    category: String,
    confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightWarning {
    pub id: String,
    pub title: String,
    pub content: String,
    pub confidence: f64,
    pub confirmation_count: i64,
    pub stack_tags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompounderStatus {
    pub last_run: Option<String>,
    pub items_since_last_run: i64,
    pub total_items: i64,
    pub health: String,
    pub total_runs: i64,
    pub flywheel: FlywheelStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlywheelStats {
    pub sessions_processed: i64,
    pub knowledge_items_created: i64,
    pub confidence_avg: f64,
    pub contradictions_found: i64,
}

pub async fn run_compounder(
    db: &Connection,
    session_id: &str,
    project_id: Option<&str>,
) -> Result<Vec<KnowledgeItem>, String> {
    let candidates = pass1_local_prepass(db, session_id)?;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err("No OpenRouter API key configured".to_string());
    }

    let prompt = build_compounder_prompt(&candidates);
    let request = crate::intelligence::OpenRouterRequest {
        prompt,
        model: None,
        priority: crate::intelligence::Priority::Normal,
        max_tokens: Some(2048),
        temperature: Some(0.3),
    };

    let resp = crate::intelligence::invoke_with_backoff(request, &api_key, 3)
        .await
        .map_err(|e| e.to_string())?;

    let parsed_items = parse_compounder_response(&resp.content);
    if parsed_items.is_empty() {
        return Ok(Vec::new());
    }

    // M1 (spec R11): re-tag categories via the decision layer; the LLM category
    // remains the fallback when the backend is unavailable (ADR 0002).
    let dec_cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let dec_transport = crate::decision::UreqTransport { timeout_ms: dec_cfg.timeout_ms };
    let dec_key = std::env::var("OPENROUTER_API_KEY").ok();

    let existing_query = KnowledgeQuery {
        q: None,
        stack: None,
        agent: None,
        project_id: project_id.map(String::from),
        r#type: None,
        status: Some("active".to_string()),
        min_confidence: None,
        is_global: None,
        limit: Some(500),
        offset: Some(0),
    };
    let existing = get_knowledge_items(db, &existing_query)?;

    let mut out: Vec<KnowledgeItem> = Vec::new();
    for mut cand in parsed_items {
        if let Ok(Some((cat, _))) = crate::decision::choose_category(
            &dec_cfg,
            &dec_transport,
            dec_key.as_deref(),
            &cand.content,
            Some(db),
        ) {
            cand.category = cat;
        }
        if let Some(matched) = find_jaccard_match(&cand, &existing, 0.7) {
            let new_conf = weighted_confidence(
                matched.confidence,
                matched.confirmation_count,
                cand.confidence,
            );
            let merged_content = if cand.category != matched.r#type {
                format!(
                    "{}\n---\n[{}] {}",
                    matched.content, cand.category, cand.content
                )
            } else {
                cand.content.clone()
            };
            let updates = KnowledgeItemUpdate {
                title: Some(cand.title.clone()),
                content: Some(merged_content),
                tags: None,
                stack_tags: None,
                agent_tags: None,
                confidence: Some(new_conf),
                status: None,
            };
            let updated = update_knowledge_item(db, &matched.id, &updates)?;
            out.push(updated);
        } else {
            let input = KnowledgeItemInput {
                r#type: cand.category.clone(),
                title: cand.title.clone(),
                content: cand.content.clone(),
                tags: None,
                stack_tags: None,
                agent_tags: None,
                project_id: project_id.map(String::from),
                session_ids: Some(session_id.to_string()),
                plan_ids: None,
                is_global: false,
            };
            let created = create_knowledge_item(db, &input)?;
            out.push(created);
        }
    }

    let _ = detect_and_record_contradictions(db, &out, &existing);

    Ok(out)
}

fn build_compounder_prompt(candidates: &[CompounderCandidate]) -> String {
    let body = candidates
        .iter()
        .map(|c| {
            format!(
                "- [{}] (evidence={}, conf={:.2}) {}",
                c.category, c.evidence_count, c.confidence, c.raw_text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a knowledge extraction engine for the Agent Control Center.\n\
         Given the following candidate patterns observed in a coding session, extract 2-5 \
         high-signal, reusable knowledge items. Each item should be concise, generalizable, \
         and actionable. Output strictly a JSON array of objects with shape:\n\
         [{{\"title\": string, \"content\": string, \"category\": string, \"confidence\": number}}]\n\
         Allowed categories: pattern, antipattern, convention, tooling, insight, fact, handoff, correction.\n\
         Confidence must be in [0.0, 1.0].\n\n\
         Candidates:\n{}\n\n\
         Return only the JSON array. No prose, no markdown fence.",
        body
    )
}

fn parse_compounder_response(content: &str) -> Vec<CompounderLlmItem> {
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

    if let Ok(arr) = serde_json::from_str::<Vec<CompounderLlmItem>>(&json_str) {
        return arr;
    }

    if let Ok(obj) = serde_json::from_str::<CompounderLlmItem>(&json_str) {
        return vec![obj];
    }

    if let Some(start) = json_str.find('[') {
        if let Some(end) = json_str.rfind(']') {
            if end > start {
                if let Ok(arr) =
                    serde_json::from_str::<Vec<CompounderLlmItem>>(&json_str[start..=end])
                {
                    return arr;
                }
            }
        }
    }

    Vec::new()
}

fn weighted_confidence(existing_conf: f64, existing_count: i64, new_conf: f64) -> f64 {
    let total = existing_count as f64 + 1.0;
    let weighted = (existing_conf * existing_count as f64 + new_conf) / total;
    weighted.clamp(0.0, 1.0)
}

fn pass1_local_prepass(
    db: &Connection,
    session_id: &str,
) -> Result<Vec<CompounderCandidate>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, event_type, target, lines_added, lines_removed
             FROM events WHERE session_id = ?1 ORDER BY timestamp",
        )
        .map_err(|e| e.to_string())?;

    let events: Vec<(String, String, Option<String>, Option<i64>, Option<i64>)> = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut file_counts: std::collections::HashMap<String, (i64, Vec<String>)> =
        std::collections::HashMap::new();
    let mut edit_churn: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    let mut command_count: i64 = 0;
    let mut error_markers: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for (eid, etype, target, la, lr) in &events {
        match etype.as_str() {
            "file_edit" | "file_write" | "file_read" | "edit" | "write" | "read" => {
                if let Some(path) = target {
                    let entry = file_counts
                        .entry(path.clone())
                        .or_insert((0, Vec::new()));
                    entry.0 += 1;
                    if entry.1.len() < 5 {
                        entry.1.push(eid.clone());
                    }
                    let churn = edit_churn.entry(path.clone()).or_insert((0, 0));
                    churn.0 += la.unwrap_or(0);
                    churn.1 += lr.unwrap_or(0);
                }
            }
            "command" | "shell" | "bash" | "run_command" => {
                command_count += 1;
            }
            "error" | "agent_output" => {
                if let Some(t) = target {
                    let key: String = t.chars().take(80).collect();
                    *error_markers.entry(key).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }

    let mut candidates: Vec<CompounderCandidate> = Vec::new();

    let mut files_sorted: Vec<(String, (i64, Vec<String>))> =
        file_counts.into_iter().collect();
    files_sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (path, (count, eids)) in files_sorted.into_iter().take(10) {
        if count < 2 {
            continue;
        }
        let churn = edit_churn.get(&path).copied().unwrap_or((0, 0));
        let category = if churn.0 + churn.1 > 200 && count >= 3 {
            "antipattern"
        } else {
            "pattern"
        };
        let raw = format!(
            "File {} touched {} times (added={}, removed={})",
            path, count, churn.0, churn.1
        );
        let confidence = (count as f64 / 10.0).clamp(0.2, 0.95);
        candidates.push(CompounderCandidate {
            raw_text: raw,
            category: category.to_string(),
            evidence_count: count,
            source_event_ids: eids,
            confidence,
        });
    }

    for (marker, count) in error_markers.into_iter() {
        if count >= 2 {
            candidates.push(CompounderCandidate {
                raw_text: format!("Recurring error signature: \"{}\"", marker),
                category: "antipattern".to_string(),
                evidence_count: count,
                source_event_ids: vec![],
                confidence: (count as f64 / 6.0).clamp(0.3, 0.9),
            });
        }
    }

    if command_count >= 5 {
        candidates.push(CompounderCandidate {
            raw_text: format!(
                "Session ran {} shell commands; candidate for tooling automation",
                command_count
            ),
            category: "tooling".to_string(),
            evidence_count: command_count,
            source_event_ids: vec![],
            confidence: (command_count as f64 / 20.0).clamp(0.3, 0.85),
        });
    }

    candidates.truncate(20);
    Ok(candidates)
}

pub fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let a_tokens: std::collections::HashSet<String> = a
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() > 2)
        .collect();
    let b_tokens: std::collections::HashSet<String> = b
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() > 2)
        .collect();
    if a_tokens.is_empty() && b_tokens.is_empty() {
        return 0.0;
    }
    let intersection = a_tokens.intersection(&b_tokens).count() as f64;
    let union = a_tokens.union(&b_tokens).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn find_jaccard_match<'a>(
    candidate: &CompounderLlmItem,
    existing: &'a [KnowledgeItem],
    threshold: f64,
) -> Option<&'a KnowledgeItem> {
    let haystack = format!("{} {}", candidate.title, candidate.content);
    existing
        .iter()
        .filter(|e| e.status == "active")
        .find(|e| {
            let needle = format!("{} {}", e.title, e.content);
            jaccard_similarity(&haystack, &needle) >= threshold
        })
}

fn detect_and_record_contradictions(
    db: &Connection,
    new_items: &[KnowledgeItem],
    existing: &[KnowledgeItem],
) -> Result<usize, String> {
    let mut recorded = 0usize;
    let dec_cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let dec_transport = crate::decision::UreqTransport { timeout_ms: dec_cfg.timeout_ms };
    let dec_key = std::env::var("OPENROUTER_API_KEY").ok();
    let antipattern_indicators = ["avoid", "don't", "do not", "never", "bug", "wrong", "bad"];

    for new_item in new_items {
        if new_item.r#type != "antipattern" {
            continue;
        }
        let new_text = format!("{} {}", new_item.title, new_item.content).to_lowercase();
        let new_is_antipattern = antipattern_indicators
            .iter()
            .any(|w| new_text.contains(w));

        if !new_is_antipattern {
            continue;
        }

        for ex in existing {
            if ex.id == new_item.id || ex.status != "active" {
                continue;
            }
            if ex.r#type == "pattern" || ex.r#type == "convention" {
                let ex_text =
                    format!("{} {}", ex.title, ex.content).to_lowercase();
                // M4 (spec R40): decide "contradicts" via noul; Jaccard is the fallback.
                let is_contradiction = match crate::decision::judge(
                    &dec_cfg,
                    &dec_transport,
                    dec_key.as_deref(),
                    &serde_json::json!({
                        "a": format!("{} {}", new_item.title, new_item.content),
                        "b": format!("{} {}", ex.title, ex.content),
                    }),
                    "Do `a` and `b` contradict each other?",
                    "contradicts",
                    Some(db),
                ) {
                    Ok(Some(p)) => p >= dec_cfg.review_threshold,
                    _ => jaccard_similarity(&new_text, &ex_text) >= 0.5,
                };
                if is_contradiction {
                    let now = chrono::Utc::now().to_rfc3339();
                    let res = db.execute(
                        "INSERT OR IGNORE INTO knowledge_relations (from_id, to_id, relation_type, created_at, trigram_tag, hexagram_tag, wuxing_cycle, bagua_confidence, relation_multivector) VALUES (?1, ?2, 'contradicts', ?3, NULL, NULL, NULL, NULL, NULL)",
                        rusqlite::params![new_item.id, ex.id, now],
                    );
                    if res.is_ok() {
                        recorded += 1;
                    }
                }
            }
        }
    }
    Ok(recorded)
}

pub fn get_preflight_warnings(
    db: &Connection,
    project_stack: &str,
    limit: i64,
) -> Result<Vec<PreflightWarning>, String> {
    let like = format!("%{}%", project_stack);
    let mut stmt = db
        .prepare(
            "SELECT id, title, content, confidence, confirmation_count, stack_tags
             FROM knowledge_items
             WHERE status = 'active'
               AND type = 'antipattern'
               AND (stack_tags LIKE ?1 OR stack_tags IS NULL OR stack_tags = '')
             ORDER BY confidence DESC, confirmation_count DESC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let items = stmt
        .query_map(rusqlite::params![like, limit], |row| {
            Ok(PreflightWarning {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                confidence: row.get(3)?,
                confirmation_count: row.get(4)?,
                stack_tags: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(items)
}

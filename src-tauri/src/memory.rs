use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFact {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub user_id: String,
    pub org_id: String,
    pub fact_type: String,
    pub content: String,
    pub embedding: Option<Vec<u8>>,
    pub metadata: Option<String>,
    pub confidence: f64,
    pub access_count: i64,
    pub last_accessed: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFactInput {
    pub agent_id: String,
    pub session_id: String,
    pub user_id: String,
    pub org_id: String,
    pub fact_type: String,
    pub content: String,
    pub metadata: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub turn_number: i64,
    pub state_blob: Vec<u8>,
    pub summary: Option<String>,
    pub token_count: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub org_id: Option<String>,
    pub fact_type: Option<String>,
    pub min_confidence: Option<f64>,
    pub q: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_facts: i64,
    pub total_checkpoints: i64,
    pub by_type: Vec<serde_json::Value>,
    pub avg_confidence: f64,
    pub total_tokens_saved: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub facts: Vec<MemoryFact>,
    pub method: String,
    pub latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub original_tokens: i64,
    pub compressed_tokens: i64,
    pub savings_pct: f64,
    pub summary: String,
    pub phases_applied: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub fact: MemoryFact,
    pub score: f64,
    pub match_type: String,
}

impl MemoryFactInput {
    pub fn to_fact(&self) -> MemoryFact {
        let now = Utc::now().to_rfc3339();
        MemoryFact {
            id: Uuid::new_v4().to_string(),
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            user_id: self.user_id.clone(),
            org_id: self.org_id.clone(),
            fact_type: self.fact_type.clone(),
            content: self.content.clone(),
            embedding: None,
            metadata: self.metadata.clone(),
            confidence: self.confidence.unwrap_or(0.5),
            access_count: 0,
            last_accessed: None,
            created_at: now,
        }
    }
}

fn row_to_memory_fact(row: &rusqlite::Row) -> rusqlite::Result<MemoryFact> {
    Ok(MemoryFact {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        session_id: row.get(2)?,
        user_id: row.get(3)?,
        org_id: row.get(4)?,
        fact_type: row.get(5)?,
        content: row.get(6)?,
        embedding: row.get(7)?,
        metadata: row.get(8)?,
        confidence: row.get(9)?,
        access_count: row.get(10)?,
        last_accessed: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn row_to_checkpoint(row: &rusqlite::Row) -> rusqlite::Result<SessionCheckpoint> {
    Ok(SessionCheckpoint {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        session_id: row.get(2)?,
        turn_number: row.get(3)?,
        state_blob: row.get(4)?,
        summary: row.get(5)?,
        token_count: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub fn create_memory_fact(db: &Connection, input: &MemoryFactInput) -> Result<MemoryFact, String> {
    let fact = input.to_fact();

    db.execute(
        "INSERT INTO memory_facts (id, agent_id, session_id, user_id, org_id, fact_type, content, embedding, metadata, confidence, access_count, last_accessed, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, NULL, ?11)",
        rusqlite::params![
            fact.id,
            fact.agent_id,
            fact.session_id,
            fact.user_id,
            fact.org_id,
            fact.fact_type,
            fact.content,
            fact.embedding,
            fact.metadata,
            fact.confidence,
            fact.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(fact)
}

pub fn get_memory_fact(db: &Connection, id: &str) -> Result<MemoryFact, String> {
    db.query_row(
        "SELECT id, agent_id, session_id, user_id, org_id, fact_type, content, embedding, metadata, confidence, access_count, last_accessed, created_at FROM memory_facts WHERE id = ?1",
        rusqlite::params![id],
        row_to_memory_fact,
    )
    .map_err(|e| e.to_string())
}

fn escape_like_pattern(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn get_memory_facts(db: &Connection, query: &MemoryQuery) -> Result<Vec<MemoryFact>, String> {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref agent_id) = query.agent_id {
        let idx = params.len() + 1;
        conditions.push(format!("agent_id = ?{}", idx));
        params.push(Box::new(agent_id.clone()));
    }
    if let Some(ref session_id) = query.session_id {
        let idx = params.len() + 1;
        conditions.push(format!("session_id = ?{}", idx));
        params.push(Box::new(session_id.clone()));
    }
    if let Some(ref org_id) = query.org_id {
        let idx = params.len() + 1;
        conditions.push(format!("org_id = ?{}", idx));
        params.push(Box::new(org_id.clone()));
    }
    if let Some(ref fact_type) = query.fact_type {
        let idx = params.len() + 1;
        conditions.push(format!("fact_type = ?{}", idx));
        params.push(Box::new(fact_type.clone()));
    }
    if let Some(min_conf) = query.min_confidence {
        let idx = params.len() + 1;
        conditions.push(format!("confidence >= ?{}", idx));
        params.push(Box::new(min_conf));
    }
    if let Some(ref q) = query.q {
        let idx = params.len() + 1;
        conditions.push(format!("content LIKE ?{} ESCAPE '\\'", idx));
        params.push(Box::new(format!("%{}%", escape_like_pattern(q))));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let limit_idx = params.len() + 1;
    params.push(Box::new(limit));
    let offset_idx = params.len() + 1;
    params.push(Box::new(offset));

    let sql = format!(
        "SELECT id, agent_id, session_id, user_id, org_id, fact_type, content, embedding, metadata, confidence, access_count, last_accessed, created_at FROM memory_facts {} ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
        where_clause, limit_idx, offset_idx
    );

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let facts = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            row_to_memory_fact,
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(facts)
}

pub fn update_memory_fact(
    db: &Connection,
    id: &str,
    content: Option<&str>,
    confidence: Option<f64>,
    metadata: Option<&str>,
) -> Result<MemoryFact, String> {
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(c) = content {
        let idx = params.len() + 1;
        sets.push(format!("content = ?{}", idx));
        params.push(Box::new(c.to_string()));
    }
    if let Some(c) = confidence {
        let idx = params.len() + 1;
        sets.push(format!("confidence = ?{}", idx));
        params.push(Box::new(c));
    }
    if let Some(m) = metadata {
        let idx = params.len() + 1;
        sets.push(format!("metadata = ?{}", idx));
        params.push(Box::new(m.to_string()));
    }

    if !sets.is_empty() {
        let id_idx = params.len() + 1;
        params.push(Box::new(id.to_string()));
        let sql = format!(
            "UPDATE memory_facts SET {} WHERE id = ?{}",
            sets.join(", "),
            id_idx
        );
        db.execute(
            &sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        )
        .map_err(|e| e.to_string())?;
    }

    get_memory_fact(db, id)
}

pub fn delete_memory_fact(db: &Connection, id: &str) -> Result<(), String> {
    db.execute(
        "DELETE FROM memory_facts WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn record_access(db: &Connection, id: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    db.execute(
        "UPDATE memory_facts SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn create_session_checkpoint(
    db: &Connection,
    agent_id: &str,
    session_id: &str,
    turn_number: i64,
    state_blob: &[u8],
    summary: Option<&str>,
    token_count: Option<i64>,
) -> Result<SessionCheckpoint, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO session_checkpoints (id, agent_id, session_id, turn_number, state_blob, summary, token_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, agent_id, session_id, turn_number, state_blob, summary, token_count, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(SessionCheckpoint {
        id,
        agent_id: agent_id.to_string(),
        session_id: session_id.to_string(),
        turn_number,
        state_blob: state_blob.to_vec(),
        summary: summary.map(String::from),
        token_count,
        created_at: now,
    })
}

pub fn get_latest_checkpoint(
    db: &Connection,
    agent_id: &str,
    session_id: &str,
) -> Result<Option<SessionCheckpoint>, String> {
    let result = db.query_row(
        "SELECT id, agent_id, session_id, turn_number, state_blob, summary, token_count, created_at FROM session_checkpoints WHERE agent_id = ?1 AND session_id = ?2 ORDER BY turn_number DESC LIMIT 1",
        rusqlite::params![agent_id, session_id],
        row_to_checkpoint,
    );

    match result {
        Ok(checkpoint) => Ok(Some(checkpoint)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_session_checkpoints(
    db: &Connection,
    agent_id: &str,
    session_id: &str,
) -> Result<Vec<SessionCheckpoint>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, agent_id, session_id, turn_number, state_blob, summary, token_count, created_at FROM session_checkpoints WHERE agent_id = ?1 AND session_id = ?2 ORDER BY turn_number",
        )
        .map_err(|e| e.to_string())?;

    let checkpoints = stmt
        .query_map(rusqlite::params![agent_id, session_id], row_to_checkpoint)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(checkpoints)
}

pub fn get_memory_stats(db: &Connection, org_id: &str) -> Result<MemoryStats, String> {
    let total_facts: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM memory_facts WHERE org_id = ?1",
            rusqlite::params![org_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let total_checkpoints: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM session_checkpoints WHERE agent_id IN (SELECT DISTINCT agent_id FROM memory_facts WHERE org_id = ?1)",
            rusqlite::params![org_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let avg_confidence: f64 = db
        .query_row(
            "SELECT COALESCE(AVG(confidence), 0.0) FROM memory_facts WHERE org_id = ?1",
            rusqlite::params![org_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = db
        .prepare(
            "SELECT fact_type, COUNT(*) as cnt FROM memory_facts WHERE org_id = ?1 GROUP BY fact_type",
        )
        .map_err(|e| e.to_string())?;

    let by_type: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![org_id], |row| {
            Ok(serde_json::json!({
                "type": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(MemoryStats {
        total_facts,
        total_checkpoints,
        by_type,
        avg_confidence,
        total_tokens_saved: 0,
    })
}

pub fn get_decayed_factor(last_accessed: &Option<String>) -> f64 {
    let Some(ref la) = last_accessed else {
        return 0.3;
    };
    let Ok(accessed) = la.parse::<chrono::DateTime<Utc>>() else {
        return 0.5;
    };
    let hours = Utc::now()
        .signed_duration_since(accessed)
        .num_hours() as f64;
    if hours < 1.0 {
        1.5
    } else if hours < 6.0 {
        1.2
    } else if hours < 24.0 {
        1.0
    } else if hours < 72.0 {
        0.7
    } else if hours < 168.0 {
        0.5
    } else {
        0.3
    }
}

fn sanitize_fts5_query(q: &str) -> Option<String> {
    let tokens: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"", w))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

fn append_memory_query_filters(
    query: &MemoryQuery,
    alias: &str,
    conditions: &mut Vec<String>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
) {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{}.", alias)
    };
    if let Some(ref agent_id) = query.agent_id {
        let idx = params.len() + 1;
        conditions.push(format!("{}agent_id = ?{}", prefix, idx));
        params.push(Box::new(agent_id.clone()));
    }
    if let Some(ref session_id) = query.session_id {
        let idx = params.len() + 1;
        conditions.push(format!("{}session_id = ?{}", prefix, idx));
        params.push(Box::new(session_id.clone()));
    }
    if let Some(ref org_id) = query.org_id {
        let idx = params.len() + 1;
        conditions.push(format!("{}org_id = ?{}", prefix, idx));
        params.push(Box::new(org_id.clone()));
    }
    if let Some(ref fact_type) = query.fact_type {
        let idx = params.len() + 1;
        conditions.push(format!("{}fact_type = ?{}", prefix, idx));
        params.push(Box::new(fact_type.clone()));
    }
    if let Some(min_conf) = query.min_confidence {
        let idx = params.len() + 1;
        conditions.push(format!("{}confidence >= ?{}", prefix, idx));
        params.push(Box::new(min_conf));
    }
}

pub fn hybrid_search(
    db: &Connection,
    query: &MemoryQuery,
    q: &str,
    limit: i64,
) -> Result<Vec<SearchResult>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let has_fts: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_facts_fts')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let lower_tokens: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();

    if has_fts {
        if let Some(fts_query) = sanitize_fts5_query(q) {
            let mut conditions: Vec<String> = vec!["memory_facts_fts MATCH ?1".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(fts_query)];
            append_memory_query_filters(query, "f", &mut conditions, &mut params);

            let candidate_limit = limit.saturating_mul(3).max(20);
            let limit_idx = params.len() + 1;
            params.push(Box::new(candidate_limit));

            let sql = format!(
                "SELECT f.id, f.agent_id, f.session_id, f.user_id, f.org_id, f.fact_type, \
                 f.content, f.embedding, f.metadata, f.confidence, f.access_count, \
                 f.last_accessed, f.created_at, (-bm25(memory_facts_fts)) AS bm25_score \
                 FROM memory_facts_fts \
                 JOIN memory_facts f ON f.id = memory_facts_fts.id \
                 WHERE {} \
                 ORDER BY bm25_score DESC \
                 LIMIT ?{}",
                conditions.join(" AND "),
                limit_idx
            );

            let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                    |row| {
                        let fact = row_to_memory_fact(row)?;
                        let bm25_score: f64 = row.get(13)?;
                        Ok((fact, bm25_score))
                    },
                )
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            let mut results: Vec<SearchResult> = rows
                .into_iter()
                .map(|(fact, bm25_score)| {
                    let decay = get_decayed_factor(&fact.last_accessed);
                    let final_score = (bm25_score.max(0.0) * decay * fact.confidence).clamp(0.0, 10.0);
                    let entity_match = fact.metadata.as_ref().is_some_and(|meta_str| {
                        let meta_lower = meta_str.to_lowercase();
                        lower_tokens.iter().any(|kw| meta_lower.contains(kw))
                    });
                    let match_type = if entity_match { "entity" } else { "fts5_bm25" };
                    SearchResult {
                        fact,
                        score: final_score,
                        match_type: match_type.to_string(),
                    }
                })
                .collect();

            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(limit as usize);
            return Ok(results);
        }
    }

    // Fallback: parameterized keyword scan (when FTS5 table is absent or query has no alphanumeric tokens)
    let keywords: Vec<&str> = q.split_whitespace().collect();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let base_sql = "SELECT id, agent_id, session_id, user_id, org_id, fact_type, content, embedding, metadata, confidence, access_count, last_accessed, created_at FROM memory_facts WHERE 1=1";
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    append_memory_query_filters(query, "", &mut conditions, &mut params);

    let where_ext = if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    };

    let sql = format!(
        "{}{} ORDER BY created_at DESC LIMIT ?{}",
        base_sql,
        where_ext,
        params.len() + 1
    );
    params.push(Box::new(limit.saturating_mul(3)));

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let facts = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            row_to_memory_fact,
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut results: Vec<SearchResult> = Vec::new();

    for fact in &facts {
        let content_lower = fact.content.to_lowercase();
        let mut bm25_score = 0.0f64;
        let mut matches = 0;

        for kw in &keywords {
            let kw_lower = kw.to_lowercase();
            if content_lower.contains(&kw_lower) {
                matches += 1;
                let position_factor = if let Some(pos) = content_lower.find(&kw_lower) {
                    1.0 - (pos as f64 / content_lower.len().max(1) as f64) * 0.5
                } else {
                    0.5
                };
                bm25_score += position_factor * 2.0;
            }
        }

        if matches > 0 {
            bm25_score /= keywords.len().max(1) as f64;
        }

        let entity_match = if let Some(ref meta_str) = fact.metadata {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(meta_str) {
                let meta_lower = format!("{:?}", meta).to_lowercase();
                keywords.iter().any(|kw| meta_lower.contains(&kw.to_lowercase()))
            } else {
                false
            }
        } else {
            false
        };

        let decay = get_decayed_factor(&fact.last_accessed);

        if bm25_score > 0.0 || entity_match {
            let raw_score = if entity_match {
                bm25_score * 1.5 + 0.5
            } else {
                bm25_score
            };
            let final_score = (raw_score * decay * fact.confidence).clamp(0.0, 10.0);
            let match_type = if entity_match { "entity" } else { "bm25" };

            results.push(SearchResult {
                fact: fact.clone(),
                score: final_score,
                match_type: match_type.to_string(),
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit as usize);

    Ok(results)
}

pub fn get_context(
    db: &Connection,
    agent_id: &str,
    session_id: &str,
    query: Option<&str>,
    budget: Option<i64>,
) -> Result<String, String> {
    let facts = if let Some(q) = query {
        let mem_query = MemoryQuery {
            agent_id: Some(agent_id.to_string()),
            session_id: Some(session_id.to_string()),
            org_id: None,
            fact_type: None,
            min_confidence: None,
            q: None,
            limit: Some(10),
            offset: Some(0),
        };
        let results = hybrid_search(db, &mem_query, q, 10)?;
        results.into_iter().map(|r| r.fact).collect()
    } else {
        let mem_query = MemoryQuery {
            agent_id: Some(agent_id.to_string()),
            session_id: Some(session_id.to_string()),
            org_id: None,
            fact_type: None,
            min_confidence: None,
            q: None,
            limit: Some(10),
            offset: Some(0),
        };
        get_memory_facts(db, &mem_query)?
    };

    let facts_json: Vec<serde_json::Value> = facts
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": f.fact_type,
                "content": f.content,
                "confidence": f.confidence,
                "created_at": f.created_at,
            })
        })
        .collect();

    let context = format!("Prior knowledge: {}", serde_json::to_string(&facts_json).unwrap_or_default());

    if let Some(b) = budget {
        let approx_tokens = context.len() as i64 / 4;
        if approx_tokens > b {
            let budget_chars = (b * 4) as usize;
            let truncated: String = context.chars().take(budget_chars).collect();
            return Ok(truncated);
        }
    }

    Ok(context)
}

pub struct CircuitBreaker {
    pub failure_count: i64,
    pub last_failure: Option<String>,
    pub cooldown_until: Option<String>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure: None,
            cooldown_until: None,
        }
    }
}

pub fn heuristic_extract(
    message_text: &str,
    agent_id: &str,
    session_id: &str,
    user_id: &str,
    org_id: &str,
) -> Vec<MemoryFactInput> {
    let mut candidates: Vec<MemoryFactInput> = Vec::new();

    let decision_patterns = [
        "I'll use",
        "I will use",
        "Let's use",
        "going to use",
        "We should",
        "We'll use",
    ];
    let constraint_patterns = [
        "Never modify",
        "Do not modify",
        "Must use",
        "Must not",
        "Don't",
        "Do not",
        "Cannot",
        "Must never",
    ];
    let preference_patterns = ["I prefer", "I like", "I think", "We prefer", "Better to use"];
    let pattern_patterns = ["The pattern is", "A common pattern", "The approach", "Standard way"];
    let error_patterns = ["Error:", "Exception:", "Failed:", "panic:", "FATAL:"];

    let text_lower = message_text.to_lowercase();

    let type_info: Vec<(&[&str], &str)> = vec![
        (&decision_patterns, "decision"),
        (&constraint_patterns, "constraint"),
        (&preference_patterns, "preference"),
        (&pattern_patterns, "pattern"),
    ];

    for (patterns, fact_type) in &type_info {
        for pattern in *patterns {
            if text_lower.contains(&pattern.to_lowercase()) {
                let sentences: Vec<&str> = message_text
                    .split(|c: char| c == '.' || c == '!' || c == '?')
                    .collect();
                for sentence in &sentences {
                    if sentence.to_lowercase().contains(&pattern.to_lowercase()) {
                        let confidence = if pattern.contains('\'') || pattern.contains(':') || pattern.len() > 10 {
                            0.8
                        } else {
                            0.6
                        };
                        candidates.push(MemoryFactInput {
                            agent_id: agent_id.to_string(),
                            session_id: session_id.to_string(),
                            user_id: user_id.to_string(),
                            org_id: org_id.to_string(),
                            fact_type: fact_type.to_string(),
                            content: sentence.trim().to_string(),
                            metadata: None,
                            confidence: Some(confidence),
                        });
                        break;
                    }
                }
            }
        }
    }

    for pattern in &error_patterns {
        if text_lower.contains(&pattern.to_lowercase()) {
            for line in message_text.lines() {
                if line.to_lowercase().contains(&pattern.to_lowercase()) {
                    candidates.push(MemoryFactInput {
                        agent_id: agent_id.to_string(),
                        session_id: session_id.to_string(),
                        user_id: user_id.to_string(),
                        org_id: org_id.to_string(),
                        fact_type: "error".to_string(),
                        content: line.trim().to_string(),
                        metadata: None,
                        confidence: Some(0.9),
                    });
                }
            }
        }
    }

    let port_marker = "port ";
    for line in message_text.lines() {
        let line_lower = line.to_lowercase();
        if let Some(idx) = line_lower.find(port_marker) {
            let rest = &line_lower[idx + port_marker.len()..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !num_str.is_empty() {
                candidates.push(MemoryFactInput {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    user_id: user_id.to_string(),
                    org_id: org_id.to_string(),
                    fact_type: "constraint".to_string(),
                    content: format!("Port {}", num_str),
                    metadata: None,
                    confidence: Some(0.85),
                });
            }
        }
    }

    let mut chars = message_text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            let mut name = String::new();
            while let Some(next) = chars.next() {
                if next == '"' { break; }
                name.push(next);
            }
            if let Some(':') = chars.peek() {
                chars.next();
                let mut ver = String::new();
                while let Some(vch) = chars.next() {
                    if vch == ',' || vch == ' ' || vch == '}' || vch == '\n' { break; }
                    ver.push(vch);
                }
                let ver_trimmed = ver.trim().trim_matches('"');
                if ver_trimmed.chars().filter(|&c| c == '.').count() >= 1
                    && ver_trimmed.chars().any(|c| c.is_ascii_digit())
                {
                    candidates.push(MemoryFactInput {
                        agent_id: agent_id.to_string(),
                        session_id: session_id.to_string(),
                        user_id: user_id.to_string(),
                        org_id: org_id.to_string(),
                        fact_type: "constraint".to_string(),
                        content: format!("Version pin: \"{}\": \"{}\"", name, ver_trimmed),
                        metadata: None,
                        confidence: Some(0.9),
                    });
                }
            }
        }
    }

    candidates
}

pub fn extraction_template() -> String {
    r#"Extract critical facts from this agent exchange. Output JSON:

{
  "facts": [
    {
      "type": "decision|constraint|preference|pattern|error|entity",
      "content": "...",
      "confidence": 0.0-1.0,
      "entities": ["function_name", "file_path", "library"]
    }
  ]
}"#
    .to_string()
}

pub fn parse_extraction_response(json_str: &str) -> Vec<MemoryFactInput> {
    let trimmed = json_str.trim();
    let cleaned = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    };

    #[derive(Deserialize)]
    struct ExtractionItem {
        r#type: String,
        content: String,
        confidence: Option<f64>,
        entities: Option<Vec<String>>,
    }

    #[derive(Deserialize)]
    struct ExtractionResponse {
        facts: Vec<ExtractionItem>,
    }

    let items = if let Ok(resp) = serde_json::from_str::<ExtractionResponse>(&cleaned) {
        resp.facts
    } else if let Ok(arr) = serde_json::from_str::<Vec<ExtractionItem>>(&cleaned) {
        arr
    } else if let Some(start) = cleaned.find('[') {
        if let Some(end) = cleaned.rfind(']') {
            if end > start {
                if let Ok(arr) =
                    serde_json::from_str::<Vec<ExtractionItem>>(&cleaned[start..=end])
                {
                    arr
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    items
        .into_iter()
        .map(|item| MemoryFactInput {
            agent_id: String::new(),
            session_id: String::new(),
            user_id: String::new(),
            org_id: String::new(),
            fact_type: item.r#type,
            content: item.content,
            metadata: item.entities.map(|e| serde_json::to_string(&e).unwrap_or_default()),
            confidence: item.confidence,
        })
        .collect()
}

pub fn check_circuit_breaker(cb: &mut CircuitBreaker) -> bool {
    if cb.failure_count >= 5 {
        if let Some(ref cooldown) = cb.cooldown_until {
            if let Ok(cooldown_time) = cooldown.parse::<chrono::DateTime<Utc>>() {
                if Utc::now() < cooldown_time {
                    return false;
                }
            }
        }
        cb.failure_count = 0;
        cb.cooldown_until = None;
    }
    true
}

pub fn record_circuit_failure(cb: &mut CircuitBreaker) {
    cb.failure_count += 1;
    cb.last_failure = Some(Utc::now().to_rfc3339());
    if cb.failure_count >= 5 {
        let cooldown = Utc::now() + chrono::Duration::minutes(2);
        cb.cooldown_until = Some(cooldown.to_rfc3339());
    }
}

pub async fn extract_and_store(
    db: &Connection,
    input: &MemoryFactInput,
    cb: &mut CircuitBreaker,
) -> Result<Option<MemoryFact>, String> {
    if !check_circuit_breaker(cb) {
        return Ok(None);
    }

    let heuristic_candidates = heuristic_extract(
        &input.content,
        &input.agent_id,
        &input.session_id,
        &input.user_id,
        &input.org_id,
    );

    let high_conf: Vec<&MemoryFactInput> = heuristic_candidates
        .iter()
        .filter(|c| c.confidence.unwrap_or(0.0) >= 0.7)
        .collect();

    if high_conf.len() >= 3 {
        let mut stored: Option<MemoryFact> = None;
        for cand in high_conf {
            stored = Some(create_memory_fact(db, cand)?);
        }
        return Ok(stored);
    }

    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if !api_key.is_empty() {
        let prompt = format!(
            "{}\n\nMessage:\n{}",
            extraction_template(),
            input.content
        );

        let request = crate::intelligence::OpenRouterRequest {
            prompt,
            model: None,
            priority: crate::intelligence::Priority::Normal,
            max_tokens: Some(1024u32),
            temperature: Some(0.1),
        };

        match crate::intelligence::invoke_with_backoff(request, &api_key, 2).await {
            Ok(resp) => {
                let llm_facts = parse_extraction_response(&resp.content);
                let mut stored: Option<MemoryFact> = None;
                for mut fact in llm_facts {
                    fact.agent_id = input.agent_id.clone();
                    fact.session_id = input.session_id.clone();
                    fact.user_id = input.user_id.clone();
                    fact.org_id = input.org_id.clone();
                    stored = Some(create_memory_fact(db, &fact)?);
                }
                if stored.is_some() {
                    return Ok(stored);
                }
            }
            Err(_) => {
                record_circuit_failure(cb);
            }
        }
    }

    if !heuristic_candidates.is_empty() {
        let mut stored: Option<MemoryFact> = None;
        for cand in &heuristic_candidates {
            stored = Some(create_memory_fact(db, cand)?);
        }
        return Ok(stored);
    }

    Ok(None)
}

#[derive(Debug)]
pub struct AntiThrashingLock {
    pub strikes: HashMap<String, (i64, bool)>,
}

impl AntiThrashingLock {
    pub fn new() -> Self {
        Self {
            strikes: HashMap::new(),
        }
    }
}

pub fn check_anti_thrashing(
    locks: &mut HashMap<String, (i64, bool)>,
    session_id: &str,
    savings_pct: f64,
) -> bool {
    let entry = locks.entry(session_id.to_string()).or_insert((0, false));
    if entry.1 {
        return false;
    }
    if savings_pct < 10.0 {
        entry.0 += 1;
        if entry.0 >= 2 {
            entry.1 = true;
            return false;
        }
    } else {
        entry.0 = 0;
    }
    true
}

pub fn write_before_compaction(
    db: &Connection,
    messages: &[&str],
    agent_id: &str,
    session_id: &str,
    user_id: &str,
    org_id: &str,
) -> Result<usize, String> {
    let mut count = 0usize;
    for msg in messages {
        let candidates = heuristic_extract(msg, agent_id, session_id, user_id, org_id);
        for cand in candidates {
            let _ = create_memory_fact(db, &cand);
            count += 1;
        }
    }
    Ok(count)
}

pub async fn compress_context(
    db: &Connection,
    agent_id: &str,
    session_id: &str,
    user_id: &str,
    org_id: &str,
    messages: &[&str],
    current_tokens: i64,
    max_tokens: i64,
    previous_summary: Option<&str>,
    locks: &mut HashMap<String, (i64, bool)>,
) -> Result<CompressionResult, String> {
    let mut phases: Vec<String> = Vec::new();
    let original_tokens = current_tokens;

    if !check_anti_thrashing(locks, session_id, 0.0) {
        if let Some(l) = locks.get(session_id) {
            if l.1 {
                return Err("Thrashing lock active for session".to_string());
            }
        }
    }

    let mut compressed: Vec<String> = Vec::new();
    for msg in messages {
        if msg.len() > 200 {
            compressed.push("[tool output truncated]".to_string());
        } else {
            compressed.push(msg.to_string());
        }
    }
    phases.push("prune_tool_outputs".to_string());

    let head_count = 3usize.min(compressed.len());
    let tail_budget = (max_tokens as f64 * 0.3) as usize;
    let mut tail_count = 0usize;
    let mut tail_chars = 0usize;

    for msg in compressed.iter().rev() {
        tail_chars += msg.len() / 4 + 1;
        tail_count += 1;
        if tail_chars >= tail_budget {
            break;
        }
    }

    let middle_start = head_count;
    let middle_end = compressed.len().saturating_sub(tail_count);

    phases.push("determine_boundaries".to_string());

    let middle_messages: Vec<&str> = if middle_end > middle_start {
        compressed[middle_start..middle_end]
            .iter()
            .map(|s| s.as_str())
            .collect()
    } else {
        Vec::new()
    };

    let _ = write_before_compaction(db, &middle_messages, agent_id, session_id, user_id, org_id);

    phases.push("write_before_compaction".to_string());

    let summary = if middle_messages.is_empty() {
        previous_summary.unwrap_or("").to_string()
    } else {
        let middle_text = middle_messages.join("\n");
        let summary_prompt = format!(
            "Summarize the following conversation in structured format:\n\
             - Current goal: [task_description]\n\
             - Completed: [list_of_completed_subtasks]\n\
             - In progress: [current_work]\n\
             - Key decisions: [decisions_made]\n\
             - Constraints: [hard_constraints]\n\
             - Dependencies: [cross_task_dependencies]\n\
             - Files touched: [list_of_files]\n\
             - Errors encountered: [errors_and_lessons]\n\n\
             Previous summary:\n{}\n\n\
             Messages to compress:\n{}",
            previous_summary.unwrap_or("(none)"),
            middle_text
        );

        let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        if api_key.is_empty() || middle_messages.len() < 2 {
            "[compression skipped: no LLM available or too few messages]".to_string()
        } else {
            let request = crate::intelligence::OpenRouterRequest {
                prompt: summary_prompt,
                model: None,
                priority: crate::intelligence::Priority::Normal,
                max_tokens: Some(1024u32),
                temperature: Some(0.3),
            };

            match crate::intelligence::invoke_with_backoff(request, &api_key, 2).await {
                Ok(resp) => resp.content,
                Err(e) => format!("[compression failed: {}]", e),
            }
        }
    };

    phases.push("generate_summary".to_string());

    let mut reassembled: Vec<String> = Vec::new();
    for i in 0..head_count.min(compressed.len()) {
        reassembled.push(compressed[i].clone());
    }
    reassembled.push(format!("[COMPRESSED SUMMARY]: {}", summary));
    for i in compressed.len().saturating_sub(tail_count)..compressed.len() {
        if i >= compressed.len() {
            break;
        }
        reassembled.push(compressed[i].clone());
    }

    phases.push("reassemble".to_string());

    let reassembled_text = reassembled.join("\n");
    let compressed_tokens = reassembled_text.len() as i64 / 4;
    let savings_pct = if original_tokens > 0 {
        ((original_tokens - compressed_tokens) as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };

    let _ = check_anti_thrashing(locks, session_id, savings_pct);

    Ok(CompressionResult {
        original_tokens,
        compressed_tokens,
        savings_pct,
        summary,
        phases_applied: phases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_memory_db(with_fts5: bool) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // 011_memory.sql ends with a vec0 virtual table that requires sqlite-vec;
        // execute_batch creates memory_facts + session_checkpoints + indexes before stopping.
        let _ = conn.execute_batch(include_str!("../migrations/011_memory.sql"));
        if with_fts5 {
            conn.execute_batch(include_str!("../migrations/023_memory_fts5.sql"))
                .unwrap();
        }
        conn
    }

    #[test]
    fn test_sql_injection_and_single_quotes_safe() {
        let db = setup_memory_db(false);

        let quote_content = "I'll use Rust's rusqlite::params! so don't interpolate";
        let sqli_payload = "'; DROP TABLE memory_facts; --";

        let f1 = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "agent'1".to_string(),
                session_id: "sess'1".to_string(),
                user_id: "u1".to_string(),
                org_id: "org'1".to_string(),
                fact_type: "decision".to_string(),
                content: quote_content.to_string(),
                metadata: Some("{\"note\":\"it's safe\"}".to_string()),
                confidence: Some(0.9),
            },
        )
        .unwrap();

        let f2 = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "agent'1".to_string(),
                session_id: "sess'1".to_string(),
                user_id: "u1".to_string(),
                org_id: "org'1".to_string(),
                fact_type: "constraint".to_string(),
                content: sqli_payload.to_string(),
                metadata: None,
                confidence: Some(0.95),
            },
        )
        .unwrap();

        let updated = update_memory_fact(
            &db,
            &f1.id,
            Some("We don't use raw format!('...') in SQL"),
            Some(0.99),
            Some("{\"note\":\"updated's value\"}"),
        )
        .unwrap();
        assert_eq!(updated.content, "We don't use raw format!('...') in SQL");

        let by_quote = get_memory_facts(
            &db,
            &MemoryQuery {
                agent_id: Some("agent'1".to_string()),
                session_id: Some("sess'1".to_string()),
                org_id: Some("org'1".to_string()),
                fact_type: None,
                min_confidence: None,
                q: Some("don't".to_string()),
                limit: Some(10),
                offset: Some(0),
            },
        )
        .unwrap();
        assert_eq!(by_quote.len(), 1);
        assert_eq!(by_quote[0].id, f1.id);

        let by_sqli = get_memory_facts(
            &db,
            &MemoryQuery {
                agent_id: None,
                session_id: None,
                org_id: None,
                fact_type: None,
                min_confidence: None,
                q: Some(sqli_payload.to_string()),
                limit: Some(10),
                offset: Some(0),
            },
        )
        .unwrap();
        assert_eq!(by_sqli.len(), 1);
        assert_eq!(by_sqli[0].id, f2.id);

        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM memory_facts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "memory_facts table must remain intact");
    }

    #[test]
    fn test_fts5_bm25_hybrid_search() {
        let db = setup_memory_db(true);

        let f1 = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "a1".to_string(),
                session_id: "s1".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj1".to_string(),
                fact_type: "pattern".to_string(),
                content: "authentication token expiry handling".to_string(),
                metadata: None,
                confidence: Some(0.9),
            },
        )
        .unwrap();

        let f2 = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "a1".to_string(),
                session_id: "s1".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj1".to_string(),
                fact_type: "pattern".to_string(),
                content: "database connection pool sizing".to_string(),
                metadata: None,
                confidence: Some(0.9),
            },
        )
        .unwrap();

        let f3 = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "a1".to_string(),
                session_id: "s1".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj1".to_string(),
                fact_type: "pattern".to_string(),
                content: "token refresh rotation strategy".to_string(),
                metadata: None,
                confidence: Some(0.9),
            },
        )
        .unwrap();

        let q = MemoryQuery {
            agent_id: None,
            session_id: None,
            org_id: Some("proj1".to_string()),
            fact_type: None,
            min_confidence: None,
            q: None,
            limit: Some(10),
            offset: Some(0),
        };

        let results = hybrid_search(&db, &q, "token rotation", 10).unwrap();
        assert_eq!(results.len(), 2, "only token/rotation facts should match");
        assert_eq!(results[0].fact.id, f3.id, "multi-term match must rank first");
        assert_eq!(results[1].fact.id, f1.id);
        assert_eq!(results[0].match_type, "fts5_bm25");
        assert!(results[0].score > results[1].score);

        // Verify FTS5 update trigger sync
        update_memory_fact(
            &db,
            &f2.id,
            Some("key rotation management for postgres"),
            None,
            Some("{\"entity\":\"rotation\"}"),
        )
        .unwrap();
        let after_update = hybrid_search(&db, &q, "key management", 10).unwrap();
        assert_eq!(after_update.len(), 1);
        assert_eq!(after_update[0].fact.id, f2.id);

        // Verify FTS5 delete trigger sync
        delete_memory_fact(&db, &f3.id).unwrap();
        let after_delete = hybrid_search(&db, &q, "refresh", 10).unwrap();
        assert!(after_delete.is_empty());

        // Special characters / punctuation must not crash FTS5
        let special = hybrid_search(&db, &q, "don't c++ !!!", 10).unwrap();
        assert!(special.is_empty());

        // Punctuation-only query falls back gracefully and returns empty
        let punct = hybrid_search(&db, &q, "!!! --- ***", 10).unwrap();
        assert!(punct.is_empty());
    }

    #[test]
    fn test_fts5_backfill_and_entity_match_and_fallback() {
        // 1. Create DB without FTS5, insert a fact, then apply 023_memory_fts5.sql
        let db = setup_memory_db(false);
        let pre_fact = create_memory_fact(
            &db,
            &MemoryFactInput {
                agent_id: "a1".to_string(),
                session_id: "s1".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj2".to_string(),
                fact_type: "decision".to_string(),
                content: "Use WAL mode for concurrent reads".to_string(),
                metadata: Some("{\"component\":\"sqlite\"}".to_string()),
                confidence: Some(0.85),
            },
        )
        .unwrap();

        let q = MemoryQuery {
            agent_id: Some("a1".to_string()),
            session_id: Some("s1".to_string()),
            org_id: Some("proj2".to_string()),
            fact_type: None,
            min_confidence: None,
            q: None,
            limit: Some(10),
            offset: Some(0),
        };

        // Fallback keyword scan works when memory_facts_fts is absent
        let fallback_res = hybrid_search(&db, &q, "concurrent", 10).unwrap();
        assert_eq!(fallback_res.len(), 1);
        assert_eq!(fallback_res[0].fact.id, pre_fact.id);
        assert_eq!(fallback_res[0].match_type, "bm25");

        // Apply 023_memory_fts5.sql and verify backfill + entity match
        db.execute_batch(include_str!("../migrations/023_memory_fts5.sql"))
            .unwrap();
        let fts_entity = hybrid_search(&db, &q, "sqlite", 10).unwrap();
        assert_eq!(fts_entity.len(), 1);
        assert_eq!(fts_entity[0].fact.id, pre_fact.id);
        assert_eq!(fts_entity[0].match_type, "entity");
    }
}

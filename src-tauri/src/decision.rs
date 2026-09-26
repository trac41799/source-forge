// src-tauri/src/decision.rs
//
// Decision layer (spec 001). M0 foundation: backend config, usage audit, review queue.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionConfig {
    pub backend: String,
    pub base_url: String,
    pub model: String,
    pub accept_threshold: f64,
    pub review_threshold: f64,
    pub context_limit: i64,
    pub timeout_ms: u64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            backend: "hosted".into(),
            base_url: "https://openrouter.ai/api".into(),
            model: "typesafe/jev-1.13".into(),
            accept_threshold: 0.75,
            review_threshold: 0.40,
            context_limit: 32000,
            timeout_ms: 5000,
        }
    }
}

pub fn get_decision_config(conn: &Connection) -> Result<DecisionConfig, String> {
    let mut stmt = conn
        .prepare(
            "SELECT backend, base_url, model, accept_threshold, review_threshold, context_limit, timeout_ms
             FROM decision_config WHERE id = 'default'",
        )
        .map_err(|e| e.to_string())?;

    let result = stmt.query_row([], |row| {
        Ok(DecisionConfig {
            backend: row.get(0)?,
            base_url: row.get(1)?,
            model: row.get(2)?,
            accept_threshold: row.get(3)?,
            review_threshold: row.get(4)?,
            context_limit: row.get(5)?,
            timeout_ms: row.get::<_, i64>(6)? as u64,
        })
    });

    match result {
        Ok(cfg) => Ok(cfg),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DecisionConfig::default()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_decision_config(conn: &Connection, cfg: &DecisionConfig) -> Result<(), String> {
    if cfg.review_threshold > cfg.accept_threshold {
        return Err(format!(
            "review_threshold ({}) must be <= accept_threshold ({})",
            cfg.review_threshold, cfg.accept_threshold
        ));
    }
    conn.execute(
        "INSERT INTO decision_config (id, backend, base_url, model, accept_threshold, review_threshold, context_limit, timeout_ms, updated_at)
         VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
           backend = excluded.backend,
           base_url = excluded.base_url,
           model = excluded.model,
           accept_threshold = excluded.accept_threshold,
           review_threshold = excluded.review_threshold,
           context_limit = excluded.context_limit,
           timeout_ms = excluded.timeout_ms,
           updated_at = excluded.updated_at",
        rusqlite::params![
            cfg.backend,
            cfg.base_url,
            cfg.model,
            cfg.accept_threshold,
            cfg.review_threshold,
            cfg.context_limit,
            cfg.timeout_ms as i64,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// Normalized decision responses (spec R2/R3)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DecisionError {
    Transport(String),
    Timeout,
    Malformed(String),
    Validation(String),
    Backend(String),
    NoBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionAnswer {
    pub kind: String,
    pub value: serde_json::Value,
    pub confidence: f64,
    #[serde(default)]
    pub probabilities: BTreeMap<String, f64>,
    #[serde(default)]
    pub legend: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionResult {
    pub model: String,
    pub answers: BTreeMap<String, DecisionAnswer>,
    pub input_tokens: i64,
    pub cost: f64,
    #[serde(default)]
    pub truncated: bool,
}

fn f64_map(v: Option<&serde_json::Value>) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if let Some(serde_json::Value::Object(m)) = v {
        for (k, val) in m {
            if let Some(n) = val.as_f64() {
                out.insert(k.clone(), n);
            }
        }
    }
    out
}

fn str_map(v: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(serde_json::Value::Object(m)) = v {
        for (k, val) in m {
            if let Some(s) = val.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

fn check_probabilities(kind: &str, probs: &BTreeMap<String, f64>) -> Result<(), DecisionError> {
    if probs.is_empty() {
        return Err(DecisionError::Validation(format!(
            "{kind} answer has no probabilities"
        )));
    }
    let sum: f64 = probs.values().sum();
    if (sum - 1.0).abs() > 0.001 {
        return Err(DecisionError::Validation(format!(
            "{kind} probabilities sum to {sum:.4}, expected 1.0 (±0.001)"
        )));
    }
    Ok(())
}

fn check_unit_range(name: &str, field: &str, v: f64) -> Result<(), DecisionError> {
    if !(0.0..=1.0).contains(&v) {
        return Err(DecisionError::Validation(format!(
            "{name} {field}={v} out of [0,1]"
        )));
    }
    Ok(())
}

/// Normalize a backend `/v1/systemone` response into typed answers (spec R2/R3).
/// `choice`/`score` require a distribution summing to 1.0; `noul` has no wire
/// confidence, so `confidence` is derived from its value.
pub fn normalize_response(json: &serde_json::Value) -> Result<DecisionResult, DecisionError> {
    let answers_obj = json
        .get("answers")
        .and_then(|a| a.as_object())
        .ok_or_else(|| DecisionError::Malformed("missing 'answers' object".into()))?;

    let mut answers = BTreeMap::new();
    for (name, ans) in answers_obj {
        let kind = ans
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| DecisionError::Malformed(format!("answer '{name}' missing 'type'")))?;

        let normalized = match kind {
            "choice" => {
                let value = ans.get("choice").cloned().ok_or_else(|| {
                    DecisionError::Malformed(format!("choice '{name}' missing 'choice'"))
                })?;
                let probabilities = f64_map(ans.get("probabilities"));
                check_probabilities("choice", &probabilities)?;
                if let Some(label) = value.as_str() {
                    if !probabilities.contains_key(label) {
                        return Err(DecisionError::Validation(format!(
                            "choice '{name}' value '{label}' not among offered labels"
                        )));
                    }
                }
                let confidence = ans.get("confidence").and_then(|c| c.as_f64()).ok_or_else(|| {
                    DecisionError::Validation(format!("choice '{name}' missing confidence"))
                })?;
                check_unit_range(name, "confidence", confidence)?;
                DecisionAnswer {
                    kind: kind.into(),
                    value,
                    confidence,
                    probabilities,
                    legend: BTreeMap::new(),
                }
            }
            "score" => {
                let value = ans.get("score").cloned().ok_or_else(|| {
                    DecisionError::Malformed(format!("score '{name}' missing 'score'"))
                })?;
                let probabilities = f64_map(ans.get("probabilities"));
                check_probabilities("score", &probabilities)?;
                let legend = str_map(ans.get("legend"));
                let confidence = ans.get("confidence").and_then(|c| c.as_f64()).ok_or_else(|| {
                    DecisionError::Validation(format!("score '{name}' missing confidence"))
                })?;
                check_unit_range(name, "confidence", confidence)?;
                DecisionAnswer {
                    kind: kind.into(),
                    value,
                    confidence,
                    probabilities,
                    legend,
                }
            }
            "noul" => {
                let value = ans.get("noul").and_then(|n| n.as_f64()).ok_or_else(|| {
                    DecisionError::Malformed(format!("noul '{name}' missing 'noul'"))
                })?;
                check_unit_range(name, "noul", value)?;
                DecisionAnswer {
                    kind: kind.into(),
                    value: serde_json::json!(value),
                    confidence: value,
                    probabilities: BTreeMap::new(),
                    legend: BTreeMap::new(),
                }
            }
            other => {
                return Err(DecisionError::Validation(format!(
                    "unknown answer type '{other}'"
                )))
            }
        };
        answers.insert(name.clone(), normalized);
    }

    let usage = json.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("input_tokens"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    let cost = usage
        .and_then(|u| u.get("cost"))
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);
    let model = json
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();

    Ok(DecisionResult {
        model,
        answers,
        input_tokens,
        cost,
        truncated: false,
    })
}

// ============================================================================
// Endpoint, request body, truncation, validation (spec R1, R6, R7)
// ============================================================================

/// `{base_url}/v1/systemone` (trailing slash tolerated). The `chat/completions`
/// path is intentionally NOT used (spec R1, ADR 0001).
pub fn endpoint(cfg: &DecisionConfig) -> String {
    format!("{}/v1/systemone", cfg.base_url.trim_end_matches('/'))
}

/// Conservative token estimate: ceil(chars / 4) (spec R7).
pub fn estimate_tokens(s: &str) -> i64 {
    ((s.chars().count() as i64) + 3) / 4
}

/// Truncate `state` to `limit_tokens`; returns `(state, truncated)`.
pub fn truncate_state(state: &str, limit_tokens: i64) -> (String, bool) {
    if limit_tokens <= 0 {
        return (String::new(), !state.is_empty());
    }
    let max_chars = (limit_tokens as usize).saturating_mul(4);
    let count = state.chars().count();
    if count <= max_chars {
        (state.to_string(), false)
    } else {
        (state.chars().take(max_chars).collect(), true)
    }
}

/// A `choice` needs at least two distinct labels (spec edge case #4).
pub fn validate_choice_labels(labels: &[String]) -> Result<(), DecisionError> {
    let distinct: BTreeMap<&String, ()> = labels.iter().map(|l| (l, ())).collect();
    if distinct.len() < 2 {
        return Err(DecisionError::Validation(
            "choice requires at least 2 distinct labels".into(),
        ));
    }
    Ok(())
}

/// Build the outbound request body. Contains `state` (required) but never the key.
pub fn build_request_body(
    model: &str,
    state: &serde_json::Value,
    questions: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({ "model": model, "state": state, "questions": questions })
}

// ============================================================================
// Usage audit (spec R5) — never stores `state`
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionUsage {
    pub id: String,
    pub backend_id: String,
    pub model: String,
    pub primitives: String,
    pub answers: String,
    pub confidence: f64,
    pub policy_outcome: String,
    pub latency_ms: i64,
    pub input_tokens: i64,
    pub cost: f64,
    pub truncated: bool,
}

pub fn record_usage(conn: &Connection, u: &DecisionUsage) -> Result<(), String> {
    conn.execute(
        "INSERT INTO decision_usage (id, backend_id, model, primitives, answers, confidence, policy_outcome, latency_ms, input_tokens, cost, truncated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            u.id,
            u.backend_id,
            u.model,
            u.primitives,
            u.answers,
            u.confidence,
            u.policy_outcome,
            u.latency_ms,
            u.input_tokens,
            u.cost,
            u.truncated as i64,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// Threshold policy + review queue (spec R8, R50, ADR 0002)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOutcome {
    Accept,
    Review,
    Fallback,
}

impl PolicyOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyOutcome::Accept => "accept",
            PolicyOutcome::Review => "review",
            PolicyOutcome::Fallback => "fallback",
        }
    }
}

pub fn classify_confidence(cfg: &DecisionConfig, confidence: f64) -> PolicyOutcome {
    if confidence >= cfg.accept_threshold {
        PolicyOutcome::Accept
    } else if confidence >= cfg.review_threshold {
        PolicyOutcome::Review
    } else {
        PolicyOutcome::Fallback
    }
}

pub fn enqueue_review(
    conn: &Connection,
    consumer: &str,
    question: &str,
    decided_value: &str,
    confidence: f64,
    payload: &str,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO decision_reviews (id, consumer, question, decided_value, confidence, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, consumer, question, decided_value, confidence, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

// ============================================================================
// Transport seam + bounded-retry request (spec R4)
// ============================================================================

/// Injectable transport so the timeout/retry path is unit-testable.
pub trait DecisionTransport: Send + Sync {
    fn post(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, DecisionError>;
}

/// Real HTTP transport (synchronous `ureq`), honouring `decision.timeout_ms`.
pub struct UreqTransport {
    pub timeout_ms: u64,
}

impl DecisionTransport for UreqTransport {
    fn post(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, DecisionError> {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build();
        let mut req = agent.post(url).set("Content-Type", "application/json");
        if let Some(key) = api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        match req.send_json(body) {
            Ok(resp) => resp
                .into_json::<serde_json::Value>()
                .map_err(|e| DecisionError::Malformed(e.to_string())),
            Err(ureq::Error::Status(code, _)) => {
                Err(DecisionError::Backend(format!("backend returned status {code}")))
            }
            Err(ureq::Error::Transport(t)) => {
                let msg = t.to_string();
                if msg.to_lowercase().contains("timed out") {
                    Err(DecisionError::Timeout)
                } else {
                    Err(DecisionError::Transport(msg))
                }
            }
        }
    }
}

/// POST state+questions and normalize, with bounded backoff retries.
/// Truncates text `state` to `cfg.context_limit` (R7) and records a
/// `decision_usage` row on success and failure when `conn` is provided (R5).
pub fn decision_request(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    questions: &serde_json::Value,
    max_retries: u32,
    conn: Option<&Connection>,
) -> Result<DecisionResult, DecisionError> {
    // R7: truncate text state to the configured context limit before sending.
    let (send_state, truncated) = match state {
        serde_json::Value::String(s) => {
            let (t, tr) = truncate_state(s, cfg.context_limit);
            (serde_json::Value::String(t), tr)
        }
        other => {
            if estimate_tokens(&other.to_string()) > cfg.context_limit {
                return Err(DecisionError::Validation(format!(
                    "structured state exceeds context_limit ({} tokens)",
                    cfg.context_limit
                )));
            }
            (other.clone(), false)
        }
    };

    let backend_id = if cfg.backend == "local" { "local" } else { "hosted" };
    let url = endpoint(cfg);
    let body = build_request_body(&cfg.model, &send_state, questions);
    let started = std::time::Instant::now();
    let mut last = DecisionError::NoBackend;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let backoff = 10u64.saturating_mul(1 << (attempt - 1).min(6));
            std::thread::sleep(std::time::Duration::from_millis(backoff));
        }
        match transport.post(&url, api_key, &body) {
            Ok(json) => {
                let mut result = normalize_response(&json)?;
                result.truncated = truncated;
                if let Some(c) = conn {
                    let conf = result
                        .answers
                        .values()
                        .map(|a| a.confidence)
                        .fold(0.0_f64, f64::max);
                    let outcome = classify_confidence(cfg, conf);
                    let _ = record_usage(
                        c,
                        &DecisionUsage {
                            id: uuid::Uuid::new_v4().to_string(),
                            backend_id: backend_id.to_string(),
                            model: result.model.clone(),
                            primitives: result.answers.keys().cloned().collect::<Vec<_>>().join(","),
                            answers: serde_json::to_string(&result.answers).unwrap_or_default(),
                            confidence: conf,
                            policy_outcome: outcome.as_str().to_string(),
                            latency_ms: started.elapsed().as_millis() as i64,
                            input_tokens: result.input_tokens,
                            cost: result.cost,
                            truncated,
                        },
                    );
                }
                return Ok(result);
            }
            Err(e) => last = e,
        }
    }

    // R5: record a failure row too.
    if let Some(c) = conn {
        let _ = record_usage(
            c,
            &DecisionUsage {
                id: uuid::Uuid::new_v4().to_string(),
                backend_id: backend_id.to_string(),
                model: cfg.model.clone(),
                primitives: String::new(),
                answers: String::new(),
                confidence: 0.0,
                policy_outcome: PolicyOutcome::Fallback.as_str().to_string(),
                latency_ms: started.elapsed().as_millis() as i64,
                input_tokens: 0,
                cost: 0.0,
                truncated,
            },
        );
    }
    Err(last)
}

// ============================================================================
// Mode dispatch (spec R1) — "decision" routes to the decision layer
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DecisionDispatch {
    Decision(DecisionResult),
    Unsupported(String),
}

pub fn dispatch_mode(
    mode: &str,
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    questions: &serde_json::Value,
    conn: Option<&Connection>,
) -> Result<DecisionDispatch, DecisionError> {
    match mode {
        "decision" => Ok(DecisionDispatch::Decision(decision_request(
            cfg, transport, api_key, state, questions, 2, conn,
        )?)),
        other => Ok(DecisionDispatch::Unsupported(other.to_string())),
    }
}

// ============================================================================
// Reusable classification helpers (M1+ consumers)
// ============================================================================

pub const COMPOUNDER_CATEGORIES: &[&str] = &[
    "pattern", "antipattern", "convention", "tooling", "insight", "fact", "handoff", "correction",
];
pub const KG_ENTITY_TYPES: &[&str] = &["file", "function", "pattern", "error", "decision", "library"];
pub const KG_RELATION_TYPES: &[&str] =
    &["caused_by", "fixed_by", "extends", "requires", "contradicts", "similar_to"];
pub const OUTCOMES: &[&str] = &["done", "failed", "revised", "stalled"];
pub const COMPLEXITIES: &[&str] = &["low", "medium", "high"];

fn criteria_from(labels: &[&str]) -> BTreeMap<String, String> {
    labels.iter().map(|l| ((*l).to_string(), String::new())).collect()
}

/// Ask a `choice` question; returns `(selected_label, confidence)` or `None` if absent.
pub fn choose(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    instructions: &str,
    criteria: &BTreeMap<String, String>,
    question_id: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    let mut questions = serde_json::Map::new();
    questions.insert(
        question_id.to_string(),
        serde_json::json!({ "type": "choice", "instructions": instructions, "criteria": criteria }),
    );
    let res = decision_request(
        cfg,
        transport,
        api_key,
        state,
        &serde_json::Value::Object(questions),
        2,
        conn,
    )?;
    Ok(res
        .answers
        .get(question_id)
        .and_then(|a| a.value.as_str().map(|s| (s.to_string(), a.confidence))))
}

/// Ask a `noul`; returns the yes-probability or `None` if the question is absent.
pub fn judge(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    instructions: &str,
    question_id: &str,
    conn: Option<&Connection>,
) -> Result<Option<f64>, DecisionError> {
    let mut questions = serde_json::Map::new();
    questions.insert(
        question_id.to_string(),
        serde_json::json!({ "type": "noul", "instructions": instructions }),
    );
    let res = decision_request(
        cfg,
        transport,
        api_key,
        state,
        &serde_json::Value::Object(questions),
        2,
        conn,
    )?;
    Ok(res.answers.get(question_id).and_then(|a| a.value.as_f64()))
}

pub fn choose_category(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    content: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(content),
        "Which category best describes this coding knowledge item?",
        &criteria_from(COMPOUNDER_CATEGORIES),
        "category",
        conn,
    )
}

pub fn classify_outcome(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    pty_tail: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(pty_tail),
        "What is the final outcome of this coding session?",
        &criteria_from(OUTCOMES),
        "outcome",
        conn,
    )
}

pub fn choose_complexity(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    task_desc: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(task_desc),
        "How complex is this software task?",
        &criteria_from(COMPLEXITIES),
        "complexity",
        conn,
    )
}

pub fn choose_entity_type(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    description: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(description),
        "What kind of code entity is this?",
        &criteria_from(KG_ENTITY_TYPES),
        "entity_type",
        conn,
    )
}

pub fn choose_relation_type(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    description: &str,
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(description),
        "How are these two code entities related?",
        &criteria_from(KG_RELATION_TYPES),
        "relation_type",
        conn,
    )
}

/// Pick the best agent for a task from `agents` (spec R20).
pub fn choose_agent(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    task_desc: &str,
    agents: &[String],
    conn: Option<&Connection>,
) -> Result<Option<(String, f64)>, DecisionError> {
    let criteria: BTreeMap<String, String> =
        agents.iter().map(|a| (a.clone(), String::new())).collect();
    choose(
        cfg,
        t,
        key,
        &serde_json::json!(task_desc),
        "Which agent is best suited to this software task?",
        &criteria,
        "agent",
        conn,
    )
}

/// M5 (spec R52): probe the backend with an empty state and a single `noul`,
/// carrying no real data. Returns "healthy" | "degraded" | "offline".
pub fn health_probe(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
) -> &'static str {
    let questions = serde_json::json!({
        "ok": { "type": "noul", "instructions": "Is this decision endpoint reachable?" }
    });
    match decision_request(
        cfg,
        transport,
        api_key,
        &serde_json::json!(""),
        &questions,
        0,
        None,
    ) {
        Ok(_) => "healthy",
        Err(DecisionError::Validation(_)) | Err(DecisionError::Malformed(_)) => "degraded",
        Err(_) => "offline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrations_and_config() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql"))
            .unwrap();

        // Defaults load from the single-row config table.
        let cfg = get_decision_config(&conn).unwrap();
        assert_eq!(cfg, DecisionConfig::default());
        assert_eq!(cfg.backend, "hosted");
        assert_eq!(cfg.base_url, "https://openrouter.ai/api");
        assert_eq!(cfg.context_limit, 32000);
        assert_eq!(cfg.timeout_ms, 5000);

        // Set/get round-trip.
        let mut updated = cfg.clone();
        updated.backend = "local".into();
        updated.base_url = "http://127.0.0.1:8009".into();
        updated.accept_threshold = 0.8;
        set_decision_config(&conn, &updated).unwrap();
        assert_eq!(get_decision_config(&conn).unwrap(), updated);
    }

    #[test]
    fn parses_jev_fixture() {
        let raw = include_str!("../tests/fixtures/jev_all.json");
        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        let result = normalize_response(&json).unwrap();

        assert_eq!(result.model, "typesafe/jev-1.13-20260917");
        assert_eq!(result.input_tokens, 447);
        assert!((result.cost - 0.000018774).abs() < 1e-12);

        let team = &result.answers["team"];
        assert_eq!(team.kind, "choice");
        assert_eq!(team.value, serde_json::json!("billing"));
        assert!((team.confidence - 1.0).abs() < 1e-9);
        assert!((team.probabilities["billing"] - 1.0).abs() < 1e-9);

        let urgency = &result.answers["urgency"];
        assert_eq!(urgency.kind, "score");
        assert_eq!(urgency.value, serde_json::json!(2.0));
        assert_eq!(urgency.legend.len(), 3);

        let refund = &result.answers["refund"];
        assert_eq!(refund.kind, "noul");
        assert_eq!(refund.value, serde_json::json!(0.82));
        assert!(
            (refund.confidence - 0.82).abs() < 1e-9,
            "noul confidence must be derived from its value"
        );
    }

    #[test]
    fn rejects_bad_probability_distribution() {
        let json = serde_json::json!({
            "model": "m",
            "answers": { "x": { "type": "choice", "choice": "a",
                                "probabilities": { "a": 0.3, "b": 0.2 }, "confidence": 0.6 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let err = normalize_response(&json).unwrap_err();
        assert!(
            matches!(err, DecisionError::Validation(_)),
            "expected Validation error, got {err:?}"
        );
    }

    // ── T5: backend selector ────────────────────────────────────────────
    #[test]
    fn selects_backend_url() {
        let mut cfg = DecisionConfig::default();
        assert_eq!(endpoint(&cfg), "https://openrouter.ai/api/v1/systemone");
        cfg.backend = "local".into();
        cfg.base_url = "http://127.0.0.1:8009".into();
        assert_eq!(endpoint(&cfg), "http://127.0.0.1:8009/v1/systemone");
        cfg.base_url = "http://127.0.0.1:8009/".into();
        assert_eq!(endpoint(&cfg), "http://127.0.0.1:8009/v1/systemone");
    }

    // ── T7: no secret / no state persisted ──────────────────────────────
    #[test]
    fn no_secret_or_state_in_output() {
        let secret = "sk-SECRET-KEY-123";
        let state = serde_json::json!("customer SECRET_STATE_TEXT here");
        let questions = serde_json::json!({ "q": { "type": "noul", "instructions": "x?" } });

        let body = build_request_body("typesafe/jev-1.13", &state, &questions);
        let body_str = body.to_string();
        assert!(!body_str.contains(secret), "api key must never be in the body");
        assert!(body_str.contains("SECRET_STATE_TEXT"), "state is sent (required)");

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql")).unwrap();
        let u = DecisionUsage {
            id: "u1".into(),
            backend_id: "hosted".into(),
            model: "typesafe/jev-1.13".into(),
            primitives: "noul".into(),
            answers: "{\"q\":0.9}".into(),
            confidence: 0.9,
            policy_outcome: "accept".into(),
            latency_ms: 120,
            input_tokens: 42,
            cost: 0.0001,
            truncated: false,
        };
        record_usage(&conn, &u).unwrap();
        let row: String = conn
            .query_row(
                "SELECT answers || '|' || backend_id || '|' || model FROM decision_usage WHERE id='u1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!row.contains(secret), "secret must not be persisted");
        assert!(!row.contains("SECRET_STATE_TEXT"), "state must not be persisted");
    }

    // ── T8: truncation + label validation ───────────────────────────────
    #[test]
    fn truncates_oversized_and_rejects_empty_labels() {
        let s = "a".repeat(400); // 100 estimated tokens
        let (out, trunc) = truncate_state(&s, 100);
        assert_eq!(out.chars().count(), 400);
        assert!(!trunc, "exactly at limit is not truncated");

        let big = "a".repeat(4000); // 1000 estimated tokens
        let (out2, trunc2) = truncate_state(&big, 100);
        assert!(trunc2);
        assert_eq!(estimate_tokens(&out2), 100);

        assert!(validate_choice_labels(&["a".into(), "b".into()]).is_ok());
        assert!(matches!(
            validate_choice_labels(&["a".into(), "a".into()]),
            Err(DecisionError::Validation(_))
        ));
        assert!(matches!(validate_choice_labels(&[]), Err(DecisionError::Validation(_))));
    }

    // ── T10: timeout + bounded retry + fallback ─────────────────────────
    struct MockTransport {
        calls: std::sync::atomic::AtomicU32,
        fail: bool,
        payload: serde_json::Value,
    }
    impl DecisionTransport for MockTransport {
        fn post(
            &self,
            _url: &str,
            _key: Option<&str>,
            _body: &serde_json::Value,
        ) -> Result<serde_json::Value, DecisionError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail {
                Err(DecisionError::Transport("boom".into()))
            } else {
                Ok(self.payload.clone())
            }
        }
    }

    #[test]
    fn timeout_falls_back() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let cfg = DecisionConfig::default();
        let t = MockTransport {
            calls: AtomicU32::new(0),
            fail: true,
            payload: serde_json::json!({}),
        };
        let err = decision_request(
            &cfg,
            &t,
            Some("k"),
            &serde_json::json!("x"),
            &serde_json::json!({}),
            2,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, DecisionError::Transport(_)));
        assert_eq!(t.calls.load(Ordering::SeqCst), 3, "1 initial + 2 retries");
    }

    #[test]
    fn decision_request_normalizes_success() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let cfg = DecisionConfig::default();
        let payload: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/jev_all.json")).unwrap();
        let t = MockTransport {
            calls: AtomicU32::new(0),
            fail: false,
            payload,
        };
        let res = decision_request(
            &cfg,
            &t,
            Some("k"),
            &serde_json::json!("x"),
            &serde_json::json!({}),
            0,
            None,
        )
        .unwrap();
        assert_eq!(res.answers.len(), 3);
        assert_eq!(t.calls.load(Ordering::SeqCst), 1);
    }

    // ── T12/T13: threshold policy + review queue ────────────────────────
    #[test]
    fn threshold_policy() {
        let cfg = DecisionConfig::default(); // accept 0.75, review 0.40
        assert_eq!(classify_confidence(&cfg, 0.90), PolicyOutcome::Accept);
        assert_eq!(classify_confidence(&cfg, 0.75), PolicyOutcome::Accept);
        assert_eq!(classify_confidence(&cfg, 0.50), PolicyOutcome::Review);
        assert_eq!(classify_confidence(&cfg, 0.40), PolicyOutcome::Review);
        assert_eq!(classify_confidence(&cfg, 0.20), PolicyOutcome::Fallback);
    }

    #[test]
    fn review_enqueue_persists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql")).unwrap();
        let id = enqueue_review(&conn, "router", "agent", "none", 0.5, "{}").unwrap();
        let (consumer, decided, conf, resolved): (String, String, f64, i64) = conn
            .query_row(
                "SELECT consumer, decided_value, confidence, resolved FROM decision_reviews WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(consumer, "router");
        assert_eq!(decided, "none");
        assert!((conf - 0.5).abs() < 1e-9);
        assert_eq!(resolved, 0);
    }

    // ── T14: mode dispatch ──────────────────────────────────────────────
    #[test]
    fn decision_mode_dispatch() {
        use std::sync::atomic::AtomicU32;
        let cfg = DecisionConfig::default();
        let payload: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/jev_all.json")).unwrap();
        let t = MockTransport {
            calls: AtomicU32::new(0),
            fail: false,
            payload,
        };

        let dispatched = dispatch_mode(
            "decision",
            &cfg,
            &t,
            Some("k"),
            &serde_json::json!("x"),
            &serde_json::json!({}),
            None,
        )
        .unwrap();
        assert!(matches!(
            dispatched,
            DecisionDispatch::Decision(ref r) if r.answers.len() == 3
        ));

        let unsupported = dispatch_mode(
            "openrouter",
            &cfg,
            &t,
            None,
            &serde_json::json!("x"),
            &serde_json::json!({}),
            None,
        )
        .unwrap();
        assert!(matches!(unsupported, DecisionDispatch::Unsupported(_)));
    }

    #[test]
    fn records_usage_on_success() {
        use std::sync::atomic::AtomicU32;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql")).unwrap();
        let cfg = DecisionConfig::default();
        let payload: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/jev_all.json")).unwrap();
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        let res = decision_request(
            &cfg,
            &t,
            Some("k"),
            &serde_json::json!("x"),
            &serde_json::json!({}),
            0,
            Some(&conn),
        )
        .unwrap();
        assert!(!res.truncated);
        let (n, outcome, conf): (i64, String, f64) = conn
            .query_row(
                "SELECT COUNT(*), MAX(policy_outcome), MAX(confidence) FROM decision_usage",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(n, 1, "exactly one usage row on success");
        assert_eq!(outcome, "accept", "fixture confidence 1.0 >= accept threshold");
        assert!((conf - 1.0).abs() < 1e-9);
    }

    #[test]
    fn records_usage_on_failure() {
        use std::sync::atomic::AtomicU32;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql")).unwrap();
        let cfg = DecisionConfig::default();
        let t = MockTransport { calls: AtomicU32::new(0), fail: true, payload: serde_json::json!({}) };
        let _ = decision_request(
            &cfg,
            &t,
            None,
            &serde_json::json!("x"),
            &serde_json::json!({}),
            0,
            Some(&conn),
        )
        .unwrap_err();
        let (n, outcome): (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), MAX(policy_outcome) FROM decision_usage",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(n, 1, "one usage row on failure");
        assert_eq!(outcome, "fallback");
    }

    #[test]
    fn truncates_state_on_real_path() {
        use std::sync::atomic::AtomicU32;
        let cfg = DecisionConfig::default(); // context_limit 32000 tokens
        let payload: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/jev_all.json")).unwrap();
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        let big = "a".repeat(200_000); // ~50k tokens > 32k
        let res = decision_request(
            &cfg,
            &t,
            None,
            &serde_json::json!(big),
            &serde_json::json!({}),
            0,
            None,
        )
        .unwrap();
        assert!(res.truncated, "oversized text state must be flagged truncated");
    }

    #[test]
    fn rejects_out_of_range_and_unoffered_values() {
        let bad_noul = serde_json::json!({
            "model": "m",
            "answers": { "x": { "type": "noul", "noul": 1.2 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        assert!(matches!(
            normalize_response(&bad_noul),
            Err(DecisionError::Validation(_))
        ));

        let bad_label = serde_json::json!({
            "model": "m",
            "answers": { "x": { "type": "choice", "choice": "bogus",
                                "probabilities": {"a": 0.5, "b": 0.5}, "confidence": 0.6 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        assert!(matches!(
            normalize_response(&bad_label),
            Err(DecisionError::Validation(_))
        ));
    }

    #[test]
    fn rejects_inverted_thresholds() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql")).unwrap();
        let mut cfg = DecisionConfig::default();
        cfg.review_threshold = 0.9;
        cfg.accept_threshold = 0.5;
        assert!(set_decision_config(&conn, &cfg).is_err());
    }

    // ── M1 helpers ──────────────────────────────────────────────────────
    #[test]
    fn choose_category_and_judge_use_decision() {
        use std::sync::atomic::AtomicU32;
        let cfg = DecisionConfig::default();

        let cat_payload = serde_json::json!({
            "model": "m",
            "answers": { "category": { "type": "choice", "choice": "pattern",
                                       "probabilities": {"pattern": 1.0}, "confidence": 0.9 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload: cat_payload };
        let got = choose_category(&cfg, &t, None, "always run tests first", None).unwrap();
        assert_eq!(got, Some(("pattern".to_string(), 0.9)));

        let noul_payload = serde_json::json!({
            "model": "m",
            "answers": { "ok": { "type": "noul", "noul": 0.7 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t2 = MockTransport { calls: AtomicU32::new(0), fail: false, payload: noul_payload };
        let v = judge(&cfg, &t2, None, &serde_json::json!("x"), "is it ok?", "ok", None).unwrap();
        assert_eq!(v, Some(0.7));
    }

    #[test]
    fn classification_helper_labels_are_offered() {
        // Sanity: the criteria maps are non-empty and contain the labels we expect.
        assert!(criteria_from(COMPOUNDER_CATEGORIES).contains_key("pattern"));
        assert!(criteria_from(KG_ENTITY_TYPES).contains_key("function"));
        assert!(criteria_from(KG_RELATION_TYPES).contains_key("contradicts"));
        assert!(criteria_from(OUTCOMES).contains_key("stalled"));
        assert!(criteria_from(COMPLEXITIES).contains_key("medium"));
    }

    #[test]
    fn health_probe_reports_state() {
        use std::sync::atomic::AtomicU32;
        let cfg = DecisionConfig::default();

        let ok = serde_json::json!({
            "model": "m",
            "answers": { "ok": { "type": "noul", "noul": 0.9 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t_ok = MockTransport { calls: AtomicU32::new(0), fail: false, payload: ok };
        assert_eq!(health_probe(&cfg, &t_ok, None), "healthy");

        let t_bad = MockTransport { calls: AtomicU32::new(0), fail: true, payload: serde_json::json!({}) };
        assert_eq!(health_probe(&cfg, &t_bad, None), "offline");
    }
}

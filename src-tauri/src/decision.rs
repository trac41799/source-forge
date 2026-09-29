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
    // Selecting "local" while retaining the hosted URL would silently send
    // local-intended traffic to OpenRouter — reject that misconfiguration.
    if cfg.backend == "local" && cfg.base_url.trim_end_matches('/') == "https://openrouter.ai/api" {
        return Err(
            "backend=local requires changing base_url away from the hosted OpenRouter URL".into(),
        );
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
    /// R-12: FNV-1a fingerprint of `(state, questions)` — lets runs be compared
    /// without ever persisting the raw state.
    pub input_hash: String,
}

/// R-12: stable, dependency-free FNV-1a fingerprint of the decision inputs.
pub fn input_fingerprint(state: &serde_json::Value, questions: &serde_json::Value) -> String {
    fn fnv(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h ^= *b as u64;
            *h = h.wrapping_mul(0x100000001b3);
        }
    }
    let mut h: u64 = 0xcbf29ce484222325;
    fnv(&mut h, state.to_string().as_bytes());
    fnv(&mut h, b"\x1f");
    fnv(&mut h, questions.to_string().as_bytes());
    format!("{h:016x}")
}

pub fn record_usage(conn: &Connection, u: &DecisionUsage) -> Result<(), String> {
    conn.execute(
        "INSERT INTO decision_usage (id, backend_id, model, primitives, answers, confidence, policy_outcome, latency_ms, input_tokens, cost, truncated, input_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            u.input_hash,
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

/// R50: the action a consumer should take for a confidence — act, review, or skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Apply,
    Review,
    Skip,
}

pub fn policy_action(cfg: &DecisionConfig, confidence: f64) -> PolicyAction {
    if confidence >= cfg.accept_threshold {
        PolicyAction::Apply
    } else if confidence >= cfg.review_threshold {
        PolicyAction::Review
    } else {
        PolicyAction::Skip
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
    // R-15: one row per (consumer, question, decided_value) — re-deciding the same
    // thing reuses the existing review (requires migration 017's unique index).
    let changed = conn
        .execute(
            "INSERT OR IGNORE INTO decision_reviews (id, consumer, question, decided_value, confidence, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, consumer, question, decided_value, confidence, payload],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        let existing: String = conn
            .query_row(
                "SELECT id FROM decision_reviews
                 WHERE consumer = ?1 AND question = ?2 AND decided_value = ?3
                 LIMIT 1",
                rusqlite::params![consumer, question, decided_value],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        return Ok(existing);
    }
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

/// R-2: reuse one `ureq::Agent` per timeout so keep-alive/connection pooling
/// works instead of constructing a fresh agent (and TLS state) on every call.
fn agent_for(timeout_ms: u64) -> ureq::Agent {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<u64, ureq::Agent>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().unwrap();
    map.entry(timeout_ms)
        .or_insert_with(|| {
            ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_millis(timeout_ms))
                .build()
        })
        .clone()
}

impl DecisionTransport for UreqTransport {
    fn post(
        &self,
        url: &str,
        api_key: Option<&str>,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, DecisionError> {
        let agent = agent_for(self.timeout_ms);
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
    let started = std::time::Instant::now();
    let backend_id = if cfg.backend == "local" { "local" } else { "hosted" };
    let fingerprint = input_fingerprint(state, questions);

    // R-10: short-circuit while the breaker is open (repeated failures / spend cap).
    if breaker_is_open() {
        if let Some(c) = conn {
            record_failure_usage(c, backend_id, cfg, started, false, &fingerprint);
        }
        return Err(DecisionError::Backend(
            "decision circuit open — backing off (failures or spend cap reached)".into(),
        ));
    }

    // R7: truncate text state to the configured context limit before sending.
    let (send_state, truncated) = match state {
        serde_json::Value::String(s) => {
            let (t, tr) = truncate_state(s, cfg.context_limit);
            (serde_json::Value::String(t), tr)
        }
        other => {
            if estimate_tokens(&other.to_string()) > cfg.context_limit {
                let err = DecisionError::Validation(format!(
                    "structured state exceeds context_limit ({} tokens)",
                    cfg.context_limit
                ));
                if let Some(c) = conn {
                    record_failure_usage(c, backend_id, cfg, started, false, &fingerprint);
                }
                return Err(err);
            }
            (other.clone(), false)
        }
    };

    let url = endpoint(cfg);
    let body = build_request_body(&cfg.model, &send_state, questions);
    let mut last = DecisionError::NoBackend;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let backoff = 10u64.saturating_mul(1 << (attempt - 1).min(6));
            std::thread::sleep(std::time::Duration::from_millis(backoff));
        }
        match transport.post(&url, api_key, &body) {
            Ok(json) => {
                match normalize_response(&json) {
                    Ok(mut result) => {
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
                                    primitives: result
                                        .answers
                                        .keys()
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join(","),
                                    answers: serde_json::to_string(&result.answers)
                                        .unwrap_or_default(),
                                    confidence: conf,
                                    policy_outcome: outcome.as_str().to_string(),
                                    latency_ms: started.elapsed().as_millis() as i64,
                                    input_tokens: result.input_tokens,
                                    cost: result.cost,
                                    truncated,
                                    input_hash: fingerprint.clone(),
                                },
                            );
                        }
                        mark_decision_success(now_epoch_ms());
                        breaker_record_success(result.cost);
                        return Ok(result);
                    }
                    Err(e) => {
                        // Malformed/validation responses are not retried, but must
                        // still produce a failure row (R5).
                        if let Some(c) = conn {
                            record_failure_usage(c, backend_id, cfg, started, truncated, &fingerprint);
                        }
                        return Err(e);
                    }
                }
            }
            Err(e) => last = e,
        }
    }

    // R5: a failure row is written even when every attempt fails.
    if let Some(c) = conn {
        record_failure_usage(c, backend_id, cfg, started, truncated, &fingerprint);
    }
    breaker_record_failure();
    Err(last)
}

/// R5: a `decision_usage` row with the fallback outcome, for any non-success path.
fn record_failure_usage(
    conn: &Connection,
    backend_id: &str,
    cfg: &DecisionConfig,
    started: std::time::Instant,
    truncated: bool,
    input_hash: &str,
) {
    let _ = record_usage(
        conn,
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
            input_hash: input_hash.to_string(),
        },
    );
}

// ============================================================================
// R-10: circuit breaker + spend cap
// ============================================================================

/// A simple circuit breaker: opens after `threshold` consecutive failures for
/// `cooldown_ms`, and also once cumulative spend reaches `cap_micros`
/// (`cap_micros == 0` disables the spend cap).
#[derive(Debug, Clone)]
pub struct Breaker {
    pub threshold: u32,
    pub cooldown_ms: u64,
    pub cap_micros: u64,
    failures: u32,
    open_until_ms: u64,
    spent_micros: u64,
}

impl Breaker {
    pub fn new(threshold: u32, cooldown_ms: u64, cap_micros: u64) -> Self {
        Self {
            threshold,
            cooldown_ms,
            cap_micros,
            failures: 0,
            open_until_ms: 0,
            spent_micros: 0,
        }
    }

    pub fn is_open(&self, now_ms: u64) -> bool {
        now_ms < self.open_until_ms
            || (self.cap_micros > 0 && self.spent_micros >= self.cap_micros)
    }

    pub fn record_success(&mut self, cost: f64, now_ms: u64) {
        self.failures = 0;
        self.spent_micros = self
            .spent_micros
            .saturating_add((cost.max(0.0) * 1_000_000.0).round() as u64);
        if self.cap_micros > 0 && self.spent_micros >= self.cap_micros {
            self.open_until_ms = now_ms.saturating_add(self.cooldown_ms);
        }
    }

    pub fn record_failure(&mut self, now_ms: u64) {
        self.failures = self.failures.saturating_add(1);
        if self.threshold > 0 && self.failures >= self.threshold {
            self.open_until_ms = now_ms.saturating_add(self.cooldown_ms);
        }
    }

    pub fn spent_micros(&self) -> u64 {
        self.spent_micros
    }
}

/// Process-wide breaker (threshold 5, 30 s cooldown, $5 spend cap by default;
/// overridable via `ACC_DECISION_BREAKER_THRESHOLD`, `ACC_DECISION_BREAKER_COOLDOWN_MS`,
/// `ACC_DECISION_COST_CAP_MICROS`).
fn global_breaker() -> &'static std::sync::Mutex<Breaker> {
    static B: std::sync::OnceLock<std::sync::Mutex<Breaker>> = std::sync::OnceLock::new();
    B.get_or_init(|| {
        let num = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        std::sync::Mutex::new(Breaker::new(
            num("ACC_DECISION_BREAKER_THRESHOLD", 5) as u32,
            num("ACC_DECISION_BREAKER_COOLDOWN_MS", 30_000),
            num("ACC_DECISION_COST_CAP_MICROS", 5_000_000),
        ))
    })
}

fn breaker_is_open() -> bool {
    global_breaker()
        .lock()
        .map(|b| b.is_open(now_epoch_ms()))
        .unwrap_or(false)
}

fn breaker_record_failure() {
    if let Ok(mut b) = global_breaker().lock() {
        b.record_failure(now_epoch_ms());
    }
}

fn breaker_record_success(cost: f64) {
    if let Ok(mut b) = global_breaker().lock() {
        b.record_success(cost, now_epoch_ms());
    }
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

/// Public criteria builder for callers that batch questions (R-4).
pub fn criteria(labels: &[&str]) -> BTreeMap<String, String> {
    criteria_from(labels)
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
    // Edge case #4: a `choice` needs at least two distinct labels.
    let labels: Vec<String> = criteria.keys().cloned().collect();
    validate_choice_labels(&labels)?;
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
/// R-4: ask many `choice` questions about one shared `state` in a single request.
/// Returns `qid → (label, confidence)` for each answered question.
pub fn choose_batch(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    questions: &[(String, String, BTreeMap<String, String>)],
    conn: Option<&Connection>,
) -> Result<BTreeMap<String, (String, f64)>, DecisionError> {
    let mut qmap = serde_json::Map::new();
    for (qid, instructions, criteria) in questions {
        let labels: Vec<String> = criteria.keys().cloned().collect();
        validate_choice_labels(&labels)?;
        qmap.insert(
            qid.clone(),
            serde_json::json!({ "type": "choice", "instructions": instructions, "criteria": criteria }),
        );
    }
    let res = decision_request(
        cfg,
        transport,
        api_key,
        state,
        &serde_json::Value::Object(qmap),
        2,
        conn,
    )?;
    let mut out = BTreeMap::new();
    for (qid, _, _) in questions {
        if let Some(ans) = res.answers.get(qid) {
            if let Some(label) = ans.value.as_str() {
                out.insert(qid.clone(), (label.to_string(), ans.confidence));
            }
        }
    }
    Ok(out)
}

/// R-4: ask many `noul` questions about one shared `state` in a single request.
/// Returns `qid → probability` for each answered question.
pub fn judge_batch(
    cfg: &DecisionConfig,
    transport: &dyn DecisionTransport,
    api_key: Option<&str>,
    state: &serde_json::Value,
    questions: &[(String, String)],
    conn: Option<&Connection>,
) -> Result<BTreeMap<String, f64>, DecisionError> {
    let mut qmap = serde_json::Map::new();
    for (qid, instructions) in questions {
        qmap.insert(
            qid.clone(),
            serde_json::json!({ "type": "noul", "instructions": instructions }),
        );
    }
    let res = decision_request(
        cfg,
        transport,
        api_key,
        state,
        &serde_json::Value::Object(qmap),
        2,
        conn,
    )?;
    Ok(questions
        .iter()
        .filter_map(|(qid, _)| {
            res.answers
                .get(qid)
                .and_then(|a| a.value.as_f64())
                .map(|p| (qid.clone(), p))
        })
        .collect())
}

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

/// All agents scored by Jev's **own** probabilities, best-first (spec R20).
/// Returns `None` when there are fewer than two distinct candidates.
pub fn rank_agents(
    cfg: &DecisionConfig,
    t: &dyn DecisionTransport,
    key: Option<&str>,
    task_desc: &str,
    agents: &[String],
    conn: Option<&Connection>,
) -> Result<Option<Vec<(String, f64)>>, DecisionError> {
    // Edge case #4: need ≥2 distinct labels (mirror `choose`).
    let mut distinct: Vec<&String> = agents.iter().collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() < 2 {
        return Ok(None);
    }
    let criteria: BTreeMap<String, String> =
        agents.iter().map(|a| (a.clone(), String::new())).collect();
    let mut questions = serde_json::Map::new();
    questions.insert(
        "agent".to_string(),
        serde_json::json!({
            "type": "choice",
            "instructions": "Which agent is best suited to this software task?",
            "criteria": criteria
        }),
    );
    let res = decision_request(
        cfg,
        t,
        key,
        &serde_json::json!(task_desc),
        &serde_json::Value::Object(questions),
        2,
        conn,
    )?;
    let ans = match res.answers.get("agent") {
        Some(a) => a,
        None => return Ok(None),
    };
    // Default any candidate absent from the wire map to 0.0 so a zero-probability
    // label never keeps its prior success_rate (R20).
    let mut ranked: Vec<(String, f64)> = agents
        .iter()
        .map(|a| (a.clone(), *ans.probabilities.get(a).unwrap_or(&0.0)))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Some(ranked))
}

/// M5 (spec R52): probe the backend with an empty state and a single `noul`,
/// carrying no real data. Returns "healthy" | "degraded" | "offline".
/// R-17: epoch-ms of the last successful decision call (0 = never).
static LAST_SUCCESS_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn mark_decision_success(now: u64) {
    LAST_SUCCESS_MS.store(now, std::sync::atomic::Ordering::SeqCst);
}

/// R-17: epoch-ms of the last healthy decision call (0 if none yet).
pub fn last_success_ms() -> u64 {
    LAST_SUCCESS_MS.load(std::sync::atomic::Ordering::SeqCst)
}

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
        Ok(_) => {
            mark_decision_success(now_epoch_ms());
            "healthy"
        }
        Err(DecisionError::Validation(_)) | Err(DecisionError::Malformed(_)) => "degraded",
        // R-17: a backend that answers with an HTTP/auth error is reachable, not offline.
        Err(DecisionError::Backend(_)) => "degraded",
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
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
            input_hash: "abc123".into(),
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
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
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
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

    #[test]
    fn rejects_local_backend_with_hosted_url() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
        let mut cfg = DecisionConfig::default();
        cfg.backend = "local".into(); // base_url still the hosted default
        assert!(set_decision_config(&conn, &cfg).is_err());
        cfg.base_url = "http://127.0.0.1:8009".into();
        assert!(set_decision_config(&conn, &cfg).is_ok());
    }

    #[test]
    fn records_usage_on_malformed_response() {
        use std::sync::atomic::AtomicU32;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql"))).unwrap();
        let cfg = DecisionConfig::default();
        // transport succeeds but the payload is invalid (noul out of range)
        let bad = serde_json::json!({
            "model": "m",
            "answers": { "x": { "type": "noul", "noul": 5.0 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload: bad };
        let err = decision_request(
            &cfg,
            &t,
            None,
            &serde_json::json!("x"),
            &serde_json::json!({}),
            2,
            Some(&conn),
        )
        .unwrap_err();
        assert!(matches!(err, DecisionError::Validation(_)));
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM decision_usage", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "malformed responses must still record a failure row (R5)");
    }

    #[test]
    fn policy_action_boundaries() {
        let cfg = DecisionConfig::default(); // accept 0.75, review 0.40
        assert_eq!(policy_action(&cfg, 0.75), PolicyAction::Apply);
        assert_eq!(policy_action(&cfg, 0.74), PolicyAction::Review);
        assert_eq!(policy_action(&cfg, 0.40), PolicyAction::Review);
        assert_eq!(policy_action(&cfg, 0.39), PolicyAction::Skip);
    }

    #[test]
    fn rank_agents_defaults_missing_labels_to_zero() {
        use std::sync::atomic::AtomicU32;
        let cfg = DecisionConfig::default();
        // wire map only contains "a"; "b" must be treated as 0.0
        let payload = serde_json::json!({
            "model": "m",
            "answers": { "agent": { "type": "choice", "choice": "a",
                                    "probabilities": {"a": 1.0}, "confidence": 1.0 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        let ranked = rank_agents(
            &cfg,
            &t,
            None,
            "task",
            &["a".to_string(), "b".to_string()],
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0], ("a".to_string(), 1.0));
        assert_eq!(ranked[1], ("b".to_string(), 0.0));
    }

    // ── R-15: review queue must deduplicate ─────────────────────────────
    #[test]
    fn enqueue_review_is_idempotent_for_same_decision() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/019_decision_reviews_unique.sql"))
            .unwrap();
        let a = enqueue_review(&conn, "c", "q", "v", 0.5, "{}").unwrap();
        let b = enqueue_review(&conn, "c", "q", "v", 0.6, "{\"again\":1}").unwrap();
        assert_eq!(a, b, "same decision must reuse the existing review id");
        let n: i64 = conn
            .query_row("SELECT count(*) FROM decision_reviews", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    // ── R-17: backend/auth errors are "degraded", and success is timestamped ──
    struct ErrTransport {
        err: DecisionError,
    }
    impl DecisionTransport for ErrTransport {
        fn post(
            &self,
            _url: &str,
            _key: Option<&str>,
            _body: &serde_json::Value,
        ) -> Result<serde_json::Value, DecisionError> {
            Err(self.err.clone())
        }
    }

    #[test]
    fn health_probe_backend_error_is_degraded_not_offline() {
        let cfg = DecisionConfig::default();
        let backend = ErrTransport {
            err: DecisionError::Backend("backend returned status 401".into()),
        };
        assert_eq!(health_probe(&cfg, &backend, None), "degraded");
        let timeout = ErrTransport {
            err: DecisionError::Timeout,
        };
        assert_eq!(health_probe(&cfg, &timeout, None), "offline");
    }

    #[test]
    fn success_updates_last_success_timestamp() {
        let cfg = DecisionConfig::default();
        let payload = serde_json::json!({
            "model": "m",
            "answers": { "ok": { "type": "noul", "noul": 0.9 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t = MockTransport {
            calls: std::sync::atomic::AtomicU32::new(0),
            fail: false,
            payload,
        };
        assert_eq!(health_probe(&cfg, &t, None), "healthy");
        assert!(last_success_ms() > 0, "a healthy call records last success");
    }

    // ── R-10: circuit breaker + spend cap ───────────────────────────────
    #[test]
    fn breaker_opens_after_threshold_failures() {
        let mut b = Breaker::new(3, 1000, 0);
        assert!(!b.is_open(0));
        b.record_failure(0);
        b.record_failure(10);
        assert!(!b.is_open(20), "below threshold stays closed");
        b.record_failure(30);
        assert!(b.is_open(40), "3rd consecutive failure opens the circuit");
        assert!(!b.is_open(1_100), "closes again after the cooldown");
    }

    #[test]
    fn breaker_resets_on_success() {
        let mut b = Breaker::new(3, 1000, 0);
        b.record_failure(0);
        b.record_failure(0);
        b.record_success(0.0, 0);
        b.record_failure(0);
        assert!(!b.is_open(0), "success clears the failure streak");
    }

    #[test]
    fn cost_cap_opens_breaker() {
        let mut b = Breaker::new(10, 1000, 1_000_000); // $1 cap
        b.record_success(0.4, 0);
        assert!(!b.is_open(0));
        b.record_success(0.7, 0);
        assert!(b.is_open(0), "spend cap reached opens the circuit");
        assert!(b.spent_micros() >= 1_000_000);
    }

    // ── R-4: batching — many questions, one request ─────────────────────
    #[test]
    fn choose_batch_issues_one_request_for_many_questions() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let cfg = DecisionConfig::default();
        let payload = serde_json::json!({
            "model": "m",
            "answers": {
                "q0": { "type": "choice", "choice": "file",
                        "probabilities": {"file": 0.9, "error": 0.1}, "confidence": 0.9 },
                "q1": { "type": "choice", "choice": "error",
                        "probabilities": {"file": 0.2, "error": 0.8}, "confidence": 0.8 }
            },
            "usage": { "input_tokens": 3, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        let crit = criteria(&["file", "error"]);
        let qs = vec![
            ("q0".to_string(), "classify 0".to_string(), crit.clone()),
            ("q1".to_string(), "classify 1".to_string(), crit.clone()),
        ];
        let out =
            choose_batch(&cfg, &t, Some("k"), &serde_json::json!(["a", "b"]), &qs, None).unwrap();
        assert_eq!(out.get("q0").unwrap().0, "file");
        assert_eq!(out.get("q1").unwrap().0, "error");
        assert_eq!(
            t.calls.load(Ordering::SeqCst),
            1,
            "both questions must go in a single request"
        );
    }

    #[test]
    fn judge_batch_issues_one_request_for_many_questions() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let cfg = DecisionConfig::default();
        let payload = serde_json::json!({
            "model": "m",
            "answers": {
                "a": { "type": "noul", "noul": 0.9 },
                "b": { "type": "noul", "noul": 0.2 }
            },
            "usage": { "input_tokens": 2, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        let qs = vec![
            ("a".to_string(), "is a real?".to_string()),
            ("b".to_string(), "is b real?".to_string()),
        ];
        let out = judge_batch(&cfg, &t, Some("k"), &serde_json::json!(["x", "y"]), &qs, None)
            .unwrap();
        assert_eq!(out.get("a"), Some(&0.9));
        assert_eq!(out.get("b"), Some(&0.2));
        assert_eq!(t.calls.load(Ordering::SeqCst), 1);
    }

    // ── R-12: hash-only replay fingerprint ──────────────────────────────
    #[test]
    fn input_fingerprint_is_stable_and_sensitive() {
        let s = serde_json::json!({ "a": 1 });
        let q = serde_json::json!({ "x": { "type": "noul", "instructions": "i" } });
        let f = input_fingerprint(&s, &q);
        assert_eq!(f, input_fingerprint(&s, &q), "same inputs → same hash");
        assert_eq!(f.len(), 16, "64-bit hex");
        assert_ne!(
            f,
            input_fingerprint(&serde_json::json!({ "a": 2 }), &q),
            "state change → different hash"
        );
        assert_ne!(
            f,
            input_fingerprint(&s, &serde_json::json!({ "x": { "type": "noul", "instructions": "j" } })),
            "questions change → different hash"
        );
    }

    #[test]
    fn usage_row_records_input_hash_but_not_state() {
        use std::sync::atomic::AtomicU32;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(concat!(include_str!("../migrations/016_decision_usage.sql"), "\n", include_str!("../migrations/020_decision_usage_hash.sql")))
            .unwrap();
        let cfg = DecisionConfig::default();
        let payload = serde_json::json!({
            "model": "m",
            "answers": { "ok": { "type": "noul", "noul": 0.9 } },
            "usage": { "input_tokens": 1, "cost": 0.0 }
        });
        let t = MockTransport { calls: AtomicU32::new(0), fail: false, payload };
        decision_request(
            &cfg,
            &t,
            Some("k"),
            &serde_json::json!("SECRET_STATE_TEXT"),
            &serde_json::json!({ "ok": { "type": "noul", "instructions": "i" } }),
            0,
            Some(&conn),
        )
        .unwrap();
        let (hash, answers): (String, String) = conn
            .query_row(
                "SELECT input_hash, answers FROM decision_usage LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(hash.len(), 16, "fingerprint persisted");
        assert!(!answers.contains("SECRET_STATE_TEXT"), "raw state never stored");
    }
}

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Outcome Tracker (Module 4)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecord {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub task_type: String,
    pub outcome: String,
    pub duration_s: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeStats {
    pub agent_id: String,
    pub task_type: String,
    pub project_id: Option<String>,
    pub total: i64,
    pub done: i64,
    pub failed: i64,
    pub revised: i64,
    pub avg_duration_s: Option<f64>,
    pub success_rate: f64,
}

pub fn record_outcome(
    db: &Connection,
    session_id: &str,
    agent_id: &str,
    task_type: &str,
    outcome: &str,
    duration_s: f64,
) -> Result<OutcomeRecord, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "UPDATE sessions SET outcome = ?1, outcome_at = ?2 WHERE id = ?3",
        rusqlite::params![outcome, now, session_id],
    )
    .map_err(|e| e.to_string())?;

    // Upsert stats
    db.execute(
        "INSERT INTO outcome_stats (agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s)
         VALUES (?1, ?2, NULL, 1, ?3, ?4, ?5, ?6)
         ON CONFLICT(agent_id, task_type, project_id) DO UPDATE SET
           total = total + 1,
           done = done + CASE WHEN ?3 = 1 THEN 1 ELSE 0 END,
           failed = failed + CASE WHEN ?4 = 1 THEN 1 ELSE 0 END,
           revised = revised + CASE WHEN ?5 = 1 THEN 1 ELSE 0 END,
           avg_duration_s = (avg_duration_s * (total - 1) + ?6) / total",
        rusqlite::params![
            agent_id,
            task_type,
            if outcome == "done" { 1 } else { 0 },
            if outcome == "failed" { 1 } else { 0 },
            if outcome == "revised" { 1 } else { 0 },
            duration_s,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(OutcomeRecord {
        id,
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        task_type: task_type.to_string(),
        outcome: outcome.to_string(),
        duration_s,
        created_at: now,
    })
}

pub fn get_outcome_stats(
    db: &Connection,
    project_id: Option<&str>,
    agent_id: Option<&str>,
) -> Result<Vec<OutcomeStats>, String> {
    let query = if let (Some(pid), Some(aid)) = (project_id, agent_id) {
        format!("SELECT agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s, CASE WHEN total > 0 THEN CAST(done AS REAL) / CAST(total AS REAL) ELSE 0 END as success_rate FROM outcome_stats WHERE project_id = '{}' AND agent_id = '{}' ORDER BY total DESC", pid, aid)
    } else if let Some(pid) = project_id {
        format!("SELECT agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s, CASE WHEN total > 0 THEN CAST(done AS REAL) / CAST(total AS REAL) ELSE 0 END as success_rate FROM outcome_stats WHERE project_id = '{}' ORDER BY total DESC", pid)
    } else if let Some(aid) = agent_id {
        format!("SELECT agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s, CASE WHEN total > 0 THEN CAST(done AS REAL) / CAST(total AS REAL) ELSE 0 END as success_rate FROM outcome_stats WHERE agent_id = '{}' ORDER BY total DESC", aid)
    } else {
        "SELECT agent_id, task_type, project_id, total, done, failed, revised, avg_duration_s, CASE WHEN total > 0 THEN CAST(done AS REAL) / CAST(total AS REAL) ELSE 0 END as success_rate FROM outcome_stats ORDER BY total DESC".to_string()
    };

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;
    let stats = stmt
        .query_map([], |row| {
            Ok(OutcomeStats {
                agent_id: row.get(0)?,
                task_type: row.get(1)?,
                project_id: row.get(2)?,
                total: row.get(3)?,
                done: row.get(4)?,
                failed: row.get(5)?,
                revised: row.get(6)?,
                avg_duration_s: row.get(7)?,
                success_rate: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(stats)
}

// ============================================================================
// Failure Analyzer (Module 7)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    pub id: String,
    pub session_id: String,
    pub pty_excerpt: String,
    pub diagnosis: Option<String>,
    pub root_cause: Option<String>,
    pub suggested_fix: Option<String>,
    pub confidence: f64,
    pub created_at: String,
}

pub fn create_failure_analysis(
    db: &Connection,
    session_id: &str,
    pty_excerpt: &str,
) -> Result<FailureAnalysis, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO failure_analyses (id, session_id, pty_excerpt, diagnosis, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4)",
        rusqlite::params![id, session_id, pty_excerpt, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(FailureAnalysis {
        id,
        session_id: session_id.to_string(),
        pty_excerpt: pty_excerpt.to_string(),
        diagnosis: None,
        root_cause: None,
        suggested_fix: None,
        confidence: 0.0,
        created_at: now,
    })
}

pub fn update_failure_diagnosis(
    db: &Connection,
    analysis_id: &str,
    diagnosis: &str,
    root_cause: &str,
    suggested_fix: &str,
    confidence: f64,
) -> Result<(), String> {
    db.execute(
        "UPDATE failure_analyses SET diagnosis = ?1, diagnosis = ?1 WHERE id = ?2",
        rusqlite::params![diagnosis, analysis_id],
    )
    .map_err(|e| e.to_string())?;

    // Store the more detailed fields in the diagnosis column as JSON
    let full = serde_json::json!({
        "diagnosis": diagnosis,
        "root_cause": root_cause,
        "suggested_fix": suggested_fix,
        "confidence": confidence,
    });
    db.execute(
        "UPDATE failure_analyses SET diagnosis = ?1 WHERE id = ?2",
        rusqlite::params![full.to_string(), analysis_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_failure_analyses(
    db: &Connection,
    session_id: Option<&str>,
    limit: i64,
) -> Result<Vec<FailureAnalysis>, String> {
    let query = if let Some(sid) = session_id {
        format!(
            "SELECT id, session_id, pty_excerpt, diagnosis, created_at FROM failure_analyses WHERE session_id = '{}' ORDER BY created_at DESC LIMIT {}",
            sid, limit
        )
    } else {
        format!(
            "SELECT id, session_id, pty_excerpt, diagnosis, created_at FROM failure_analyses ORDER BY created_at DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;
    let analyses = stmt
        .query_map([], |row| {
            let diag: Option<String> = row.get(3)?;
            let (root_cause, suggested_fix, confidence) = if let Some(ref d) = diag {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(d) {
                    (
                        v.get("root_cause")
                            .and_then(|s| s.as_str())
                            .map(String::from),
                        v.get("suggested_fix")
                            .and_then(|s| s.as_str())
                            .map(String::from),
                        v.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
                    )
                } else {
                    (None, None, 0.0)
                }
            } else {
                (None, None, 0.0)
            };

            Ok(FailureAnalysis {
                id: row.get(0)?,
                session_id: row.get(1)?,
                pty_excerpt: row.get(2)?,
                diagnosis: diag,
                root_cause,
                suggested_fix,
                confidence,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(analyses)
}

// ============================================================================
// Token Guard & Limit Detector (Module 18)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitEvent {
    pub id: String,
    pub session_id: String,
    pub plan_agent_id: Option<String>,
    pub event_type: String,
    pub raw_message: String,
    pub resolved: bool,
    pub resolved_at: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub id: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub context: String,
    pub model: Option<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub recorded_at: String,
}

// Known PTY error signatures for limit/rate events
pub const LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "too many requests",
    "quota exceeded",
    "token limit exceeded",
    "context window exceeded",
    "plan limit reached",
    "usage limit",
    "billing limit",
    "credit limit",
    "max tokens",
    "exceeded your quota",
    "rate limited",
    "you've reached the limit",
    "API rate limit",
    "insufficient credits",
];

pub fn detect_limit_event(raw_output: &str) -> Option<(String, &str)> {
    let lower = raw_output.to_lowercase();
    for pattern in LIMIT_PATTERNS {
        if lower.contains(pattern) {
            return Some(("limit_event".to_string(), pattern));
        }
    }
    None
}

pub fn record_limit_event(
    db: &Connection,
    session_id: &str,
    plan_agent_id: Option<&str>,
    event_type: &str,
    raw_message: &str,
) -> Result<LimitEvent, String> {
    let id = Uuid::new_v4().to_string();

    db.execute(
        "INSERT INTO limit_events (id, session_id, plan_agent_id, event_type, raw_message, resolved)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![id, session_id, plan_agent_id, event_type, raw_message],
    )
    .map_err(|e| e.to_string())?;

    Ok(LimitEvent {
        id,
        session_id: session_id.to_string(),
        plan_agent_id: plan_agent_id.map(String::from),
        event_type: event_type.to_string(),
        raw_message: raw_message.to_string(),
        resolved: false,
        resolved_at: None,
        resolution: None,
    })
}

pub fn resolve_limit_event(
    db: &Connection,
    event_id: &str,
    resolution: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    db.execute(
        "UPDATE limit_events SET resolved = 1, resolved_at = ?1, resolution = ?2 WHERE id = ?3",
        rusqlite::params![now, resolution, event_id],
    )
    .map_err(|e| e.to_string())?;

    // If limit-paused plan agent, we might want to unblock it
    if let Ok(plan_agent_id) = db.query_row::<String, _, _>(
        "SELECT COALESCE(plan_agent_id, '') FROM limit_events WHERE id = ?1",
        rusqlite::params![event_id],
        |row| row.get(0),
    ) {
        if !plan_agent_id.is_empty() {
            let _ = db.execute(
                "UPDATE plan_agents SET status = 'queued' WHERE id = ?1 AND status = 'limit-paused'",
                rusqlite::params![plan_agent_id],
            );
        }
    }

    Ok(())
}

pub fn get_unresolved_limits(
    db: &Connection,
    session_id: Option<&str>,
) -> Result<Vec<LimitEvent>, String> {
    let query = if let Some(sid) = session_id {
        format!(
            "SELECT id, session_id, plan_agent_id, event_type, raw_message, resolved, resolved_at, resolution FROM limit_events WHERE resolved = 0 AND session_id = '{}' ORDER BY id DESC",
            sid
        )
    } else {
        "SELECT id, session_id, plan_agent_id, event_type, raw_message, resolved, resolved_at, resolution FROM limit_events WHERE resolved = 0 ORDER BY id DESC".to_string()
    };

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;
    let events = stmt
        .query_map([], |row| {
            Ok(LimitEvent {
                id: row.get(0)?,
                session_id: row.get(1)?,
                plan_agent_id: row.get(2)?,
                event_type: row.get(3)?,
                raw_message: row.get(4)?,
                resolved: row.get::<_, i64>(5)? == 1,
                resolved_at: row.get(6)?,
                resolution: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(events)
}

pub fn record_token_usage(
    db: &Connection,
    session_id: &str,
    agent_id: Option<&str>,
    context: &str,
    model: Option<&str>,
    tokens_in: i64,
    tokens_out: i64,
) -> Result<TokenUsage, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO token_usage (id, session_id, agent_id, context, model, tokens_in, tokens_out, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, session_id, agent_id, context, model, tokens_in, tokens_out, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(TokenUsage {
        id,
        session_id: session_id.to_string(),
        agent_id: agent_id.map(String::from),
        context: context.to_string(),
        model: model.map(String::from),
        tokens_in,
        tokens_out,
        recorded_at: now,
    })
}

pub fn get_token_usage_stats(
    db: &Connection,
    session_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let query = if let Some(sid) = session_id {
        format!(
            "SELECT COALESCE(SUM(tokens_in), 0), COALESCE(SUM(tokens_out), 0) FROM token_usage WHERE session_id = '{}'",
            sid
        )
    } else {
        "SELECT COALESCE(SUM(tokens_in), 0), COALESCE(SUM(tokens_out), 0) FROM token_usage".to_string()
    };

    let (total_in, total_out): (i64, i64) = db
        .query_row(&query, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    // Get by-model breakdown
    let model_query = "SELECT model, COALESCE(SUM(tokens_in), 0), COALESCE(SUM(tokens_out), 0), COUNT(*) as calls FROM token_usage GROUP BY model ORDER BY calls DESC";
    let mut stmt = db.prepare(model_query).map_err(|e| e.to_string())?;
    let models: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "model": row.get::<_, Option<String>>(0)?,
                "tokens_in": row.get::<_, i64>(1)?,
                "tokens_out": row.get::<_, i64>(2)?,
                "calls": row.get::<_, i64>(3)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "total_tokens_in": total_in,
        "total_tokens_out": total_out,
        "total_tokens": total_in + total_out,
        "by_model": models,
    }))
}

// ============================================================================
// Session Heartbeat (Module 19)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthState {
    Healthy,
    Thinking,
    Stalled,
    Crashed,
    Unresponsive,
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthState::Healthy => write!(f, "HEALTHY"),
            HealthState::Thinking => write!(f, "THINKING"),
            HealthState::Stalled => write!(f, "STALLED"),
            HealthState::Crashed => write!(f, "CRASHED"),
            HealthState::Unresponsive => write!(f, "UNRESPONSIVE"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResult {
    pub session_id: String,
    pub state: String,
    pub last_activity_s: i64,
    pub checked_at: String,
}

pub fn run_heartbeat_check(
    db: &Connection,
    session_id: &str,
    last_activity_at: &str,
    pid_active: bool,
) -> Result<HeartbeatResult, String> {
    let now = Utc::now();
    let checked_at = now.to_rfc3339();

    let state = if !pid_active {
        HealthState::Crashed
    } else {
        let last: chrono::DateTime<Utc> = last_activity_at
            .parse()
            .unwrap_or_else(|_| Utc::now());
        let elapsed = now
            .signed_duration_since(last)
            .num_seconds();

        if elapsed > 600 {
            HealthState::Stalled // 10 min no activity
        } else if elapsed > 300 {
            HealthState::Thinking // 5 min thinking
        } else if elapsed < 30 {
            HealthState::Healthy
        } else {
            HealthState::Thinking
        }
    };

    let state_str = state.to_string();
    let last_activity_s = last_activity_at
        .parse::<chrono::DateTime<Utc>>()
        .map(|t| Utc::now().signed_duration_since(t).num_seconds())
        .unwrap_or(0);

    // Log heartbeat event
    let _ = db.execute(
        "INSERT INTO events (id, session_id, timestamp, agent_id, event_type, target, lines_added, lines_removed, exit_code)
         VALUES (?1, ?2, ?3, NULL, 'heartbeat', ?4, NULL, NULL, NULL)",
        rusqlite::params![Uuid::new_v4().to_string(), session_id, checked_at, state_str],
    );

    Ok(HeartbeatResult {
        session_id: session_id.to_string(),
        state: state_str,
        last_activity_s,
        checked_at,
    })
}

// ============================================================================
// Intelligence Layer (3 modes)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceRequest {
    pub mode: String, // "openrouter", "agent_spawn", "panel"
    pub prompt: String,
    pub context: serde_json::Value,
    pub max_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceResponse {
    pub content: String,
    pub tokens_used: Option<i64>,
    pub model_used: Option<String>,
    pub mode: String,
}

pub fn build_intelligence_prompt(task: &str, context: &str, format_hint: &str) -> String {
    format!(
        "You are ACC Intelligence, an internal analysis engine for the Agent Control Center.\n\
         Task: {}\n\n\
         Context:\n{}\n\n\
         Output format: {}\n\n\
         Provide only the requested format output. No preamble, no explanation.",
        task, context, format_hint
    )
}

// Session context extractor for PTY excerpt analysis
pub fn extract_pty_context(output: &str, last_n_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = if lines.len() > last_n_lines {
        lines.len() - last_n_lines
    } else {
        0
    };
    lines[start..].join("\n")
}

// Outcome suggestion from PTY patterns
pub fn suggest_outcome(pty_output: &str, idle_seconds: u64) -> Option<String> {
    let lower = pty_output.to_lowercase();

    if lower.contains("tests passed")
        || lower.contains("all tests passing")
        || lower.contains("✓")
        || lower.contains("success")
    {
        return Some("done".to_string());
    }

    if lower.contains("error:")
        || lower.contains("✗ ")
        || lower.contains("failed")
        || lower.contains("test failed")
    {
        return Some("failed".to_string());
    }

    if idle_seconds > 120 {
        return Some("stalled".to_string());
    }

    None
}

/// M2 (spec R21): classify the session outcome via the decision layer, falling
/// back to the keyword heuristic when the backend is unavailable.
pub fn suggest_outcome_decision(
    db: &Connection,
    pty_output: &str,
    idle_seconds: u64,
) -> Option<String> {
    let cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let key = std::env::var("OPENROUTER_API_KEY").ok();
    if key.is_some() || cfg.backend == "local" {
        let transport = crate::decision::UreqTransport { timeout_ms: cfg.timeout_ms };
        let tail = extract_pty_context(pty_output, 200);
        if let Ok(Some((label, _))) =
            crate::decision::classify_outcome(&cfg, &transport, key.as_deref(), &tail, Some(db))
        {
            return Some(label);
        }
    }
    suggest_outcome(pty_output, idle_seconds)
}

/// M2 (spec R23): confidence that a suggested fix addresses the root cause,
/// via a `noul`. Falls back to 0.0 (prior behaviour) when unavailable.
pub fn failure_confidence_decision(
    db: &Connection,
    diagnosis: &str,
    root_cause: &str,
    suggested_fix: &str,
) -> f64 {
    let cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let key = std::env::var("OPENROUTER_API_KEY").ok();
    if key.is_some() || cfg.backend == "local" {
        let transport = crate::decision::UreqTransport { timeout_ms: cfg.timeout_ms };
        let state = serde_json::json!({
            "diagnosis": diagnosis,
            "root_cause": root_cause,
            "suggested_fix": suggested_fix,
        });
        if let Ok(Some(p)) = crate::decision::judge(
            &cfg,
            &transport,
            key.as_deref(),
            &state,
            "Does `suggested_fix` address `root_cause`?",
            "addresses",
            Some(db),
        ) {
            return p;
        }
    }
    0.0
}

// ============================================================================
// GAP 3: Intelligence subagent pattern detection
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentDetection {
    pub agent_id: String,
    pub family: String,
    pub pattern: String,
    pub count: i64,
    pub last_seen: String,
}

pub const SUBAGENT_PATTERNS: &[(&str, &str, &str)] = &[
    ("claude", "task-tool", "Dispatching subagent|Agent\\d+ started|task(subagent_type"),
    ("opencode", "task-tool", "Dispatching subagent|subagent_type"),
    ("qwen-code", "task-tool", "Dispatching subagent|subagent_type"),
    ("gemini", "gemini", "Delegating|subagent.*started|/agents\\s"),
    ("codex", "codex", "spawn_agent|Spawned agent"),
    ("cline", "cline", "Spawning subagent|Sub-task started"),
    ("cursor", "cursor", "Background agent|Parallel agent"),
];

pub fn get_subagent_patterns() -> Vec<SubagentDetection> {
    SUBAGENT_PATTERNS.iter().map(|(id, family, pattern)| {
        SubagentDetection {
            agent_id: id.to_string(),
            family: family.to_string(),
            pattern: pattern.to_string(),
            count: 0,
            last_seen: String::new(),
        }
    }).collect()
}

pub fn detect_subagent_activity(pty_output: &str, patterns: &[&str]) -> Vec<String> {
    let mut detections = Vec::new();
    for line in pty_output.lines() {
        for pattern in patterns {
            if line.contains(pattern) {
                detections.push(line.trim().to_string());
                break;
            }
        }
    }
    detections
}

// ============================================================================
// GAP 4: Token Guard subagent attribution
// ============================================================================

pub fn record_subagent_token_usage(
    db: &Connection,
    session_id: &str,
    parent_agent_id: Option<&str>,
    subagent_agent_id: Option<&str>,
    context: &str,
    model: Option<&str>,
    tokens_in: i64,
    tokens_out: i64,
) -> Result<TokenUsage, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let agent = subagent_agent_id.or(parent_agent_id);

    db.execute(
        "INSERT INTO token_usage (id, session_id, agent_id, context, model, tokens_in, tokens_out, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, session_id, agent, context, model, tokens_in, tokens_out, now],
    ).map_err(|e| e.to_string())?;

    Ok(TokenUsage {
        id,
        session_id: session_id.to_string(),
        agent_id: agent.map(String::from),
        context: context.to_string(),
        model: model.map(String::from),
        tokens_in,
        tokens_out,
        recorded_at: now,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub priority: Priority,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Normal,
    High,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: u32,
}

pub async fn invoke_with_backoff(
    request: OpenRouterRequest,
    api_key: &str,
    max_retries: u32,
) -> Result<OpenRouterResponse, String> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = 2u64.pow(attempt) * 500;
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
        match invoke_openrouter(&request, api_key).await {
            Ok(r) => return Ok(r),
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    Err(last_err)
}

async fn invoke_openrouter(
    request: &OpenRouterRequest,
    api_key: &str,
) -> Result<OpenRouterResponse, String> {
    let model = request.model.as_deref().unwrap_or("anthropic/claude-3.5-sonnet");
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": &request.prompt}],
        "max_tokens": request.max_tokens.unwrap_or(4096),
        "temperature": request.temperature.unwrap_or(0.7),
    });
    let resp: ureq::Response = ureq::post("https://openrouter.ai/api/v1/chat/completions")
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| format!("OpenRouter request failed: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| format!("Parse response: {e}"))?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let tokens_used = json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32;
    Ok(OpenRouterResponse {
        content,
        model: model.to_string(),
        tokens_used,
    })
}

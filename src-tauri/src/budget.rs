use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudget {
    pub id: String,
    pub session_id: Option<String>,
    pub plan_agent_id: Option<String>,
    pub agent_id: String,
    pub task_complexity: Option<String>,
    pub model: Option<String>,
    pub budget_total: i64,
    pub budget_used: i64,
    pub state: String,
    pub wip_path: Option<String>,
    pub usage_percent: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetInput {
    pub session_id: Option<String>,
    pub plan_agent_id: Option<String>,
    pub agent_id: String,
    pub task_complexity: Option<String>,
    pub model: Option<String>,
    pub budget_total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveResumptionPlan {
    pub id: String,
    pub wave_id: String,
    pub pending_task_id: Option<String>,
    pub plan_path: String,
    pub agents_completed: Option<String>,
    pub agents_wipd: Option<String>,
    pub agents_pending: Option<String>,
    pub estimated_remaining_tokens: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub model: Option<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetThresholds {
    pub warning_pct: f64,
    pub critical_pct: f64,
    pub halt_pct: f64,
}

// ============================================================================
// Budget Auto-Sizing
// ============================================================================

fn resolve_budget_total(input: &BudgetInput) -> i64 {
    if let Some(total) = input.budget_total {
        return total;
    }
    match input.task_complexity.as_deref() {
        Some("low") => 50_000,
        Some("medium") => 200_000,
        Some("high") => 500_000,
        _ => 1_000_000,
    }
}

// ============================================================================
// Threshold Evaluation
// ============================================================================

fn evaluate_thresholds(budget_used: i64, budget_total: i64) -> String {
    if budget_total <= 0 {
        return "active".to_string();
    }
    let pct = (budget_used as f64 / budget_total as f64) * 100.0;
    if pct >= 100.0 {
        "exceeded".to_string()
    } else if pct >= 90.0 {
        "critical".to_string()
    } else if pct >= 70.0 {
        "warning".to_string()
    } else {
        "active".to_string()
    }
}

fn compute_usage_percent(budget_used: i64, budget_total: i64) -> f64 {
    if budget_total <= 0 {
        return 0.0;
    }
    (budget_used as f64 / budget_total as f64) * 100.0
}

fn build_agent_budget(row: &rusqlite::Row) -> rusqlite::Result<AgentBudget> {
    // Column order from the SELECT: 0 id, 1 session_id, 2 plan_agent_id,
    // 3 agent_id, 4 task_complexity, 5 model, 6 budget_total, 7 budget_used,
    // 8 state, 9 wip_path, 10 created_at, 11 updated_at.
    let budget_total: i64 = row.get(6)?;
    let budget_used: i64 = row.get(7)?;
    Ok(AgentBudget {
        id: row.get(0)?,
        session_id: row.get(1)?,
        plan_agent_id: row.get(2)?,
        agent_id: row.get(3)?,
        task_complexity: row.get(4)?,
        model: row.get(5)?,
        budget_total,
        budget_used,
        state: row.get(8)?,
        wip_path: row.get(9)?,
        usage_percent: compute_usage_percent(budget_used, budget_total),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

// ============================================================================
// Public API Functions
// ============================================================================

pub fn create_budget(db: &Connection, input: &BudgetInput) -> Result<AgentBudget, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let budget_total = resolve_budget_total(input);

    db.execute(
        "INSERT INTO agent_budgets (id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 'active', NULL, ?8, ?9)",
        rusqlite::params![
            id,
            input.session_id,
            input.plan_agent_id,
            input.agent_id,
            input.task_complexity,
            input.model,
            budget_total,
            now,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(AgentBudget {
        id,
        session_id: input.session_id.clone(),
        plan_agent_id: input.plan_agent_id.clone(),
        agent_id: input.agent_id.clone(),
        task_complexity: input.task_complexity.clone(),
        model: input.model.clone(),
        budget_total,
        budget_used: 0,
        state: "active".to_string(),
        wip_path: None,
        usage_percent: 0.0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn get_budgets(
    db: &Connection,
    session_id: Option<&str>,
    plan_agent_id: Option<&str>,
    state: Option<&str>,
) -> Result<Vec<AgentBudget>, String> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(sid) = session_id {
        conditions.push(format!("session_id = ?{}", params.len() + 1));
        params.push(Box::new(sid.to_string()));
    }
    if let Some(pid) = plan_agent_id {
        conditions.push(format!("plan_agent_id = ?{}", params.len() + 1));
        params.push(Box::new(pid.to_string()));
    }
    if let Some(st) = state {
        conditions.push(format!("state = ?{}", params.len() + 1));
        params.push(Box::new(st.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let query = format!(
        "SELECT id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at FROM agent_budgets{} ORDER BY created_at DESC",
        where_clause
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;
    let budgets = stmt
        .query_map(param_refs.as_slice(), build_agent_budget)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(budgets)
}

pub fn update_budget_usage(
    db: &Connection,
    budget_id: &str,
    tokens_used: i64,
) -> Result<AgentBudget, String> {
    let now = Utc::now().to_rfc3339();

    db.execute(
        "UPDATE agent_budgets SET budget_used = budget_used + ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![tokens_used, now, budget_id],
    )
    .map_err(|e| e.to_string())?;

    let row = db
        .query_row(
            "SELECT budget_used, budget_total FROM agent_budgets WHERE id = ?1",
            rusqlite::params![budget_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let (budget_used, budget_total) = row;
    let new_state = evaluate_thresholds(budget_used, budget_total);

    db.execute(
        "UPDATE agent_budgets SET state = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![new_state, now, budget_id],
    )
    .map_err(|e| e.to_string())?;

    db.query_row(
        "SELECT id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at FROM agent_budgets WHERE id = ?1",
        rusqlite::params![budget_id],
        build_agent_budget,
    )
    .map_err(|e| e.to_string())
}

pub fn capture_wip(
    db: &Connection,
    budget_id: &str,
    wip_path: &str,
) -> Result<AgentBudget, String> {
    let now = Utc::now().to_rfc3339();

    db.execute(
        "UPDATE agent_budgets SET state = 'exceeded', wip_path = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![wip_path, now, budget_id],
    )
    .map_err(|e| e.to_string())?;

    db.query_row(
        "SELECT id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at FROM agent_budgets WHERE id = ?1",
        rusqlite::params![budget_id],
        build_agent_budget,
    )
    .map_err(|e| e.to_string())
}

pub fn resume_budget(
    db: &Connection,
    budget_id: &str,
    additional_tokens: i64,
) -> Result<AgentBudget, String> {
    let now = Utc::now().to_rfc3339();

    db.execute(
        "UPDATE agent_budgets SET budget_total = budget_total + ?1, state = 'active', wip_path = NULL, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![additional_tokens, now, budget_id],
    )
    .map_err(|e| e.to_string())?;

    db.query_row(
        "SELECT id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at FROM agent_budgets WHERE id = ?1",
        rusqlite::params![budget_id],
        build_agent_budget,
    )
    .map_err(|e| e.to_string())
}

pub fn create_resumption_plan(
    db: &Connection,
    wave_id: &str,
    pending_task_id: Option<&str>,
    plan_path: &str,
    agents_completed: &[String],
    agents_wipd: &[String],
    agents_pending: &[String],
    estimated_remaining_tokens: Option<i64>,
) -> Result<WaveResumptionPlan, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let completed_str = if agents_completed.is_empty() {
        None
    } else {
        Some(agents_completed.join(","))
    };
    let wipd_str = if agents_wipd.is_empty() {
        None
    } else {
        Some(agents_wipd.join(","))
    };
    let pending_str = if agents_pending.is_empty() {
        None
    } else {
        Some(agents_pending.join(","))
    };

    db.execute(
        "INSERT INTO wave_resumption_plans (id, wave_id, pending_task_id, plan_path, agents_completed, agents_wipd, agents_pending, estimated_remaining_tokens, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            wave_id,
            pending_task_id,
            plan_path,
            completed_str,
            wipd_str,
            pending_str,
            estimated_remaining_tokens,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(WaveResumptionPlan {
        id,
        wave_id: wave_id.to_string(),
        pending_task_id: pending_task_id.map(String::from),
        plan_path: plan_path.to_string(),
        agents_completed: completed_str,
        agents_wipd: wipd_str,
        agents_pending: pending_str,
        estimated_remaining_tokens,
        created_at: now,
    })
}

pub fn get_resumption_plan(
    db: &Connection,
    wave_id: &str,
) -> Result<Option<WaveResumptionPlan>, String> {
    let result = db.query_row(
        "SELECT id, wave_id, pending_task_id, plan_path, agents_completed, agents_wipd, agents_pending, estimated_remaining_tokens, created_at FROM wave_resumption_plans WHERE wave_id = ?1 ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![wave_id],
        |row| {
            Ok(WaveResumptionPlan {
                id: row.get(0)?,
                wave_id: row.get(1)?,
                pending_task_id: row.get(2)?,
                plan_path: row.get(3)?,
                agents_completed: row.get(4)?,
                agents_wipd: row.get(5)?,
                agents_pending: row.get(6)?,
                estimated_remaining_tokens: row.get(7)?,
                created_at: row.get(8)?,
            })
        },
    );

    match result {
        Ok(plan) => Ok(Some(plan)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_cost_breakdown(
    db: &Connection,
    session_id: &str,
) -> Result<Vec<CostBreakdown>, String> {
    let query = "SELECT tu.model, COALESCE(SUM(tu.tokens_in), 0), COALESCE(SUM(tu.tokens_out), 0), mc.cost_per_1k_input, mc.cost_per_1k_output FROM token_usage tu LEFT JOIN model_costs mc ON tu.model = mc.model_id WHERE tu.session_id = ?1 GROUP BY tu.model";

    let mut stmt = db.prepare(query).map_err(|e| e.to_string())?;
    let breakdowns = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let tokens_in: i64 = row.get(1)?;
            let tokens_out: i64 = row.get(2)?;
            let cost_in: Option<f64> = row.get(3)?;
            let cost_out: Option<f64> = row.get(4)?;

            let estimated_cost_usd = match (cost_in, cost_out) {
                (Some(ci), Some(co)) => {
                    (tokens_in as f64 / 1000.0) * ci + (tokens_out as f64 / 1000.0) * co
                }
                (Some(ci), None) => (tokens_in as f64 / 1000.0) * ci,
                (None, Some(co)) => (tokens_out as f64 / 1000.0) * co,
                (None, None) => 0.0,
            };

            Ok(CostBreakdown {
                model: row.get(0)?,
                tokens_in,
                tokens_out,
                estimated_cost_usd,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(breakdowns)
}

pub fn check_budget_thresholds(
    db: &Connection,
) -> Result<Vec<AgentBudget>, String> {
    let query = "SELECT id, session_id, plan_agent_id, agent_id, task_complexity, model, budget_total, budget_used, state, wip_path, created_at, updated_at FROM agent_budgets WHERE state IN ('warning', 'critical', 'exceeded') ORDER BY created_at DESC";

    let mut stmt = db.prepare(query).map_err(|e| e.to_string())?;
    let budgets = stmt
        .query_map([], build_agent_budget)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(budgets)
}

// === W5.C: Threshold Ladder ===

use crate::events::with_app_handle;
use crate::pty::PtyManager;
use serde_json::json;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

pub const THRESHOLD_LADDER: &[(f64, &str)] = &[
    (
        60.0,
        "INFO: 60% of token budget used. Continue but be aware.",
    ),
    (
        80.0,
        "WARNING: 80% of budget used. Wrap up current task and write HANDOFF_*.md soon.",
    ),
    (
        95.0,
        "CRITICAL: 95% of budget used. Stop new work, write WIP_CHECKPOINT.md NOW.",
    ),
    (
        100.0,
        "BUDGET EXHAUSTED: Stop immediately. Write WIP_CHECKPOINT.md and exit.",
    ),
];

pub struct ThresholdLadder {
    last_fired: Arc<Mutex<Option<i64>>>,
    db: Arc<Mutex<Connection>>,
    pty_manager: Arc<PtyManager>,
}

impl ThresholdLadder {
    pub fn new(db: Arc<Mutex<Connection>>, pty_manager: Arc<PtyManager>) -> Self {
        Self {
            last_fired: Arc::new(Mutex::new(None)),
            db,
            pty_manager,
        }
    }

    pub async fn check_and_fire(
        &self,
        agent_ref: &str,
        current_pct: f64,
    ) -> Result<Option<String>, String> {
        let mut last = self.last_fired.lock().await;
        for (pct, msg) in THRESHOLD_LADDER.iter() {
            if current_pct >= *pct && last.map_or(true, |last_val| *pct as i64 > last_val) {
                *last = Some(*pct as i64);

                if let Err(e) = self.pty_manager.write_to_process(agent_ref, msg).await {
                    eprintln!("[budget] PTY injection failed: {e}");
                }

                with_app_handle(|h| {
                    let _ = h.emit(
                        "budget-threshold-fired",
                        json!({
                            "agent_ref": agent_ref,
                            "percentage": pct,
                            "message": msg,
                        }),
                    );
                });

                return Ok(Some(msg.to_string()));
            }
        }
        Ok(None)
    }

    pub async fn reset(&self) {
        let mut last = self.last_fired.lock().await;
        *last = None;
    }
}

pub async fn check_budget_thresholds_for_all(
    db: &Connection,
    pty_manager: &PtyManager,
) -> Result<Vec<String>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, agent_id, budget_used, budget_total FROM agent_budgets WHERE state IN ('warning', 'critical', 'exceeded')",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut fired = Vec::new();
    for (_id, agent_id, used, total) in rows {
        if total <= 0 {
            continue;
        }
        let pct = (used as f64 / total as f64) * 100.0;
        for (threshold, msg) in THRESHOLD_LADDER.iter() {
            if pct >= *threshold {
                fired.push(format!(
                    "Budget/agent {agent_id} at {pct:.1}% — {msg}"
                ));
            }
        }
    }
    let _ = pty_manager;
    Ok(fired)
}

// === W5.C: WIP_CHECKPOINT.md Detection ===

pub async fn check_for_wip_checkpoint(
    project_path: &str,
) -> Result<Option<String>, String> {
    let wip_path = std::path::Path::new(project_path).join("WIP_CHECKPOINT.md");
    if !wip_path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&wip_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(content))
}

pub fn generate_fallback_wip_sync(
    db: &Connection,
    session_id: &str,
) -> Result<String, String> {
    let mut stmt = db
        .prepare(
            "SELECT event_type, timestamp FROM events WHERE session_id = ?1 ORDER BY timestamp",
        )
        .map_err(|e| e.to_string())?;
    let events: Vec<(String, String)> = match stmt.query_map(
        rusqlite::params![session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ) {
        Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
        Err(_) => Vec::new(),
    };

    let mut wip = String::from("# WIP_CHECKPOINT (Auto-generated)\n\n");
    wip.push_str(&format!("Session: {session_id}\n"));
    wip.push_str(&format!("Generated: {}\n\n", Utc::now().to_rfc3339()));
    wip.push_str("## Session Events\n\n");
    for (etype, ts) in events {
        wip.push_str(&format!("- {ts} — {etype}\n"));
    }
    Ok(wip)
}

pub async fn watch_for_wip_checkpoint(
    project_path: &str,
) -> Result<notify::RecommendedWatcher, String> {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    let (tx, _rx) = channel::<notify::Result<notify::Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| e.to_string())?;
    let path = std::path::Path::new(project_path);
    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    Ok(watcher)
}

// === W5.C: Wave Resumption Plan ===

pub fn create_resumption_plan_from_wips(wips: Vec<String>) -> String {
    let mut plan = String::from("# Wave Resumption Plan\n\n");
    let mut tasks: Vec<String> = Vec::new();

    for wip in wips {
        for line in wip.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]") {
                tasks.push(trimmed.to_string());
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for t in tasks {
        let key = t.replace("- [x]", "- [ ]");
        if !seen.contains(&key) {
            seen.insert(key);
            unique.push(t);
        }
    }

    plan.push_str("## Pending Tasks\n");
    if unique.is_empty() {
        plan.push_str("_No outstanding tasks parsed from WIP files._\n");
    } else {
        for t in unique {
            plan.push_str("- ");
            plan.push_str(&t);
            plan.push('\n');
        }
    }
    plan.push_str(&format!(
        "\nGenerated: {}\n",
        Utc::now().to_rfc3339()
    ));
    plan
}

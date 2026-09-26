// src-tauri/src/pipeline_store.rs
//
// Persistence for supervised build runs (SPEC-001 §5 DG-1, Step 3.1).
// The pipeline is resumable: every stage transition is appended to
// `stage_log`, and non-terminal runs can be resumed by project path.

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_RUNNING: &str = "running";
pub const STATUS_AWAITING_USER: &str = "awaiting_user";
pub const STATUS_SUCCEEDED: &str = "succeeded";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_CANCELLED: &str = "cancelled";

pub const TERMINAL_STATUSES: &[&str] = &[STATUS_SUCCEEDED, STATUS_FAILED, STATUS_CANCELLED];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageLogEntry {
    pub stage: String,
    pub status: String,
    pub message: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildRun {
    pub id: String,
    pub project_id: Option<String>,
    pub spec_path: String,
    pub project_path: String,
    pub stack_id: Option<String>,
    pub status: String,
    pub current_stage: Option<String>,
    pub stage_log: Vec<StageLogEntry>,
    pub report: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl StageLogEntry {
    pub fn new(stage: &str, status: &str, message: &str) -> Self {
        Self {
            stage: stage.to_string(),
            status: status.to_string(),
            message: message.to_string(),
            at: Utc::now().to_rfc3339(),
        }
    }
}

fn row_to_run(row: &rusqlite::Row) -> rusqlite::Result<BuildRun> {
    let stage_log_json: String = row.get(7)?;
    let stage_log: Vec<StageLogEntry> =
        serde_json::from_str(&stage_log_json).unwrap_or_default();
    let report_json: Option<String> = row.get(8)?;
    let report: Option<serde_json::Value> =
        report_json.and_then(|s| serde_json::from_str(&s).ok());

    Ok(BuildRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        spec_path: row.get(2)?,
        project_path: row.get(3)?,
        stack_id: row.get(4)?,
        status: row.get(5)?,
        current_stage: row.get(6)?,
        stage_log,
        report,
        error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const SELECT_COLUMNS: &str = "id, project_id, spec_path, project_path, stack_id, status, \
     current_stage, stage_log, report, error, created_at, updated_at";

pub fn create_run(
    db: &Connection,
    project_id: Option<&str>,
    spec_path: &str,
    project_path: &str,
    stack_id: Option<&str>,
) -> Result<BuildRun, String> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO build_runs (id, project_id, spec_path, project_path, stack_id, status, stage_log, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '[]', ?7, ?7)",
        rusqlite::params![id, project_id, spec_path, project_path, stack_id, STATUS_PENDING, now],
    )
    .map_err(|e| e.to_string())?;
    get_run(db, &id)
}

pub fn get_run(db: &Connection, id: &str) -> Result<BuildRun, String> {
    db.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM build_runs WHERE id = ?1"),
        rusqlite::params![id],
        row_to_run,
    )
    .map_err(|e| e.to_string())
}

/// Find the most recent non-terminal run for a project path (resume support).
pub fn find_resumable_run(
    db: &Connection,
    project_path: &str,
) -> Result<Option<BuildRun>, String> {
    let mut stmt = db
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM build_runs
             WHERE project_path = ?1 AND status IN ('pending','running','awaiting_user')
             ORDER BY created_at DESC LIMIT 1"
        ))
        .map_err(|e| e.to_string())?;

    let mut rows = stmt.query_map(rusqlite::params![project_path], row_to_run)
        .map_err(|e| e.to_string())?;
    match rows.next() {
        Some(run) => Ok(Some(run.map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

pub fn update_status(
    db: &Connection,
    id: &str,
    status: &str,
    current_stage: Option<&str>,
) -> Result<(), String> {
    db.execute(
        "UPDATE build_runs SET status = ?1, current_stage = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![status, current_stage, Utc::now().to_rfc3339(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn append_stage_log(
    db: &Connection,
    id: &str,
    entry: &StageLogEntry,
) -> Result<(), String> {
    let run = get_run(db, id)?;
    let mut log = run.stage_log;
    log.push(entry.clone());
    let json = serde_json::to_string(&log).map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE build_runs SET stage_log = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![json, Utc::now().to_rfc3339(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_report(
    db: &Connection,
    id: &str,
    report: &serde_json::Value,
) -> Result<(), String> {
    let json = serde_json::to_string(report).map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE build_runs SET report = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![json, Utc::now().to_rfc3339(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_error(db: &Connection, id: &str, error: &str) -> Result<(), String> {
    db.execute(
        "UPDATE build_runs SET error = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![error, Utc::now().to_rfc3339(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Cancel a run. Only non-terminal runs can be cancelled.
pub fn cancel_run(db: &Connection, id: &str) -> Result<bool, String> {
    let run = get_run(db, id)?;
    if TERMINAL_STATUSES.contains(&run.status.as_str()) {
        return Ok(false);
    }
    update_status(db, id, STATUS_CANCELLED, run.current_stage.as_deref())?;
    Ok(true)
}

pub fn is_cancelled(db: &Connection, id: &str) -> Result<bool, String> {
    Ok(get_run(db, id)?.status == STATUS_CANCELLED)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/017_build_runs.sql"))
            .unwrap();
        conn
    }

    #[test]
    fn test_create_and_get_build_run() {
        let conn = setup_db();
        let run = create_run(
            &conn,
            Some("proj-1"),
            "docs/spec.md",
            "/tmp/project",
            Some("nextjs-supabase-vercel"),
        )
        .unwrap();

        assert_eq!(run.status, STATUS_PENDING);
        assert_eq!(run.project_path, "/tmp/project");
        assert!(run.stage_log.is_empty());

        let loaded = get_run(&conn, &run.id).unwrap();
        assert_eq!(loaded.id, run.id);
        assert_eq!(loaded.stack_id.as_deref(), Some("nextjs-supabase-vercel"));
    }

    #[test]
    fn test_stage_log_appends_in_order() {
        let conn = setup_db();
        let run = create_run(&conn, None, "spec.md", "/tmp/p", None).unwrap();

        append_stage_log(&conn, &run.id, &StageLogEntry::new("parse_spec", "done", "3 tasks")).unwrap();
        append_stage_log(&conn, &run.id, &StageLogEntry::new("resolve_stack", "done", "nextjs")).unwrap();

        let loaded = get_run(&conn, &run.id).unwrap();
        assert_eq!(loaded.stage_log.len(), 2);
        assert_eq!(loaded.stage_log[0].stage, "parse_spec");
        assert_eq!(loaded.stage_log[1].stage, "resolve_stack");
    }

    #[test]
    fn test_resume_returns_non_terminal_run() {
        let conn = setup_db();
        let run = create_run(&conn, None, "spec.md", "/tmp/p", None).unwrap();
        update_status(&conn, &run.id, STATUS_AWAITING_USER, Some("provision")).unwrap();

        let found = find_resumable_run(&conn, "/tmp/p").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, run.id);

        // Terminal runs are not resumable.
        update_status(&conn, &run.id, STATUS_SUCCEEDED, Some("report")).unwrap();
        assert!(find_resumable_run(&conn, "/tmp/p").unwrap().is_none());
    }

    #[test]
    fn test_cancel_sets_cancelled_and_is_idempotent() {
        let conn = setup_db();
        let run = create_run(&conn, None, "spec.md", "/tmp/p", None).unwrap();
        update_status(&conn, &run.id, STATUS_RUNNING, Some("execute_waves")).unwrap();

        assert!(cancel_run(&conn, &run.id).unwrap());
        assert!(is_cancelled(&conn, &run.id).unwrap());

        // Cancelling a terminal run is a no-op.
        assert!(!cancel_run(&conn, &run.id).unwrap());
    }

    #[test]
    fn test_report_roundtrip() {
        let conn = setup_db();
        let run = create_run(&conn, None, "spec.md", "/tmp/p", None).unwrap();
        let report = serde_json::json!({"status": "succeeded", "stages": []});
        set_report(&conn, &run.id, &report).unwrap();

        let loaded = get_run(&conn, &run.id).unwrap();
        assert_eq!(loaded.report.unwrap()["status"], "succeeded");
    }
}

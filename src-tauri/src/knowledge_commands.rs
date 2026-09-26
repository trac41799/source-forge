// src-tauri/src/knowledge_commands.rs
//
// Tauri command wrappers for Knowledge Compounder + preflight warnings
// (SPEC-001 §3.2 / §5 DG-3).
//
// These live in their own module (not commands.rs) so parallel work streams
// never edit the same file — see SPEC-001 §4.2 rule 2/3.
//
// `run_compounder_cmd` is intentionally split into three steps so the async
// LLM call never holds the `MutexGuard<Connection>` across an await
// (`&Connection` is `!Send`):
//   1. prepare prompt (sync, lock scoped)
//   2. complete via `compounder_llm` (async, no lock)
//   3. merge into the knowledge base (sync, lock re-acquired)

use tauri::State;

use crate::commands::AppState;
use crate::knowledge::{KnowledgeItem, PreflightWarning};

const PREFLIGHT_LIMIT: i64 = 10;

#[tauri::command]
pub async fn run_compounder_cmd(
    state: State<'_, AppState>,
    session_id: String,
    project_id: Option<String>,
) -> Result<Vec<KnowledgeItem>, String> {
    let prompt = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        crate::knowledge::compounder_prepare_prompt(&db, &session_id)?
    };

    let Some(prompt) = prompt else {
        return Ok(Vec::new());
    };

    let content = crate::compounder_llm::complete(
        &crate::compounder_llm::LlmProvider::OpenRouter,
        &prompt,
    )
    .await?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    crate::knowledge::compounder_merge(&db, &session_id, project_id.as_deref(), &content)
}

#[tauri::command]
pub fn get_preflight_warnings_cmd(
    state: State<'_, AppState>,
    stack: String,
) -> Result<Vec<PreflightWarning>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    crate::knowledge::get_preflight_warnings(&db, &stack, PREFLIGHT_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/010_knowledge_graph.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/014_bagua_semantics.sql"))
            .unwrap();
        conn
    }

    fn seed_session_events(conn: &Connection, session_id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, started_at) VALUES (?1, datetime('now'))",
            rusqlite::params![session_id],
        )
        .unwrap();
        for i in 0..2 {
            conn.execute(
                "INSERT INTO events (id, session_id, timestamp, event_type, target, lines_added, lines_removed)
                 VALUES (?1, ?2, datetime('now'), 'file_edit', 'src/registry.rs', 10, 5)",
                rusqlite::params![format!("ev-{i}"), session_id],
            )
            .unwrap();
        }
    }

    #[test]
    fn test_prepare_prompt_none_without_events() {
        let conn = setup_db();
        let prompt = crate::knowledge::compounder_prepare_prompt(&conn, "s-empty").unwrap();
        assert!(prompt.is_none());
    }

    #[test]
    fn test_prepare_prompt_includes_candidate_text() {
        let conn = setup_db();
        seed_session_events(&conn, "s1");
        let prompt = crate::knowledge::compounder_prepare_prompt(&conn, "s1")
            .unwrap()
            .expect("prompt expected");
        assert!(prompt.contains("src/registry.rs"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn test_run_compounder_pipeline_with_mock_llm() {
        let conn = setup_db();
        seed_session_events(&conn, "s1");

        let prompt = crate::knowledge::compounder_prepare_prompt(&conn, "s1")
            .unwrap()
            .unwrap();
        let mock = crate::compounder_llm::LlmProvider::Static(
            r#"[{"title":"Registry LazyLock","content":"Use LazyLock for the stack registry","category":"pattern","confidence":0.8}]"#
                .to_string(),
        );
        let content = tauri::async_runtime::block_on(crate::compounder_llm::complete(
            &mock, &prompt,
        ))
        .unwrap();

        let items =
            crate::knowledge::compounder_merge(&conn, "s1", None, &content).unwrap();
        assert_eq!(items.len(), 1, "one knowledge item expected");
        assert_eq!(items[0].r#type, "pattern");
        assert_eq!(items[0].title, "Registry LazyLock");
    }

    #[test]
    fn test_compounder_merge_dedupes_similar_item() {
        let conn = setup_db();
        let content = r#"[{"title":"Registry LazyLock","content":"Use LazyLock for the stack registry","category":"pattern","confidence":0.8}]"#;

        let first = crate::knowledge::compounder_merge(&conn, "s1", None, content).unwrap();
        assert_eq!(first.len(), 1);

        let second = crate::knowledge::compounder_merge(&conn, "s1", None, content).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].confirmation_count, 2,
            "second run must merge into the existing item"
        );

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM knowledge_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 1, "no duplicate rows");
    }

    #[test]
    fn test_get_preflight_warnings_returns_rows() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, stack_tags, first_seen, last_confirmed)
             VALUES ('k1', 'antipattern', 'Avoid unwrap in prod', 'Use Result instead', 0.9, 3, 0, 'active', 'rust', datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();

        let warnings =
            crate::knowledge::get_preflight_warnings(&conn, "rust", PREFLIGHT_LIMIT).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].title, "Avoid unwrap in prod");

        let other =
            crate::knowledge::get_preflight_warnings(&conn, "python", PREFLIGHT_LIMIT).unwrap();
        assert!(other.is_empty(), "unrelated stack must not match rust tags");
    }
}

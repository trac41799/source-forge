use crate::backward_channel;
use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

pub fn get_db_path(app: &tauri::App) -> PathBuf {
    let app_data_dir = app.path().app_data_dir().expect("Failed to get app data directory");
    fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
    app_data_dir.join("acc.db")
}

fn apply_migrations(conn: &Connection) -> Result<()> {
    let migrations: &[&str] = &[
        include_str!("../migrations/001_init.sql"),
        include_str!("../migrations/002_assets.sql"),
        include_str!("../migrations/003_integrations.sql"),
    ];

    for (i, sql) in migrations.iter().enumerate() {
        if let Err(e) = conn.execute_batch(sql) {
            eprintln!("Migration {:03} (non-fatal): {e}", i + 1);
        }
    }

    if let Err(e) = backward_channel::init_backward_channel_tables(conn)
    {
        eprintln!("Migration 004 (non-fatal): {e}");
    }

    // Migrations 008+ use ALTER TABLE which fails if columns exist.
    // Run them individually — ignore errors.
    let late: &[(&str, &str)] = &[
        ("008", include_str!("../migrations/008_control_sessions.sql")),
        ("010", include_str!("../migrations/010_knowledge_graph.sql")),
        ("011", include_str!("../migrations/011_memory.sql")),
        ("012", include_str!("../migrations/012_codebase_exploration.sql")),
        ("013", include_str!("../migrations/013_app_state_snapshot.sql")),
        ("014", include_str!("../migrations/014_bagua_semantics.sql")),
        ("015", include_str!("../migrations/015_user_preferences.sql")),
        ("016", include_str!("../migrations/016_decision_usage.sql")),
        ("017", include_str!("../migrations/017_build_runs.sql")),
        ("019", include_str!("../migrations/019_decision_reviews_unique.sql")),
        ("020", include_str!("../migrations/020_decision_usage_hash.sql")),
    ];

    for (id, sql) in late {
        if let Err(e) = conn.execute_batch(sql) {
            eprintln!("Migration {id} (non-fatal): {e}");
        }
    }

    // R-14: the decision layer hard-depends on these tables. The loop above is
    // deliberately non-fatal for ALTER-style migrations, but a silently missing
    // decision table would break every decision call — fail loudly instead.
    assert_decision_tables(conn)?;

    Ok(())
}

/// R-14: error if any decision-layer table is absent after migrations.
fn assert_decision_tables(conn: &Connection) -> Result<()> {
    for table in ["decision_usage", "decision_reviews", "decision_config"] {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })?;
    }
    Ok(())
}

fn configure_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch("PRAGMA cache_size=-32000;")?;
    conn.execute_batch("PRAGMA temp_store=MEMORY;")?;
    conn.execute_batch("PRAGMA busy_timeout=5000;")?;
    Ok(())
}

/// R-1: open an independent connection so commands that block on network I/O
/// (decision layer, compounder, KG extraction) never hold the shared
/// `Mutex<Connection>` — another command can keep using the DB meanwhile.
pub fn open_aux(db_path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    configure_pragmas(&conn)?;
    Ok(conn)
}

pub fn init_db(app: &tauri::App) -> Result<Connection> {
    let db_path = get_db_path(app);
    let conn = Connection::open(&db_path)?;
    configure_pragmas(&conn)?;
    apply_migrations(&conn)?;
    Ok(conn)
}

pub fn init_db_path(db_path: &PathBuf) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create database directory");
    }
    let conn = Connection::open(db_path)?;
    configure_pragmas(&conn)?;
    apply_migrations(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-14: a missing decision table must be a hard error, not a silent skip.
    #[test]
    fn missing_decision_table_fails_loudly() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(
            assert_decision_tables(&conn).is_err(),
            "no decision tables yet → error"
        );
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/019_decision_reviews_unique.sql"))
            .unwrap();
        assert!(
            assert_decision_tables(&conn).is_ok(),
            "tables present → ok"
        );
    }

    /// R-1: the aux connection is a real, independent connection to the same DB
    /// that sees committed data — so network-holding commands need not take the
    /// shared lock.
    #[test]
    fn open_aux_opens_an_independent_connection() {
        let dir = std::env::temp_dir().join(format!("acc-aux-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("acc.db");

        let seed = Connection::open(&path).unwrap();
        configure_pragmas(&seed).unwrap();
        apply_migrations(&seed).unwrap();
        // migration 016 seeds the default decision_config row.
        let seeded: i64 = seed
            .query_row("SELECT count(*) FROM decision_config", [], |r| r.get(0))
            .unwrap();
        assert_eq!(seeded, 1);

        let aux = open_aux(&path).unwrap();
        let n: i64 = aux
            .query_row("SELECT count(*) FROM decision_config", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "aux connection sees committed data");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

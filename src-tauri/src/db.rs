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
    ];

    for (id, sql) in late {
        if let Err(e) = conn.execute_batch(sql) {
            eprintln!("Migration {id} (non-fatal): {e}");
        }
    }

    Ok(())
}

fn configure_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch("PRAGMA cache_size=-32000;")?;
    conn.execute_batch("PRAGMA temp_store=MEMORY;")?;
    Ok(())
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

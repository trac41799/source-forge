// src-tauri/src/preferences.rs
//
// User preferences for stack selection, deploy target, and provisioning.
// Persisted in SQLite, exposed via Tauri commands to the frontend.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserPreferences {
    pub preferred_stack: String,
    pub default_deploy_target: String,
    pub auto_provision: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            preferred_stack: "nextjs-supabase-vercel".into(),
            default_deploy_target: "vercel".into(),
            auto_provision: true,
        }
    }
}

pub fn get_preferences(conn: &Connection) -> Result<UserPreferences, String> {
    let mut stmt = conn
        .prepare(
            "SELECT preferred_stack, default_deploy_target, auto_provision FROM user_preferences WHERE id = 'default'",
        )
        .map_err(|e| e.to_string())?;

    let result = stmt.query_row([], |row| {
        Ok(UserPreferences {
            preferred_stack: row.get(0)?,
            default_deploy_target: row.get(1)?,
            auto_provision: row.get::<_, i32>(2)? != 0,
        })
    });

    match result {
        Ok(prefs) => Ok(prefs),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(UserPreferences::default()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_preferences(conn: &Connection, prefs: &UserPreferences) -> Result<(), String> {
    // Validate stack ID
    if crate::stack_registry::StackPreset::get_by_id(&prefs.preferred_stack).is_none() {
        return Err(format!(
            "Unknown stack '{}'. Valid stacks: {:?}",
            prefs.preferred_stack,
            crate::stack_registry::StackPreset::all_ids()
        ));
    }

    conn.execute(
        "INSERT INTO user_preferences (id, preferred_stack, default_deploy_target, auto_provision, updated_at)
         VALUES ('default', ?1, ?2, ?3, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
           preferred_stack = excluded.preferred_stack,
           default_deploy_target = excluded.default_deploy_target,
           auto_provision = excluded.auto_provision,
           updated_at = excluded.updated_at",
        rusqlite::params![
            prefs.preferred_stack,
            prefs.default_deploy_target,
            prefs.auto_provision as i32,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user_preferences (
                id TEXT PRIMARY KEY DEFAULT 'default',
                preferred_stack TEXT NOT NULL DEFAULT 'nextjs-supabase-vercel',
                default_deploy_target TEXT NOT NULL DEFAULT 'vercel',
                auto_provision INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO user_preferences (id) VALUES ('default');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_default_preferences() {
        let conn = setup_db();
        let prefs = get_preferences(&conn).unwrap();
        assert_eq!(prefs.preferred_stack, "nextjs-supabase-vercel");
        assert!(prefs.auto_provision);
    }

    #[test]
    fn test_set_and_get_preferences() {
        let conn = setup_db();
        let new_prefs = UserPreferences {
            preferred_stack: "express-react-supabase".into(),
            default_deploy_target: "vercel".into(),
            auto_provision: false,
        };
        set_preferences(&conn, &new_prefs).unwrap();

        let loaded = get_preferences(&conn).unwrap();
        assert_eq!(loaded.preferred_stack, "express-react-supabase");
        assert!(!loaded.auto_provision);
    }

    #[test]
    fn test_invalid_stack_id_rejected() {
        let conn = setup_db();
        let bad = UserPreferences {
            preferred_stack: "invalid-stack".into(),
            default_deploy_target: "vercel".into(),
            auto_provision: true,
        };
        let result = set_preferences(&conn, &bad);
        assert!(result.is_err());
    }

    #[test]
    fn test_preferences_json_roundtrip() {
        let prefs = UserPreferences::default();
        let json = serde_json::to_string(&prefs).unwrap();
        let parsed: UserPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(prefs, parsed);
    }

    #[test]
    fn test_missing_row_returns_defaults() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user_preferences (
                id TEXT PRIMARY KEY DEFAULT 'default',
                preferred_stack TEXT NOT NULL DEFAULT 'nextjs-supabase-vercel',
                default_deploy_target TEXT NOT NULL DEFAULT 'vercel',
                auto_provision INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        // No row inserted — should return defaults
        let prefs = get_preferences(&conn).unwrap();
        assert_eq!(prefs, UserPreferences::default());
    }
}

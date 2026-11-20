// src-tauri/src/agent_adapters/mod.rs
//
// Agent CLI Compatibility Layer
// Abstracts away CLI-specific implementations for different AI coding agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod acp;
pub mod opencode;

/// Represents a running agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub agent_id: String,
    pub worktree: String,
    pub started_at: DateTime<Utc>,
}

/// Trait that all agent adapters must implement
pub trait AgentAdapter: Send + Sync {
    /// Get the adapter name (e.g., "opencode", "claude")
    fn name(&self) -> &str;
    
    /// Get the adapter version
    fn version(&self) -> &str;
    
    /// Spawn a new agent session with the given task in the worktree
    fn spawn(&self, task: &str, worktree: &str) -> Result<AgentSession, String>;
    
    /// Kill a running agent session
    fn kill(&self, session: &AgentSession) -> Result<(), String>;
    
    /// Stream output from the agent session
    fn stream_output(&self, session: &AgentSession) -> Result<Vec<String>, String>;
    
    /// Parse cost information from agent output
    fn parse_cost(&self, output: &str) -> Option<f64>;

    /// Whether the session's process is still running.
    ///
    /// Adapters that cannot tell must return `true`: the supervisor then falls
    /// back to the deadline. Adapters that *can* tell let the supervisor fail
    /// fast when an agent exits without writing a handoff, instead of idling
    /// until the timeout.
    fn is_running(&self, _session: &AgentSession) -> bool {
        true
    }

    /// Cost reported by the session so far, if the adapter can tell. The
    /// supervisor feeds this into the cost cap; `None` means "unknown" and the
    /// cap cannot trigger for that adapter.
    fn session_cost(&self, _session: &AgentSession) -> Option<f64> {
        None
    }
}

/// Mock adapter for testing
pub struct MockAgentAdapter;

impl MockAgentAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl AgentAdapter for MockAgentAdapter {
    fn name(&self) -> &str {
        "mock"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn spawn(&self, _task: &str, worktree: &str) -> Result<AgentSession, String> {
        Ok(AgentSession {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: "mock".to_string(),
            worktree: worktree.to_string(),
            started_at: Utc::now(),
        })
    }

    fn kill(&self, _session: &AgentSession) -> Result<(), String> {
        Ok(())
    }

    fn stream_output(&self, _session: &AgentSession) -> Result<Vec<String>, String> {
        Ok(vec!["[mock] task started".to_string()])
    }

    fn parse_cost(&self, output: &str) -> Option<f64> {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(c) = v.get("cost_usd").and_then(|x| x.as_f64()) {
                return Some(c);
            }
            if let Some(c) = v.get("usage").and_then(|u| u.get("cost")).and_then(|x| x.as_f64()) {
                return Some(c);
            }
        }
        None
    }
}

/// Registry for managing agent adapters
pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::with_env(HashMap::new())
    }

    pub fn with_env(env_vars: HashMap<String, String>) -> Self {
        let mut adapters: HashMap<String, Box<dyn AgentAdapter>> = HashMap::new();
        adapters.insert("mock".to_string(), Box::new(MockAgentAdapter::new()));
        adapters.insert("opencode".to_string(), Box::new(opencode::OpenCodeAdapter::new()));

        let acp_profiles = [
            ("pi", "npx", vec!["-y", "pi-acp"]),
            ("opencode-acp", "opencode", vec!["acp"]),
            ("gemini", "gemini", vec!["--acp"]),
            ("claude", "npx", vec!["-y", "@agentclientprotocol/claude-agent-acp"]),
            ("codex", "npx", vec!["-y", "@agentclientprotocol/codex-acp"]),
        ];

        for (id, binary, args) in acp_profiles {
            let profile = acp::AcpHarnessProfile {
                id: id.to_string(),
                binary: binary.to_string(),
                args: args.into_iter().map(String::from).collect(),
                env_vars: env_vars.clone(),
            };
            adapters.insert(id.to_string(), Box::new(acp::AcpAgentAdapter::new(profile)));
        }

        Self { adapters }
    }

    pub fn new_with_db(db: &rusqlite::Connection) -> Self {
        Self::with_env(load_vault_env(db))
    }

    pub fn get(&self, name: &str) -> Option<&dyn AgentAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }

    pub fn register(&mut self, name: String, adapter: Box<dyn AgentAdapter>) {
        self.adapters.insert(name, adapter);
    }
}

/// Load decrypted provider secrets from `secrets_vault` (via `crate::assets`)
/// and known provider environment variables without logging secret values.
pub fn load_vault_env(db: &rusqlite::Connection) -> HashMap<String, String> {
    let mut env_vars = HashMap::new();

    for key in [
        "OPENROUTER_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GROQ_API_KEY",
        "MISTRAL_API_KEY",
        "XAI_API_KEY",
        "DEEPSEEK_API_KEY",
    ] {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                env_vars.insert(key.to_string(), val);
            }
        }
    }

    // `list_secrets` returns entries ordered by created_at DESC, so the first
    // entry per key_name is the newest and overrides process env if non-empty.
    if let Ok(entries) = crate::assets::list_secrets(db) {
        let mut seen_keys = std::collections::HashSet::new();
        for entry in entries {
            let key_name = entry.key_name.trim().to_string();
            if key_name.is_empty()
                || key_name.contains('=')
                || key_name.contains('\0')
                || !seen_keys.insert(key_name.clone())
            {
                continue;
            }
            if let Ok(secret) = crate::assets::get_secret_value(db, &entry.id) {
                if !secret.is_empty() {
                    env_vars.insert(key_name, secret);
                }
            }
        }
    }

    env_vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_adapter_trait_exists() {
        let adapter = MockAgentAdapter::new();
        assert_eq!(adapter.name(), "mock");
    }

    #[test]
    fn test_agent_adapter_can_spawn() {
        let adapter = MockAgentAdapter::new();
        let result = adapter.spawn("test task", "/tmp/worktree");
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_adapter_can_kill() {
        let adapter = MockAgentAdapter::new();
        let session = adapter.spawn("test", "/tmp").unwrap();
        let result = adapter.kill(&session);
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_adapter_can_stream_output() {
        let adapter = MockAgentAdapter::new();
        let session = adapter.spawn("test", "/tmp").unwrap();
        let output = adapter.stream_output(&session);
        assert!(output.is_ok());
    }

    #[test]
    fn test_agent_adapter_can_parse_cost() {
        let adapter = MockAgentAdapter::new();
        let cost = adapter.parse_cost(r#"{"cost_usd": 0.05}"#);
        assert_eq!(cost, Some(0.05));
    }

    #[test]
    fn test_adapter_registry_can_get_adapter() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("mock");
        assert!(adapter.is_some());
    }

    #[test]
    fn test_adapter_registry_returns_none_for_unknown() {
        let registry = AdapterRegistry::new();
        let adapter = registry.get("unknown");
        assert!(adapter.is_none());
    }

    #[test]
    fn test_registry_has_all_acp_harnesses_and_vault_env() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../migrations/001_init.sql")).unwrap();
        conn.execute_batch(include_str!("../../migrations/002_assets.sql")).unwrap();

        crate::assets::store_secret(&conn, "OPENROUTER_API_KEY", "or-test-key", "global", None, None)
            .unwrap();
        crate::assets::store_secret(&conn, "ANTHROPIC_API_KEY", "ant-test-key", "global", None, None)
            .unwrap();

        let env = load_vault_env(&conn);
        assert_eq!(env.get("OPENROUTER_API_KEY").map(String::as_str), Some("or-test-key"));
        assert_eq!(env.get("ANTHROPIC_API_KEY").map(String::as_str), Some("ant-test-key"));

        let registry = AdapterRegistry::with_env(env);
        for expected in ["mock", "opencode", "pi", "opencode-acp", "gemini", "claude", "codex"] {
            let adapter = registry
                .get(expected)
                .unwrap_or_else(|| panic!("registry missing adapter '{expected}'"));
            assert_eq!(adapter.name(), expected);
        }
    }
}

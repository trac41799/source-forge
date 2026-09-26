// src-tauri/src/agent_adapters/mod.rs
//
// Agent CLI Compatibility Layer
// Abstracts away CLI-specific implementations for different AI coding agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        let mut adapters: HashMap<String, Box<dyn AgentAdapter>> = HashMap::new();
        adapters.insert("mock".to_string(), Box::new(MockAgentAdapter::new()));
        adapters.insert("opencode".to_string(), Box::new(opencode::OpenCodeAdapter::new()));
        Self { adapters }
    }

    pub fn get(&self, name: &str) -> Option<&dyn AgentAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }

    pub fn register(&mut self, name: String, adapter: Box<dyn AgentAdapter>) {
        self.adapters.insert(name, adapter);
    }
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
}

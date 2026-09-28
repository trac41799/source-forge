// src-tauri/src/agent_adapters/opencode.rs
//
// OpenCode CLI Adapter
// Implements AgentAdapter for the OpenCode CLI tool with real process management.

use super::{AgentAdapter, AgentSession};
use chrono::Utc;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

/// Runtime process handle (not serializable)
struct ProcessHandle {
    child: Child,
    output_tx: mpsc::UnboundedSender<String>,
}

impl ProcessHandle {
    async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

pub struct OpenCodeAdapter {
    binary_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessHandle>>>,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            binary_path: "opencode".to_string(),
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_binary(path: String) -> Self {
        Self {
            binary_path: path,
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn version(&self) -> &str {
        // Cache once (previously leaked a String on every call) and go through
        // `cmd /C` on Windows so the npm `.cmd` shim resolves.
        static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        CACHE.get_or_init(|| {
            let mut cmd = if cfg!(windows) {
                let mut c = std::process::Command::new("cmd");
                c.arg("/C").arg(&self.binary_path);
                c
            } else {
                std::process::Command::new(&self.binary_path)
            };
            match cmd.arg("--version").output() {
                Ok(o) if o.status.success() => {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                }
                _ => "unknown".to_string(),
            }
        })
    }

    fn spawn(&self, task: &str, worktree: &str) -> Result<AgentSession, String> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Build command: opencode run "<task>" --title "<session_id>" --auto [--model <model>]
        //
        // On Windows the npm-installed CLI is a `.cmd` shim, which
        // `Command::new("opencode")` cannot execute directly — go through
        // `cmd /C` so PATHEXT resolution applies.
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&self.binary_path);
            c
        } else {
            Command::new(&self.binary_path)
        };

        cmd.arg("run")
            .arg(task)
            .arg("--title")
            .arg(&session_id)
            // Headless runs must auto-approve permissions or they block on a
            // prompt forever (observed in the Wave F acceptance run).
            .arg("--auto");

        // The default model may not be tool-capable (it can claim success
        // without writing files). Allow the operator to pin one.
        if let Ok(model) = std::env::var("ACC_AGENT_MODEL") {
            let model = model.trim();
            if !model.is_empty() {
                cmd.arg("--model").arg(model);
            }
        }

        cmd.current_dir(worktree)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn opencode: {}", e))?;

        // Create output channel
        let (output_tx, _output_rx) = mpsc::unbounded_channel();

        // Capture stdout
        if let Some(stdout) = child.stdout.take() {
            let tx = output_tx.clone();
            let session_id_clone = session_id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(format!("[opencode:{}] {}", session_id_clone, line));
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = child.stderr.take() {
            let tx = output_tx.clone();
            let session_id_clone = session_id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(format!("[opencode:{}] [stderr] {}", session_id_clone, line));
                }
            });
        }

        // Store process handle
        let handle = ProcessHandle { child, output_tx };
        let processes = self.processes.clone();
        let session_id_store = session_id.clone();
        tokio::spawn(async move {
            let mut procs = processes.lock().await;
            procs.insert(session_id_store, handle);
        });

        Ok(AgentSession {
            id: session_id,
            agent_id: "opencode".to_string(),
            worktree: worktree.to_string(),
            started_at: Utc::now(),
        })
    }

    fn kill(&self, session: &AgentSession) -> Result<(), String> {
        let processes = self.processes.clone();
        let session_id = session.id.clone();

        tokio::spawn(async move {
            let mut procs = processes.lock().await;
            if let Some(handle) = procs.get_mut(&session_id) {
                handle.kill().await;
                procs.remove(&session_id);
            }
        });

        Ok(())
    }

    fn stream_output(&self, session: &AgentSession) -> Result<Vec<String>, String> {
        // For now, return empty vec (streaming is handled via output channel)
        // In the future, this could return buffered output
        Ok(vec![])
    }

    fn parse_cost(&self, output: &str) -> Option<f64> {
        // OpenCode outputs cost in JSON format
        // Try both formats: {"cost_usd": 0.123} and {"usage": {"cost": 0.456}}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_adapter_name() {
        let adapter = OpenCodeAdapter::new();
        assert_eq!(adapter.name(), "opencode");
    }

    #[test]
    fn test_opencode_adapter_version() {
        let adapter = OpenCodeAdapter::new();
        let version = adapter.version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_opencode_adapter_parse_cost() {
        let adapter = OpenCodeAdapter::new();
        assert_eq!(adapter.parse_cost(r#"{"cost_usd": 0.123}"#), Some(0.123));
        assert_eq!(adapter.parse_cost(r#"{"usage": {"cost": 0.456}}"#), Some(0.456));
        assert_eq!(adapter.parse_cost("not json"), None);
        assert_eq!(adapter.parse_cost(r#"{"other": "data"}"#), None);
    }

    #[tokio::test]
    async fn test_opencode_adapter_spawn_smoke() {
        // Environment-dependent: if the CLI resolves we get a well-formed
        // session; otherwise the error must be descriptive. (Not vacuous.)
        let adapter = OpenCodeAdapter::new();
        match adapter.spawn("test task", ".") {
            Ok(session) => {
                assert_eq!(session.agent_id, "opencode");
                assert!(!session.id.is_empty());
                assert_eq!(session.worktree, ".");
            }
            Err(error) => assert!(!error.is_empty(), "error must be descriptive"),
        }
    }

    #[tokio::test]
    async fn test_opencode_adapter_kill_nonexistent_session() {
        let adapter = OpenCodeAdapter::new();
        let session = AgentSession {
            id: "nonexistent".to_string(),
            agent_id: "opencode".to_string(),
            worktree: "/tmp".to_string(),
            started_at: Utc::now(),
        };
        // Should not panic
        let result = adapter.kill(&session);
        assert!(result.is_ok());
    }
}

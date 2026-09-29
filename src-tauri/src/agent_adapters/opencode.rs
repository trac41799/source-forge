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
    /// Sessions whose process has exited (its output streams reached EOF).
    finished: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    /// Bounded tail of each session's stdout, used for cost accounting.
    outputs: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

/// Max bytes of stdout kept per session for cost parsing.
const OUTPUT_TAIL_BYTES: usize = 32_768;

/// Constant prompt handed to the agent. The real task text is written to
/// `.acc/TASK.md` and never reaches the command line: on Windows the agent is
/// launched through `cmd /C`, where `%VAR%` is expanded even inside quotes, so
/// spec- or user-derived text in an argument is an injection vector.
const TASK_POINTER_PROMPT: &str =
    "Read .acc/TASK.md in this worktree and complete the task it describes. \
     Write the handoff file it names in the repository root when done.";

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            binary_path: "opencode".to_string(),
            processes: Arc::new(Mutex::new(HashMap::new())),
            finished: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            outputs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn with_binary(path: String) -> Self {
        Self {
            binary_path: path,
            processes: Arc::new(Mutex::new(HashMap::new())),
            finished: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            outputs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Write the task to `<worktree>/.acc/TASK.md` and return the constant
    /// pointer prompt that is safe to pass as an argument.
    fn write_task_file(worktree: &str, task: &str) -> Result<&'static str, String> {
        let dir = std::path::Path::new(worktree).join(".acc");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        let path = dir.join("TASK.md");
        std::fs::write(&path, task).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        Ok(TASK_POINTER_PROMPT)
    }

    /// Largest numeric `*cost*` value found in the output, across the whole
    /// text and line by line. OpenCode's `--format json` events carry usage per
    /// step; taking the maximum rather than a sum keeps the result an upper
    /// bound (conservative for a cost cap) without double-counting retries.
    fn parse_cost_from_output(text: &str) -> Option<f64> {
        fn collect(value: &serde_json::Value, best: &mut Option<f64>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, item) in map {
                        if key.to_ascii_lowercase().contains("cost") {
                            if let Some(cost) = item.as_f64() {
                                if best.map(|b| cost > b).unwrap_or(true) {
                                    *best = Some(cost);
                                }
                            }
                        }
                        collect(item, best);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        collect(item, best);
                    }
                }
                _ => {}
            }
        }

        let mut best = None;
        for candidate in std::iter::once(text).chain(text.lines()) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
                collect(&value, &mut best);
            }
        }
        best
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

        // The task text goes to a file; only the constant pointer reaches the
        // command line (see TASK_POINTER_PROMPT).
        let prompt = Self::write_task_file(worktree, task)?;

        cmd.arg("run")
            .arg(prompt)
            // Machine-readable events so usage/cost can be accounted for.
            .arg("--format")
            .arg("json")
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
            let finished = self.finished.clone();
            let outputs = self.outputs.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(format!("[opencode:{}] {}", session_id_clone, line));
                    if let Ok(mut map) = outputs.lock() {
                        let tail = map.entry(session_id_clone.clone()).or_default();
                        tail.push_str(&line);
                        tail.push('\n');
                        if tail.len() > OUTPUT_TAIL_BYTES {
                            let cut = tail.len() - OUTPUT_TAIL_BYTES;
                            let boundary = tail
                                .char_indices()
                                .map(|(index, _)| index)
                                .find(|index| *index >= cut)
                                .unwrap_or(0);
                            *tail = tail.split_off(boundary);
                        }
                    }
                }
                // EOF: the process exited. Lets the supervisor fail fast
                // instead of polling until the deadline.
                if let Ok(mut set) = finished.lock() {
                    set.insert(session_id_clone);
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = child.stderr.take() {
            let tx = output_tx.clone();
            let session_id_clone = session_id.clone();
            let finished = self.finished.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(format!("[opencode:{}] [stderr] {}", session_id_clone, line));
                }
                if let Ok(mut set) = finished.lock() {
                    set.insert(session_id_clone);
                }
            });
        }

        // Store process handle
        let handle = ProcessHandle { child, output_tx };
        let processes = self.processes.clone();
        let session_id_store = session_id.clone();
        tauri::async_runtime::spawn(async move {
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

        // Tauri's global runtime, not `tokio::spawn`: the supervisor kills
        // agents from the pipeline's synchronous stage loop, where no Tokio
        // runtime is in scope ("there is no reactor running" panic).
        tauri::async_runtime::spawn(async move {
            let mut procs = processes.lock().await;
            if let Some(handle) = procs.get_mut(&session_id) {
                handle.kill().await;
                procs.remove(&session_id);
            }
        });

        Ok(())
    }

    fn is_running(&self, session: &AgentSession) -> bool {
        !self
            .finished
            .lock()
            .map(|set| set.contains(&session.id))
            .unwrap_or(false)
    }

    fn session_cost(&self, session: &AgentSession) -> Option<f64> {
        let outputs = self.outputs.lock().ok()?;
        let text = outputs.get(&session.id)?;
        Self::parse_cost_from_output(text)
    }

    fn stream_output(&self, session: &AgentSession) -> Result<Vec<String>, String> {
        // For now, return empty vec (streaming is handled via output channel)
        // In the future, this could return buffered output
        Ok(vec![])
    }

    fn parse_cost(&self, output: &str) -> Option<f64> {
        Self::parse_cost_from_output(output)
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

    #[test]
    fn test_parse_cost_from_json_lines() {
        // OpenCode `--format json` emits one event per line.
        let output = "{\"type\":\"start\"}\n\
                      {\"type\":\"step_finish\",\"part\":{\"usage\":{\"cost\":0.0123}}}\n\
                      {\"type\":\"step_finish\",\"part\":{\"usage\":{\"cost\":0.045}}}\n";
        assert_eq!(OpenCodeAdapter::parse_cost_from_output(output), Some(0.045));

        // Single-object shapes from the documented formats.
        assert_eq!(
            OpenCodeAdapter::parse_cost_from_output(r#"{"cost_usd": 0.5}"#),
            Some(0.5)
        );
        assert_eq!(
            OpenCodeAdapter::parse_cost_from_output("noise\n{\"usage\": {\"cost\": 0.75}}\n"),
            Some(0.75)
        );

        // No cost information → None (the cap then cannot trigger).
        assert_eq!(OpenCodeAdapter::parse_cost_from_output("plain output"), None);
        assert_eq!(OpenCodeAdapter::parse_cost_from_output(""), None);
    }

    #[test]
    fn test_task_text_never_reaches_the_command_line() {
        // Hostile task text (cmd metacharacters and percent expansion).
        let task = "do it & calc.exe | echo %PATH% > C:\\pwned.txt";
        let dir = tempfile::TempDir::new().unwrap();
        let worktree = dir.path().to_string_lossy().to_string();

        let prompt = OpenCodeAdapter::write_task_file(&worktree, task).expect("task file");

        // The argument handed to the process is a constant pointer...
        assert_eq!(prompt, TASK_POINTER_PROMPT);
        assert!(!prompt.contains("calc.exe"));
        assert!(!prompt.contains('%'));
        // ...and the task itself lives in the file the agent reads.
        let written = std::fs::read_to_string(dir.path().join(".acc").join("TASK.md")).unwrap();
        assert_eq!(written, task);
    }

    #[tokio::test]
    async fn test_opencode_adapter_spawn_smoke() {
        // Environment-dependent: if the CLI resolves we get a well-formed
        // session; otherwise the error must be descriptive. (Not vacuous.)
        let adapter = OpenCodeAdapter::new();
        let dir = tempfile::TempDir::new().unwrap();
        let worktree = dir.path().to_string_lossy().to_string();
        match adapter.spawn("test task", &worktree) {
            Ok(session) => {
                assert_eq!(session.agent_id, "opencode");
                assert!(!session.id.is_empty());
                assert_eq!(session.worktree, worktree);
                // Cost is unknown until the CLI reports it.
                assert_eq!(adapter.session_cost(&session), None);
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

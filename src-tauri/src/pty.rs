use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, Child as PtyChild, CommandBuilder, MasterPty, PtySize};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

const DEFAULT_PTY_COLS: u16 = 120;
const DEFAULT_PTY_ROWS: u16 = 36;
const MAX_OUTPUT_BUFFER: usize = 1000;

pub type OutputCallback = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;
pub type StatusCallback = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Starting,
    Running,
    Stopped,
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "Starting"),
            AgentStatus::Running => write!(f, "Running"),
            AgentStatus::Stopped => write!(f, "Stopped"),
            AgentStatus::Error => write!(f, "Error"),
        }
    }
}

pub struct ProcessHandle {
    pub pty_child: Option<Arc<std::sync::Mutex<Box<dyn PtyChild + Send + Sync>>>>,
    pub pty_master: Option<Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>>,
    pub pty_writer: Option<Arc<std::sync::Mutex<Box<dyn Write + Send>>>>,
    pub output_rx: Option<UnboundedReceiver<String>>,
    pub output_buffer: VecDeque<String>,
    pub cols: u16,
    pub rows: u16,
    pub status: AgentStatus,
    pub started_at: DateTime<Utc>,
    pub project_path: String,
    pub session_id: String,
    pub kill_sender: Option<UnboundedSender<()>>,
    /// Additional output subscribers (used by the guard loop to read the same stream).
    pub output_subscribers: Vec<UnboundedSender<String>>,
}

pub struct ProcessRegistry {
    pub processes: Mutex<HashMap<String, ProcessHandle>>,
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentProcessInfo {
    pub session_id: String,
    pub status: String,
    pub started_at: String,
    pub project_path: String,
}

impl From<&ProcessHandle> for AgentProcessInfo {
    fn from(handle: &ProcessHandle) -> Self {
        Self {
            session_id: handle.session_id.clone(),
            status: handle.status.to_string(),
            started_at: handle.started_at.to_rfc3339(),
            project_path: handle.project_path.clone(),
        }
    }
}

pub struct PtyManager {
    registry: Arc<ProcessRegistry>,
    output_callback: Arc<RwLock<Option<OutputCallback>>>,
    status_callback: Arc<RwLock<Option<StatusCallback>>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(ProcessRegistry::new()),
            output_callback: Arc::new(RwLock::new(None)),
            status_callback: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_output_callback(&self, cb: OutputCallback) {
        if let Ok(mut guard) = self.output_callback.write() {
            *guard = Some(cb);
        }
    }

    pub fn set_status_callback(&self, cb: StatusCallback) {
        if let Ok(mut guard) = self.status_callback.write() {
            *guard = Some(cb);
        }
    }

    pub fn registry(&self) -> Arc<ProcessRegistry> {
        self.registry.clone()
    }

    pub async fn spawn_process(
        &self,
        agent_id: String,
        project_path: String,
        command: String,
        args: Vec<String>,
        env_vars: HashMap<String, String>,
    ) -> Result<String, String> {
        self.spawn_process_with_guards(agent_id, project_path, command, args, env_vars, None, None)
            .await
    }

    /// Spawn a process inside a real pseudo-terminal (`portable-pty`) with optional time and cost caps.
    pub async fn spawn_process_with_guards(
        &self,
        agent_id: String,
        project_path: String,
        command: String,
        args: Vec<String>,
        env_vars: HashMap<String, String>,
        deadline_secs: Option<u64>,
        cost_cap_usd: Option<f64>,
    ) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let (kill_tx, mut kill_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_PTY_ROWS,
                cols: DEFAULT_PTY_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to allocate PTY: {}", e))?;

        let cmd = build_pty_command(&command, &args, &project_path, &env_vars);
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn process: {}", e))?;
        // Drop slave in parent so reader sees EOF when child exits.
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

        let handle = ProcessHandle {
            pty_child: Some(Arc::new(std::sync::Mutex::new(child))),
            pty_master: Some(Arc::new(std::sync::Mutex::new(pair.master))),
            pty_writer: Some(Arc::new(std::sync::Mutex::new(writer))),
            output_rx: Some(output_rx),
            output_buffer: VecDeque::new(),
            cols: DEFAULT_PTY_COLS,
            rows: DEFAULT_PTY_ROWS,
            status: AgentStatus::Running,
            started_at: now,
            project_path,
            session_id: session_id.clone(),
            kill_sender: Some(kill_tx),
            output_subscribers: Vec::new(),
        };

        {
            let mut processes = self.registry.processes.lock().await;
            processes.insert(agent_id.clone(), handle);
        }

        emit_status_if_configured(&self.status_callback, &agent_id, &session_id, "Running");

        // If guards requested, register a guard subscriber before starting readers.
        if deadline_secs.is_some() || cost_cap_usd.is_some() {
            let (g_kill_tx, g_output_rx) = self.register_guard_subscriber(&agent_id).await?;
            let guards =
                crate::pty_guards::ProcessGuards::new(deadline_secs, cost_cap_usd, g_kill_tx);
            let arc = Arc::new(Mutex::new(guards));
            tokio::spawn(async move {
                crate::pty_guards::run_guards(arc, g_output_rx).await;
            });
        }

        let (line_tx, line_rx) = mpsc::unbounded_channel::<String>();
        spawn_pty_reader_thread(reader, line_tx);

        let registry_for_reader = self.registry.clone();
        let agent_id_for_reader = agent_id.clone();
        let session_id_for_reader = session_id.clone();
        let status_cb_for_reader = self.status_callback.clone();
        let output_cb_for_reader = self.output_callback.clone();
        tokio::spawn(async move {
            read_stdout_broadcast(
                line_rx,
                registry_for_reader,
                agent_id_for_reader,
                session_id_for_reader,
                output_tx,
                output_cb_for_reader,
                status_cb_for_reader,
                &mut kill_rx,
            )
            .await;
        });

        Ok(session_id)
    }

    pub async fn resize_pty(&self, agent_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let master_arc = {
            let mut processes = self.registry.processes.lock().await;
            let handle = processes
                .get_mut(agent_id)
                .ok_or_else(|| format!("Process {} not found", agent_id))?;
            handle.cols = cols;
            handle.rows = rows;
            handle.pty_master.clone()
        };

        if let Some(master_arc) = master_arc {
            let master = master_arc
                .lock()
                .map_err(|_| "PTY master lock poisoned".to_string())?;
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to resize PTY: {}", e))?;
        }
        Ok(())
    }

    /// Register a guard subscriber: takes the existing kill_sender, and adds a
    /// new output subscriber that will receive all output lines.
    async fn register_guard_subscriber(
        &self,
        agent_id: &str,
    ) -> Result<(UnboundedSender<()>, UnboundedReceiver<String>), String> {
        let mut processes = self.registry.processes.lock().await;
        let handle = processes
            .get_mut(agent_id)
            .ok_or_else(|| format!("Process {} not found", agent_id))?;
        let kill_tx = handle
            .kill_sender
            .take()
            .ok_or_else(|| "kill_sender already taken".to_string())?;
        let (tx, rx) = mpsc::unbounded_channel();
        handle.output_subscribers.push(tx);
        Ok((kill_tx, rx))
    }

    pub async fn kill_process(&self, agent_id: &str) -> Result<(), String> {
        let removed = {
            let mut processes = self.registry.processes.lock().await;
            processes.remove(agent_id)
        };

        if let Some(handle) = removed {
            if let Some(kill_tx) = handle.kill_sender {
                let _ = kill_tx.send(());
            }

            if let Some(child_arc) = handle.pty_child {
                if let Ok(mut child) = child_arc.lock() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }

            emit_status_if_configured(&self.status_callback, agent_id, &handle.session_id, "Stopped");
            Ok(())
        } else {
            Err(format!("Process {} not found", agent_id))
        }
    }

    pub async fn write_to_process(&self, agent_id: &str, text: &str) -> Result<(), String> {
        let writer_arc = {
            let processes = self.registry.processes.lock().await;
            let handle = processes
                .get(agent_id)
                .ok_or_else(|| format!("Process {} not found", agent_id))?;
            handle
                .pty_writer
                .clone()
                .ok_or_else(|| format!("No stdin available for process {}", agent_id))?
        };

        let mut writer = writer_arc
            .lock()
            .map_err(|_| "PTY writer lock poisoned".to_string())?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        Ok(())
    }

    pub async fn list_processes(&self) -> Vec<AgentProcessInfo> {
        let processes = self.registry.processes.lock().await;
        processes
            .iter()
            .map(|(_agent_id, handle)| AgentProcessInfo::from(handle))
            .collect()
    }

    pub async fn get_output_receiver(
        &self,
        agent_id: &str,
    ) -> Result<Option<UnboundedReceiver<String>>, String> {
        let mut processes = self.registry.processes.lock().await;
        if let Some(handle) = processes.get_mut(agent_id) {
            Ok(handle.output_rx.take())
        } else {
            Err(format!("Process {} not found", agent_id))
        }
    }

    pub async fn drain_output(&self, agent_id: &str) -> Result<Option<String>, String> {
        let mut processes = self.registry.processes.lock().await;
        let handle = processes
            .get_mut(agent_id)
            .ok_or_else(|| format!("Process {} not found", agent_id))?;
        if handle.output_buffer.is_empty() {
            return Ok(None);
        }
        let lines: Vec<String> = handle.output_buffer.drain(..).collect();
        Ok(Some(lines.join("\n")))
    }

    pub async fn snapshot_active_agents(&self) -> Vec<ActiveAgentSnapshot> {
        let processes = self.registry.processes.lock().await;
        processes
            .iter()
            .map(|(agent_id, handle)| ActiveAgentSnapshot {
                agent_id: agent_id.clone(),
                session_id: handle.session_id.clone(),
                status: handle.status.to_string(),
                project_path: handle.project_path.clone(),
                started_at: handle.started_at.to_rfc3339(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAgentSnapshot {
    pub agent_id: String,
    pub session_id: String,
    pub status: String,
    pub project_path: String,
    pub started_at: String,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

fn build_pty_command(
    command: &str,
    args: &[String],
    project_path: &str,
    env_vars: &HashMap<String, String>,
) -> CommandBuilder {
    let mut cmd = if should_wrap_windows_cmd(command) {
        let mut c = CommandBuilder::new("cmd");
        c.arg("/C");
        c.arg(command);
        c.args(args);
        c
    } else {
        let mut c = CommandBuilder::new(command);
        c.args(args);
        c
    };

    let cwd = if project_path.trim().is_empty() {
        "."
    } else {
        project_path
    };
    if Path::new(cwd).exists() {
        cmd.cwd(cwd);
    }

    if !env_vars.contains_key("TERM") {
        cmd.env("TERM", "xterm-256color");
    }
    for (k, v) in env_vars {
        cmd.env(k, v);
    }
    cmd
}

#[cfg(target_os = "windows")]
fn should_wrap_windows_cmd(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) {
        return false;
    }
    if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        return true;
    }
    if let Ok(output) = std::process::Command::new("where").arg(command).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first) = stdout.lines().next() {
                let first_lower = first.trim().to_ascii_lowercase();
                return first_lower.ends_with(".cmd") || first_lower.ends_with(".bat");
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn should_wrap_windows_cmd(_command: &str) -> bool {
    false
}

fn spawn_pty_reader_thread(mut reader: Box<dyn Read + Send>, line_tx: UnboundedSender<String>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut pending = String::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    pending.push_str(&chunk);
                    while let Some(pos) = pending.find('\n') {
                        let line = pending[..pos].trim_end_matches('\r').to_string();
                        pending.drain(..=pos);
                        if line_tx.send(line).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = line_tx.send(format!("[error] pty read error: {}", e));
                    break;
                }
            }
        }
        let remainder = pending.trim_end_matches('\r');
        if !remainder.is_empty() {
            let _ = line_tx.send(remainder.to_string());
        }
    });
}

fn emit_status_if_configured(
    status_callback: &Arc<RwLock<Option<StatusCallback>>>,
    agent_id: &str,
    session_id: &str,
    status: &str,
) {
    if let Ok(guard) = status_callback.read() {
        if let Some(ref cb) = *guard {
            cb(agent_id, session_id, status);
        }
    }
}

fn emit_output_if_configured(
    callback: &Arc<RwLock<Option<OutputCallback>>>,
    agent_id: &str,
    session_id: &str,
    line: &str,
) {
    if let Ok(guard) = callback.read() {
        if let Some(ref cb) = *guard {
            cb(agent_id, session_id, line);
        }
    }
}

/// Read PTY output lines, broadcasting each to the output channel, subscribers,
/// buffer, and configured output callback, and firing status callback on exit.
async fn read_stdout_broadcast(
    mut line_rx: UnboundedReceiver<String>,
    registry: Arc<ProcessRegistry>,
    agent_id: String,
    session_id: String,
    output_tx: UnboundedSender<String>,
    output_callback: Arc<RwLock<Option<OutputCallback>>>,
    status_callback: Arc<RwLock<Option<StatusCallback>>>,
    kill_rx: &mut UnboundedReceiver<()>,
) {
    loop {
        tokio::select! {
            _ = kill_rx.recv() => break,
            maybe_line = line_rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        let _ = output_tx.send(line.clone());
                        broadcast_line(&registry, &agent_id, &line).await;
                        emit_output_if_configured(
                            &output_callback,
                            &agent_id,
                            &session_id,
                            &line,
                        );
                    }
                    None => break,
                }
            }
        }
    }

    let mut processes = registry.processes.lock().await;
    if let Some(handle) = processes.get_mut(&agent_id) {
        if handle.session_id == session_id {
            handle.status = AgentStatus::Stopped;
            drop(processes);
            emit_status_if_configured(&status_callback, &agent_id, &session_id, "Stopped");
        }
    }
}

/// Forward a line to all output subscribers and the bounded buffer of the given agent.
async fn broadcast_line(registry: &Arc<ProcessRegistry>, agent_id: &str, line: &str) {
    let mut processes = registry.processes.lock().await;
    if let Some(handle) = processes.get_mut(agent_id) {
        if handle.output_buffer.len() >= MAX_OUTPUT_BUFFER {
            handle.output_buffer.pop_front();
        }
        handle.output_buffer.push_back(line.to_string());
        handle
            .output_subscribers
            .retain(|tx| tx.send(line.to_string()).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_real_pty_roundtrip_and_resize() {
        let manager = PtyManager::new();
        let (shell, args): (&str, Vec<String>) = if cfg!(target_os = "windows") {
            ("cmd", vec![])
        } else {
            ("sh", vec![])
        };

        let session_id = manager
            .spawn_process(
                "test-pty".to_string(),
                ".".to_string(),
                shell.to_string(),
                args,
                HashMap::new(),
            )
            .await
            .expect("spawn_process should succeed");
        assert!(!session_id.is_empty());

        // Resize active PTY master
        manager
            .resize_pty("test-pty", 160, 48)
            .await
            .expect("resize_pty should succeed");

        let mut rx = manager
            .get_output_receiver("test-pty")
            .await
            .expect("get_output_receiver")
            .expect("receiver present");

        // Write a command to PTY stdin
        let cmd_text = if cfg!(target_os = "windows") {
            "echo PTY_ROUNDTRIP_OK\r\n"
        } else {
            "echo PTY_ROUNDTRIP_OK\n"
        };
        manager
            .write_to_process("test-pty", cmd_text)
            .await
            .expect("write_to_process should succeed");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut found = false;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(250), rx.recv()).await {
                Ok(Some(line)) => {
                    if line.contains("PTY_ROUNDTRIP_OK") {
                        found = true;
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }
        assert!(found, "Expected PTY output to contain PTY_ROUNDTRIP_OK");

        // Buffered output should also be drainable without losing the receiver
        let drained = manager
            .drain_output("test-pty")
            .await
            .expect("drain_output should succeed");
        assert!(
            drained.as_deref().unwrap_or("").contains("PTY_ROUNDTRIP_OK"),
            "Expected drained buffer to contain PTY_ROUNDTRIP_OK, got: {:?}",
            drained
        );

        manager
            .kill_process("test-pty")
            .await
            .expect("kill_process should succeed");
    }

    #[tokio::test]
    async fn test_pty_output_callback_invoked() {
        let manager = PtyManager::new();
        let captured = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let captured_clone = captured.clone();
        manager.set_output_callback(Arc::new(move |agent_id, _session_id, line| {
            if let Ok(mut g) = captured_clone.lock() {
                g.push(format!("{}:{}", agent_id, line));
            }
        }));

        let statuses = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let statuses_clone = statuses.clone();
        manager.set_status_callback(Arc::new(move |agent_id, _session_id, status| {
            if let Ok(mut g) = statuses_clone.lock() {
                g.push(format!("{}:{}", agent_id, status));
            }
        }));

        let (shell, args): (&str, Vec<String>) = if cfg!(target_os = "windows") {
            ("cmd", vec![])
        } else {
            ("sh", vec![])
        };

        manager
            .spawn_process(
                "cb-agent".to_string(),
                ".".to_string(),
                shell.to_string(),
                args,
                HashMap::new(),
            )
            .await
            .expect("spawn_process should succeed");

        let cmd_text = if cfg!(target_os = "windows") {
            "echo CALLBACK_OK\r\n"
        } else {
            "echo CALLBACK_OK\n"
        };
        manager
            .write_to_process("cb-agent", cmd_text)
            .await
            .expect("write_to_process should succeed");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut seen_output = false;
        while tokio::time::Instant::now() < deadline {
            if let Ok(g) = captured.lock() {
                if g.iter().any(|l| l.contains("cb-agent:") && l.contains("CALLBACK_OK")) {
                    seen_output = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            seen_output,
            "Expected output callback to observe CALLBACK_OK, got: {:?}",
            captured.lock().ok()
        );
        manager
            .kill_process("cb-agent")
            .await
            .expect("kill_process should succeed");
        let status_list = statuses.lock().expect("statuses lock").clone();
        assert!(
            status_list.iter().any(|st| st == "cb-agent:Running"),
            "Expected Running status, got: {:?}",
            status_list
        );
        assert!(
            status_list.iter().any(|st| st == "cb-agent:Stopped"),
            "Expected Stopped status, got: {:?}",
            status_list
        );
    }
}

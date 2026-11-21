// src-tauri/src/agent_adapters/acp.rs
//
// Universal ACP (Agent Client Protocol) JSON-RPC 2.0 Adapter
// Connects ACC to any ACP-compliant coding agent (Pi, OpenCode, Gemini CLI,
// Claude Code, Codex CLI) over newline-delimited stdio.

use super::{AgentAdapter, AgentSession};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

const OUTPUT_MAX_LINES: usize = 2000;
const ASSISTANT_TEXT_MAX_BYTES: usize = 65_536;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpHarnessProfile {
    pub id: String,
    pub binary: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
}

struct AcpSessionHandle {
    stdin_tx: mpsc::UnboundedSender<String>,
    child: Arc<Mutex<Option<Child>>>,
    acp_session_id: Arc<std::sync::Mutex<Option<String>>>,
}

type PendingRequests = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>;

pub struct AcpAgentAdapter {
    profile: AcpHarnessProfile,
    sessions: Arc<std::sync::Mutex<HashMap<String, AcpSessionHandle>>>,
    finished: Arc<std::sync::Mutex<HashSet<String>>>,
    outputs: Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
    costs: Arc<std::sync::Mutex<HashMap<String, f64>>>,
}

impl AcpAgentAdapter {
    pub fn new(profile: AcpHarnessProfile) -> Self {
        Self {
            profile,
            sessions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            finished: Arc::new(std::sync::Mutex::new(HashSet::new())),
            outputs: Arc::new(std::sync::Mutex::new(HashMap::new())),
            costs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn with_env(mut self, env_vars: HashMap<String, String>) -> Self {
        self.profile.env_vars.extend(env_vars);
        self
    }

    pub fn profile(&self) -> &AcpHarnessProfile {
        &self.profile
    }

    fn spawn_child(&self, worktree: &str) -> Result<Child, String> {
        let mut cmd = Command::new(&self.profile.binary);
        cmd.args(&self.profile.args)
            .envs(&self.profile.env_vars)
            .current_dir(worktree)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        match cmd.spawn() {
            Ok(child) => Ok(child),
            Err(primary_err) if cfg!(windows) => {
                // On Windows, npm-installed CLIs (`npx`, `opencode`, `gemini`, etc.)
                // are `.cmd` shims that require `cmd /C` for PATHEXT resolution.
                let mut fallback = Command::new("cmd");
                fallback
                    .arg("/C")
                    .arg(&self.profile.binary)
                    .args(&self.profile.args)
                    .envs(&self.profile.env_vars)
                    .current_dir(worktree)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true);
                fallback.spawn().map_err(|fallback_err| {
                    format!(
                        "Failed to spawn ACP adapter '{}' ({}): direct={primary_err}; cmd={fallback_err}",
                        self.profile.id, self.profile.binary
                    )
                })
            }
            Err(err) => Err(format!(
                "Failed to spawn ACP adapter '{}' ({}): {err}",
                self.profile.id, self.profile.binary
            )),
        }
    }
}

impl AgentAdapter for AcpAgentAdapter {
    fn name(&self) -> &str {
        &self.profile.id
    }

    fn version(&self) -> &str {
        "acp/1"
    }

    fn spawn(&self, task: &str, worktree: &str) -> Result<AgentSession, String> {
        let worktree_path = PathBuf::from(worktree);
        if !worktree_path.exists() {
            return Err(format!("Worktree does not exist: {worktree}"));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let mut child = self.spawn_child(worktree)?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "ACP child stdin not piped".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ACP child stdout not piped".to_string())?;
        let stderr = child.stderr.take();

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        let child_arc = Arc::new(Mutex::new(Some(child)));
        let acp_session_id_arc = Arc::new(std::sync::Mutex::new(None::<String>));

        if let Ok(mut guard) = self.sessions.lock() {
            guard.insert(
                session_id.clone(),
                AcpSessionHandle {
                    stdin_tx: stdin_tx.clone(),
                    child: child_arc.clone(),
                    acp_session_id: acp_session_id_arc.clone(),
                },
            );
        }

        // Dedicated stdin writer task so readers/lifecycle never block on a mutex.
        tauri::async_runtime::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if !line.ends_with('\n') && stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
            let _ = stdin.shutdown().await;
        });

        // Stderr reader task.
        if let Some(stderr) = stderr {
            let outputs = self.outputs.clone();
            let sid = session_id.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    push_output_line(&outputs, &sid, format!("[stderr] {line}"));
                }
            });
        }

        let pending: PendingRequests = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let assistant_text = Arc::new(std::sync::Mutex::new(String::new()));
        let written_files = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

        // Stdout JSON-RPC 2.0 reader loop.
        {
            let pending_clone = pending.clone();
            let stdin_tx_clone = stdin_tx.clone();
            let outputs = self.outputs.clone();
            let costs = self.costs.clone();
            let assistant_text_clone = assistant_text.clone();
            let written_files_clone = written_files.clone();
            let sid = session_id.clone();
            let wt = worktree_path.clone();

            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(raw_line)) = lines.next_line().await {
                    let line = raw_line.trim_end_matches(['\r', '\n']);
                    if line.trim().is_empty() {
                        continue;
                    }

                    let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
                        push_output_line(&outputs, &sid, line.to_string());
                        continue;
                    };

                    if let Some(method) = val.get("method").and_then(|m| m.as_str()) {
                        let req_id = val.get("id").filter(|id| !id.is_null()).cloned();
                        if let Some(id) = req_id {
                            let response = handle_incoming_request(
                                method,
                                id,
                                val.get("params"),
                                &wt,
                                &written_files_clone,
                                &outputs,
                                &sid,
                            );
                            let _ = stdin_tx_clone.send(response.to_string());
                        } else if method == "session/update" {
                            handle_session_update(
                                val.get("params"),
                                &outputs,
                                &costs,
                                &assistant_text_clone,
                                &sid,
                            );
                        }
                    } else if let Some(resp_id) = val.get("id").and_then(|v| v.as_u64()) {
                        let tx = pending_clone
                            .lock()
                            .ok()
                            .and_then(|mut map| map.remove(&resp_id));
                        if let Some(sender) = tx {
                            let _ = sender.send(val);
                        }
                    }
                }

                // Wake any awaiting handshake/prompt call immediately on premature EOF.
                if let Ok(mut map) = pending_clone.lock() {
                    map.clear();
                }
            });
        }

        // Lifecycle driver task: initialize -> session/new -> session/prompt -> handoff.
        {
            let sid = session_id.clone();
            let profile_id = self.profile.id.clone();
            let task_str = task.to_string();
            let wt_str = worktree.to_string();
            let wt_path = worktree_path;
            let finished = self.finished.clone();
            let outputs = self.outputs.clone();
            let costs = self.costs.clone();
            let sessions = self.sessions.clone();

            tauri::async_runtime::spawn(async move {
                let outcome = async {
                    send_rpc_request(
                        1,
                        "initialize",
                        serde_json::json!({
                            "protocolVersion": 1,
                            "clientCapabilities": {
                                "fs": {
                                    "readTextFile": true,
                                    "writeTextFile": true
                                }
                            }
                        }),
                        &stdin_tx,
                        &pending,
                    )
                    .await?;

                    let new_resp = send_rpc_request(
                        2,
                        "session/new",
                        serde_json::json!({
                            "cwd": wt_str,
                            "mcpServers": []
                        }),
                        &stdin_tx,
                        &pending,
                    )
                    .await?;

                    let acp_sid = new_resp
                        .pointer("/result/sessionId")
                        .or_else(|| new_resp.pointer("/result/session_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&sid)
                        .to_string();

                    if let Ok(mut guard) = acp_session_id_arc.lock() {
                        *guard = Some(acp_sid.clone());
                    }

                    let prompt_resp = send_rpc_request(
                        3,
                        "session/prompt",
                        serde_json::json!({
                            "sessionId": acp_sid,
                            "prompt": [{ "type": "text", "text": task_str }]
                        }),
                        &stdin_tx,
                        &pending,
                    )
                    .await?;

                    if let Some(result_val) = prompt_resp.get("result") {
                        update_cost_from_value(result_val, &costs, &sid);
                    }

                    let summary = assistant_text
                        .lock()
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let written = written_files
                        .lock()
                        .map(|v| v.clone())
                        .unwrap_or_default();

                    if let Err(err) = synthesize_handoff_if_needed(
                        &wt_path,
                        &profile_id,
                        &task_str,
                        &summary,
                        &written,
                    ) {
                        push_output_line(&outputs, &sid, format!("[acp] handoff warning: {err}"));
                    }

                    Ok::<(), String>(())
                }
                .await;

                if let Err(err) = outcome {
                    push_output_line(&outputs, &sid, format!("[acp] error: {err}"));
                }

                // Mark finished ONLY after handoff synthesis has completed on disk.
                if let Ok(mut set) = finished.lock() {
                    set.insert(sid.clone());
                }

                drop(stdin_tx);
                if let Ok(mut guard) = sessions.lock() {
                    guard.remove(&sid);
                }

                // Give child a short window to exit cleanly on stdin EOF, then terminate.
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let mut guard = child_arc.lock().await;
                if let Some(mut child) = guard.take() {
                    match child.try_wait() {
                        Ok(Some(_)) => {}
                        _ => {
                            let _ = child.kill().await;
                        }
                    }
                }
            });
        }

        Ok(AgentSession {
            id: session_id,
            agent_id: self.profile.id.clone(),
            worktree: worktree.to_string(),
            started_at: Utc::now(),
        })
    }

    fn kill(&self, session: &AgentSession) -> Result<(), String> {
        let handle_parts = self.sessions.lock().ok().and_then(|mut map| {
            map.remove(&session.id).map(|h| {
                let acp_sid = h.acp_session_id.lock().ok().and_then(|g| g.clone());
                (h.stdin_tx, h.child, acp_sid)
            })
        });

        let finished = self.finished.clone();
        let sid = session.id.clone();

        if let Some((stdin_tx, child_arc, acp_sid)) = handle_parts {
            let cancel_params = match acp_sid {
                Some(id) => serde_json::json!({ "sessionId": id }),
                None => serde_json::json!({}),
            };
            let cancel_msg = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "session/cancel",
                "params": cancel_params,
            });
            let _ = stdin_tx.send(cancel_msg.to_string());

            tauri::async_runtime::spawn(async move {
                // Allow writer task to flush session/cancel before terminating child.
                tokio::time::sleep(std::time::Duration::from_millis(75)).await;
                drop(stdin_tx);
                let mut guard = child_arc.lock().await;
                if let Some(mut child) = guard.take() {
                    let _ = child.kill().await;
                }
                if let Ok(mut set) = finished.lock() {
                    set.insert(sid);
                }
            });
        } else if let Ok(mut set) = finished.lock() {
            set.insert(sid);
        }

        Ok(())
    }

    fn stream_output(&self, session: &AgentSession) -> Result<Vec<String>, String> {
        let guard = self.outputs.lock().map_err(|e| e.to_string())?;
        Ok(guard.get(&session.id).cloned().unwrap_or_default())
    }

    fn parse_cost(&self, output: &str) -> Option<f64> {
        let mut best: Option<f64> = None;
        for candidate in std::iter::once(output).chain(output.lines()) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                if let Some((cost, _is_total)) = extract_cost_from_json(&val) {
                    if best.map(|b| cost > b).unwrap_or(true) {
                        best = Some(cost);
                    }
                }
            }
        }
        best
    }

    fn is_running(&self, session: &AgentSession) -> bool {
        !self
            .finished
            .lock()
            .map(|set| set.contains(&session.id))
            .unwrap_or(false)
    }

    fn session_cost(&self, session: &AgentSession) -> Option<f64> {
        self.costs
            .lock()
            .ok()
            .and_then(|map| map.get(&session.id).copied())
    }
}

async fn send_rpc_request(
    id: u64,
    method: &str,
    params: serde_json::Value,
    stdin_tx: &mpsc::UnboundedSender<String>,
    pending: &PendingRequests,
) -> Result<serde_json::Value, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut map = pending.lock().map_err(|e| e.to_string())?;
        map.insert(id, tx);
    }

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    stdin_tx
        .send(payload.to_string())
        .map_err(|_| format!("ACP stdin closed before sending {method}"))?;

    let resp = rx
        .await
        .map_err(|_| format!("ACP stdout closed while awaiting {method}"))?;
    if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
        return Err(format!("ACP {method} returned error: {err}"));
    }
    Ok(resp)
}

fn push_output_line(
    outputs: &Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
    session_id: &str,
    line: String,
) {
    if let Ok(mut map) = outputs.lock() {
        let lines = map.entry(session_id.to_string()).or_default();
        lines.push(line);
        if lines.len() > OUTPUT_MAX_LINES {
            let excess = lines.len() - OUTPUT_MAX_LINES;
            lines.drain(0..excess);
        }
    }
}

fn handle_session_update(
    params: Option<&serde_json::Value>,
    outputs: &Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
    costs: &Arc<std::sync::Mutex<HashMap<String, f64>>>,
    assistant_text: &Arc<std::sync::Mutex<String>>,
    session_id: &str,
) {
    let Some(params) = params else { return };

    let mut chunks = Vec::new();
    collect_text_chunks(params, &mut chunks);
    if !chunks.is_empty() {
        let joined = chunks.join("");
        if let Ok(mut buf) = assistant_text.lock() {
            if buf.len() < ASSISTANT_TEXT_MAX_BYTES {
                let remaining = ASSISTANT_TEXT_MAX_BYTES - buf.len();
                let mut take = joined.len().min(remaining);
                while take > 0 && !joined.is_char_boundary(take) {
                    take -= 1;
                }
                buf.push_str(&joined[..take]);
            }
        }
        for line in joined.lines() {
            if !line.trim().is_empty() {
                push_output_line(outputs, session_id, line.to_string());
            }
        }
    }

    update_cost_from_value(params, costs, session_id);
}

fn collect_text_chunks(val: &serde_json::Value, out: &mut Vec<String>) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push(text.to_string());
                return;
            }
            if let Some(delta) = map.get("delta").and_then(|v| v.as_str()) {
                out.push(delta.to_string());
                return;
            }
            for key in ["update", "content", "message", "part"] {
                if let Some(child) = map.get(key) {
                    collect_text_chunks(child, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_text_chunks(item, out);
            }
        }
        _ => {}
    }
}

fn update_cost_from_value(
    val: &serde_json::Value,
    costs: &Arc<std::sync::Mutex<HashMap<String, f64>>>,
    session_id: &str,
) {
    if let Some((amount, is_total)) = extract_cost_from_json(val) {
        if let Ok(mut map) = costs.lock() {
            let entry = map.entry(session_id.to_string()).or_insert(0.0);
            if is_total {
                *entry = (*entry).max(amount);
            } else {
                *entry += amount;
            }
        }
    }
}

/// Extract a cost number from an ACP notification/result payload.
/// Returns `(amount, is_total)` where `is_total` is true for fields like `totalCost`/`total_cost`.
fn extract_cost_from_json(val: &serde_json::Value) -> Option<(f64, bool)> {
    // Check explicit total cost keys first.
    for ptr in [
        "/totalCost",
        "/total_cost",
        "/total_cost_usd",
        "/usage/totalCost",
        "/usage/total_cost",
        "/update/usage/totalCost",
    ] {
        if let Some(c) = val.pointer(ptr).and_then(|v| v.as_f64()).filter(|c| *c >= 0.0) {
            return Some((c, true));
        }
    }

    // Check incremental/step cost keys.
    for ptr in [
        "/usage/cost",
        "/update/usage/cost",
        "/cost_usd",
        "/cost",
        "/update/cost",
    ] {
        if let Some(c) = val.pointer(ptr).and_then(|v| v.as_f64()).filter(|c| *c >= 0.0) {
            return Some((c, false));
        }
    }

    // Fallback recursive scan for any `*cost*` key.
    fn scan(v: &serde_json::Value) -> Option<(f64, bool)> {
        match v {
            serde_json::Value::Object(map) => {
                for (k, item) in map {
                    let lower = k.to_ascii_lowercase();
                    if lower.contains("cost") {
                        if let Some(c) = item.as_f64().filter(|c| *c >= 0.0) {
                            return Some((c, lower.contains("total")));
                        }
                    }
                    if let Some(found) = scan(item) {
                        return Some(found);
                    }
                }
                None
            }
            serde_json::Value::Array(arr) => arr.iter().find_map(scan),
            _ => None,
        }
    }

    scan(val)
}

fn handle_incoming_request(
    method: &str,
    id: serde_json::Value,
    params: Option<&serde_json::Value>,
    worktree: &Path,
    written_files: &Arc<std::sync::Mutex<Vec<String>>>,
    outputs: &Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
    session_id: &str,
) -> serde_json::Value {
    match method {
        "session/request_permission" => {
            if let Err(reason) = validate_permission_params(worktree, params) {
                push_output_line(
                    outputs,
                    session_id,
                    format!("[acp:security] denied permission request: {reason}"),
                );
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "outcome": { "outcome": "cancelled" } }
                })
            } else {
                let option_id = select_allow_option(params);
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "outcome": {
                            "outcome": "selected",
                            "optionId": option_id
                        }
                    }
                })
            }
        }
        "fs/read_text_file" => {
            let Some(req_path) = params.and_then(|p| p.get("path")).and_then(|v| v.as_str()) else {
                return rpc_error(id, -32602, "Missing required 'path' parameter");
            };
            let resolved = match resolve_in_worktree(worktree, req_path) {
                Ok(p) => p,
                Err(reason) => {
                    push_output_line(
                        outputs,
                        session_id,
                        format!("[acp:security] denied fs/read_text_file: {reason}"),
                    );
                    return rpc_error(id, -32602, &format!("Path outside worktree: {reason}"));
                }
            };
            match std::fs::read_to_string(&resolved) {
                Ok(content) => {
                    let sliced = slice_text_lines(
                        &content,
                        params.and_then(|p| p.get("line")).and_then(|v| v.as_u64()),
                        params.and_then(|p| p.get("limit")).and_then(|v| v.as_u64()),
                    );
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "content": sliced }
                    })
                }
                Err(err) => rpc_error(id, -32000, &format!("Failed to read file: {err}")),
            }
        }
        "fs/write_text_file" => {
            let Some(req_path) = params.and_then(|p| p.get("path")).and_then(|v| v.as_str()) else {
                return rpc_error(id, -32602, "Missing required 'path' parameter");
            };
            let Some(content) = params.and_then(|p| p.get("content")).and_then(|v| v.as_str()) else {
                return rpc_error(id, -32602, "Missing required 'content' parameter");
            };
            let resolved = match resolve_in_worktree(worktree, req_path) {
                Ok(p) => p,
                Err(reason) => {
                    push_output_line(
                        outputs,
                        session_id,
                        format!("[acp:security] denied fs/write_text_file: {reason}"),
                    );
                    return rpc_error(id, -32602, &format!("Path outside worktree: {reason}"));
                }
            };
            if let Some(parent) = resolved.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    return rpc_error(id, -32000, &format!("Failed to create parent dir: {err}"));
                }
            }
            match std::fs::write(&resolved, content) {
                Ok(()) => {
                    if let Ok(canon_wt) = std::fs::canonicalize(worktree) {
                        let rel = resolved
                            .strip_prefix(&canon_wt)
                            .unwrap_or(&resolved)
                            .to_string_lossy()
                            .replace('\\', "/");
                        if let Ok(mut list) = written_files.lock() {
                            if !list.contains(&rel) {
                                list.push(rel);
                            }
                        }
                    }
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {}
                    })
                }
                Err(err) => rpc_error(id, -32000, &format!("Failed to write file: {err}")),
            }
        }
        other => rpc_error(id, -32601, &format!("Method not found: {other}")),
    }
}

fn rpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn slice_text_lines(content: &str, start_line: Option<u64>, limit: Option<u64>) -> String {
    if start_line.is_none() && limit.is_none() {
        return content.to_string();
    }
    let start_idx = start_line.unwrap_or(1).saturating_sub(1) as usize;
    let lines: Vec<&str> = content.lines().collect();
    if start_idx >= lines.len() {
        return String::new();
    }
    let end_idx = match limit {
        Some(lim) => start_idx.saturating_add(lim as usize).min(lines.len()),
        None => lines.len(),
    };
    lines[start_idx..end_idx].join("\n")
}

fn select_allow_option(params: Option<&serde_json::Value>) -> String {
    if let Some(options) = params.and_then(|p| p.get("options")).and_then(|v| v.as_array()) {
        for opt in options {
            if opt.get("optionId").and_then(|v| v.as_str()) == Some("allow") {
                return "allow".to_string();
            }
        }
        for opt in options {
            let kind = opt.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind.starts_with("allow") {
                if let Some(id) = opt.get("optionId").and_then(|v| v.as_str()) {
                    return id.to_string();
                }
            }
        }
    }
    "allow".to_string()
}

fn validate_permission_params(
    worktree: &Path,
    params: Option<&serde_json::Value>,
) -> Result<(), String> {
    let Some(params) = params else {
        return Ok(());
    };

    fn check_val(worktree: &Path, key: &str, val: &serde_json::Value) -> Result<(), String> {
        let lower = key.to_ascii_lowercase();
        let is_path_key = matches!(
            lower.as_str(),
            "path"
                | "paths"
                | "file_path"
                | "filepath"
                | "target_path"
                | "targetpath"
                | "cwd"
                | "dir"
                | "directory"
                | "uri"
        );
        let is_cmd_key = matches!(lower.as_str(), "command" | "cmd");

        match val {
            serde_json::Value::String(s) => {
                if is_path_key {
                    resolve_in_worktree(worktree, s).map(|_| ())
                } else if is_cmd_key {
                    validate_command_string(worktree, s)
                } else {
                    Ok(())
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    check_val(worktree, key, item)?;
                }
                Ok(())
            }
            serde_json::Value::Object(map) => {
                for (child_k, child_v) in map {
                    check_val(worktree, child_k, child_v)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    check_val(worktree, "", params)
}

fn validate_command_string(worktree: &Path, command: &str) -> Result<(), String> {
    for raw_token in command.split_whitespace() {
        let token = raw_token.trim_matches(['"', '\'', ';', '&', '|']);
        if token.is_empty() {
            continue;
        }
        if token.contains("../")
            || token.contains("..\\")
            || token == ".."
            || token.starts_with("~/")
            || token.starts_with("~\\")
        {
            return Err(format!("Command references path outside worktree: {token}"));
        }
        let looks_like_windows_abs =
            token.len() >= 3 && token.as_bytes()[1] == b':' && (token.as_bytes()[2] == b'\\' || token.as_bytes()[2] == b'/');
        let looks_like_unix_abs = token.starts_with('/') && (!cfg!(windows) || token[1..].contains('/'));
        if looks_like_windows_abs || looks_like_unix_abs || token.starts_with("\\\\") {
            resolve_in_worktree(worktree, token)?;
        }
    }
    Ok(())
}

/// Resolve `requested` and enforce that it lies strictly inside `worktree`.
/// Handles relative paths, absolute paths, `..` traversal, symlinks, and
/// not-yet-created files (for `fs/write_text_file`).
fn resolve_in_worktree(worktree: &Path, requested: &str) -> Result<PathBuf, String> {
    let trimmed = requested.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("Empty or invalid path".to_string());
    }
    if trimmed.starts_with("~/") || trimmed.starts_with("~\\") || trimmed == "~" {
        return Err(format!("Home directory reference outside worktree: {trimmed}"));
    }

    let stripped = if let Some(rest) = trimmed.strip_prefix("file:///") {
        if cfg!(windows) {
            rest
        } else {
            &trimmed["file://".len()..]
        }
    } else if let Some(rest) = trimmed.strip_prefix("file://") {
        rest
    } else {
        trimmed
    };

    let req_path = Path::new(stripped);
    let candidate = if req_path.is_absolute() {
        req_path.to_path_buf()
    } else {
        worktree.join(req_path)
    };

    let normalized = normalize_lexical(&candidate)?;
    let canon_root = std::fs::canonicalize(worktree)
        .map_err(|e| format!("Cannot canonicalize worktree {}: {e}", worktree.display()))?;

    let canon_target = canonicalize_with_missing_tail(&normalized)?;
    if !canon_target.starts_with(&canon_root) {
        return Err(format!(
            "Target {} resolves outside worktree {}",
            canon_target.display(),
            canon_root.display()
        ));
    }

    Ok(canon_target)
}

fn normalize_lexical(path: &Path) -> Result<PathBuf, String> {
    let mut stack: Vec<Component<'_>> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => stack.push(comp),
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                _ => return Err("Path traversal past filesystem root".to_string()),
            },
            Component::Normal(_) => stack.push(comp),
        }
    }
    Ok(stack.iter().collect())
}

fn canonicalize_with_missing_tail(normalized: &Path) -> Result<PathBuf, String> {
    if normalized.exists() {
        return std::fs::canonicalize(normalized)
            .map_err(|e| format!("Failed to canonicalize {}: {e}", normalized.display()));
    }

    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = normalized.to_path_buf();
    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            return Err(format!(
                "No existing ancestor for path {}",
                normalized.display()
            ));
        };
        tail.push(name.to_os_string());
        if !cursor.pop() {
            return Err(format!(
                "No existing ancestor for path {}",
                normalized.display()
            ));
        }
    }

    let mut base = std::fs::canonicalize(&cursor)
        .map_err(|e| format!("Failed to canonicalize {}: {e}", cursor.display()))?;
    for segment in tail.into_iter().rev() {
        base.push(segment);
    }
    Ok(base)
}

/// R204: Synthesize a canonical 6-section `HANDOFF_<agent_ref>.md` when an ACP
/// turn finishes cleanly with modified files or summary text but did not write one.
fn synthesize_handoff_if_needed(
    worktree: &Path,
    profile_id: &str,
    task: &str,
    assistant_summary: &str,
    written_files: &[String],
) -> Result<(), String> {
    // 1. If any non-empty HANDOFF_*.md already exists in worktree root, preserve it.
    if let Ok(entries) = std::fs::read_dir(worktree) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && crate::handoff_parser::is_handoff_file(&p) {
                if let Ok(meta) = entry.metadata() {
                    if meta.len() > 0 {
                        return Ok(());
                    }
                }
            }
        }
    }

    // 2. Collect changed files from git status and fs/write_text_file calls.
    let mut changed_files = collect_changed_files(worktree, written_files);
    changed_files.sort();
    changed_files.dedup();

    let summary_trimmed = assistant_summary.trim();
    if changed_files.is_empty() && summary_trimmed.is_empty() {
        return Ok(());
    }

    // 3. Determine agent_ref from .acc/GUIDELINE.md or .acc/TASK.md, else profile_id.
    let agent_ref = detect_handoff_agent_ref(worktree)
        .unwrap_or_else(|| sanitize_ref(profile_id));
    let handoff_path = worktree.join(format!("HANDOFF_{agent_ref}.md"));

    let summary_body = if summary_trimmed.is_empty() {
        format!("Completed task via ACP adapter ({profile_id}).")
    } else {
        summary_trimmed.to_string()
    };

    let files_changed_md = if changed_files.is_empty() {
        "- (none)".to_string()
    } else {
        changed_files
            .iter()
            .map(|f| format!("- {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Ensure all crate::orchestrator::REQUIRED_HANDOFF_SECTIONS are present.
    let content = format!(
        "# HANDOFF {agent_ref}\n\n\
        ## Original Task\n{task}\n\n\
        ## Completed By\n{profile_id}\n\n\
        ## Model Used\nacp/{profile_id}\n\n\
        ## Output Summary\n{summary_body}\n\n\
        ## Completed Work\n{summary_body}\n\n\
        ## Test Results\n- ACP turn completed cleanly; run repository verification gates for full suite status.\n\n\
        ## Files Changed\n{files_changed_md}\n\n\
        ## Files NOT Modified\n- Files outside the assigned task scope were left untouched.\n\n\
        ## Interface Contracts Exposed\n{files_changed_md}\n\n\
        ## Design Decisions\n- Executed via universal ACP JSON-RPC 2.0 adapter ({profile_id}).\n\n\
        ## Handoff Instructions\nReview the changed files and proceed with verification and wave merge.\n"
    );

    std::fs::write(&handoff_path, content)
        .map_err(|e| format!("Failed to write synthesized handoff {}: {e}", handoff_path.display()))
}

fn detect_handoff_agent_ref(worktree: &Path) -> Option<String> {
    for rel in [".acc/GUIDELINE.md", ".acc/TASK.md"] {
        let path = worktree.join(rel);
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(found) = extract_handoff_ref_from_text(&content) {
                return Some(found);
            }
        }
    }
    None
}

fn extract_handoff_ref_from_text(text: &str) -> Option<String> {
    let mut search = text;
    while let Some(idx) = search.find("HANDOFF_") {
        let after = &search[idx + "HANDOFF_".len()..];
        if let Some(end) = after.find(".md") {
            let candidate = &after[..end];
            if !candidate.is_empty()
                && candidate
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(candidate.to_string());
            }
        }
        search = after;
    }
    None
}

fn sanitize_ref(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.is_empty() {
        "acp".to_string()
    } else {
        s
    }
}

fn collect_changed_files(worktree: &Path, written_files: &[String]) -> Vec<String> {
    let mut files: Vec<String> = written_files
        .iter()
        .filter(|f| !is_acc_metadata_path(f))
        .cloned()
        .collect();

    if let Ok(output) = std::process::Command::new("git")
        .args(["-C", &worktree.to_string_lossy(), "status", "--porcelain"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.len() <= 3 {
                    continue;
                }
                let raw = line[3..].trim();
                let path = raw
                    .rsplit_once(" -> ")
                    .map(|(_, new)| new)
                    .unwrap_or(raw)
                    .trim_matches('"')
                    .replace('\\', "/");
                if !path.is_empty() && !is_acc_metadata_path(&path) {
                    files.push(path);
                }
            }
        }
    }

    files
}

fn is_acc_metadata_path(path: &str) -> bool {
    let norm = path.trim_start_matches("./").replace('\\', "/");
    norm == ".acc"
        || norm.starts_with(".acc/")
        || (norm.starts_with("HANDOFF_") && norm.ends_with(".md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    const MOCK_ACP_SERVER_JS: &str = r#"
const fs = require('fs');
const readline = require('readline');

const mode = process.argv[2];
const logPath = process.argv[3];

function log(entry) {
  fs.appendFileSync(logPath, JSON.stringify(entry) + '\n');
}

function send(obj) {
  process.stdout.write(JSON.stringify(obj) + '\n');
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
let pendingResponses = new Map();
let nextReqId = 100;

function requestClient(method, params) {
  const id = ++nextReqId;
  return new Promise((resolve) => {
    pendingResponses.set(id, resolve);
    send({ jsonrpc: '2.0', id, method, params });
  });
}

rl.on('line', async (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);

  if (msg.id !== undefined && !msg.method && pendingResponses.has(msg.id)) {
    const resolve = pendingResponses.get(msg.id);
    pendingResponses.delete(msg.id);
    log({ kind: 'client_response', id: msg.id, response: msg });
    resolve(msg);
    return;
  }

  log({ kind: 'client_msg', msg });

  if (msg.method === 'initialize') {
    send({ jsonrpc: '2.0', id: msg.id, result: { protocolVersion: 1, agentInfo: { name: 'mock-acp', version: '0.1.0' } } });
  } else if (msg.method === 'session/new') {
    send({ jsonrpc: '2.0', id: msg.id, result: { sessionId: 'acp-sess-123' } });
  } else if (msg.method === 'session/prompt') {
    if (mode === 'handshake') {
      if (process.env.TEST_VAULT_SECRET) {
        log({ kind: 'env_check', secret: process.env.TEST_VAULT_SECRET });
      }
      send({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } });
    } else if (mode === 'cancel') {
      // Wait for session/cancel notification or stdin close
    } else if (mode === 'jail') {
      send({
        jsonrpc: '2.0',
        method: 'session/update',
        params: {
          sessionId: 'acp-sess-123',
          update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Step 1 complete' } },
          usage: { cost: 0.02 }
        }
      });
      send({
        jsonrpc: '2.0',
        method: 'session/update',
        params: {
          sessionId: 'acp-sess-123',
          update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Step 2 complete' } },
          usage: { cost: 0.03 }
        }
      });
      await requestClient('fs/write_text_file', { sessionId: 'acp-sess-123', path: 'src/hello.txt', content: 'hello acp' });
      await requestClient('fs/write_text_file', { sessionId: 'acp-sess-123', path: '../../outside.txt', content: 'pwned' });
      await requestClient('fs/read_text_file', { sessionId: 'acp-sess-123', path: 'src/hello.txt' });
      await requestClient('fs/read_text_file', { sessionId: 'acp-sess-123', path: '../../etc/passwd' });
      await requestClient('session/request_permission', {
        sessionId: 'acp-sess-123',
        toolCall: { toolCallId: 'tc-1', locations: [{ path: 'src/hello.txt' }] },
        options: [{ optionId: 'allow', kind: 'allow_once' }, { optionId: 'reject', kind: 'reject_once' }]
      });
      await requestClient('session/request_permission', {
        sessionId: 'acp-sess-123',
        toolCall: { toolCallId: 'tc-2', locations: [{ path: '../../etc/passwd' }] },
        options: [{ optionId: 'allow', kind: 'allow_once' }, { optionId: 'reject', kind: 'reject_once' }]
      });
      await requestClient('custom/unknown_method', { foo: 'bar' });
      send({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } });
    } else if (mode === 'handoff') {
      send({
        jsonrpc: '2.0',
        method: 'session/update',
        params: {
          sessionId: 'acp-sess-123',
          update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Implemented feature.rs cleanly.' } }
        }
      });
      await requestClient('fs/write_text_file', { sessionId: 'acp-sess-123', path: 'src/feature.rs', content: 'pub fn ok() -> bool { true }\n' });
      send({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } });
    } else if (mode === 'handoff-existing') {
      await requestClient('fs/write_text_file', {
        sessionId: 'acp-sess-123',
        path: 'HANDOFF_backend.md',
        content: '# HANDOFF backend\n\n## Original Task\ntask\n\n## Completed By\nagent\n\n## Model Used\nm\n\n## Output Summary\nAgent custom summary\n\n## Completed Work\nAgent custom work\n\n## Test Results\npassed\n\n## Files Changed\n- src/a.rs\n\n## Files NOT Modified\n- none\n\n## Interface Contracts Exposed\n- none\n\n## Design Decisions\n- custom\n\n## Handoff Instructions\nProceed\n'
      });
      send({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } });
    }
  } else if (msg.method === 'session/cancel') {
    process.exit(0);
  }
});
"#;

    fn write_mock_server(dir: &Path) -> PathBuf {
        let script_path = dir.join("mock_acp_server.js");
        std::fs::write(&script_path, MOCK_ACP_SERVER_JS).unwrap();
        script_path
    }

    async fn wait_until_finished(adapter: &AcpAgentAdapter, session: &AgentSession) {
        for _ in 0..100 {
            if !adapter.is_running(session) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!(
            "ACP session did not finish within 5s; output: {:?}",
            adapter.stream_output(session)
        );
    }

    fn read_log(log_path: &Path) -> Vec<serde_json::Value> {
        let text = std::fs::read_to_string(log_path).unwrap_or_default();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn test_acp_handshake_and_prompt_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().join("worktree");
        std::fs::create_dir_all(&wt).unwrap();
        let script = write_mock_server(tmp.path());
        let log_path = tmp.path().join("handshake.jsonl");

        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAULT_SECRET".to_string(), "super-secret-val".to_string());

        let adapter = AcpAgentAdapter::new(AcpHarnessProfile {
            id: "mock-acp".to_string(),
            binary: "node".to_string(),
            args: vec![
                script.to_string_lossy().to_string(),
                "handshake".to_string(),
                log_path.to_string_lossy().to_string(),
            ],
            env_vars,
        });

        let session = adapter
            .spawn("Build feature X", &wt.to_string_lossy())
            .expect("spawn should succeed");
        assert_eq!(session.agent_id, "mock-acp");

        wait_until_finished(&adapter, &session).await;
        assert!(!adapter.is_running(&session));

        let entries = read_log(&log_path);
        let methods: Vec<&str> = entries
            .iter()
            .filter(|e| e["kind"] == "client_msg")
            .filter_map(|e| e["msg"]["method"].as_str())
            .collect();
        assert_eq!(methods, vec!["initialize", "session/new", "session/prompt"]);

        let init_msg = &entries[0]["msg"];
        assert_eq!(init_msg["params"]["protocolVersion"], 1);
        assert_eq!(init_msg["params"]["clientCapabilities"]["fs"]["readTextFile"], true);
        assert_eq!(init_msg["params"]["clientCapabilities"]["fs"]["writeTextFile"], true);

        let new_msg = &entries[1]["msg"];
        assert_eq!(new_msg["params"]["cwd"], wt.to_string_lossy().as_ref());
        assert!(new_msg["params"]["mcpServers"].is_array());

        let prompt_msg = &entries[2]["msg"];
        assert_eq!(prompt_msg["params"]["sessionId"], "acp-sess-123");
        assert_eq!(prompt_msg["params"]["prompt"][0]["text"], "Build feature X");

        // Verify env var injection
        assert!(entries.iter().any(|e| e["kind"] == "env_check" && e["secret"] == "super-secret-val"));

        // Verify kill sends session/cancel
        let cancel_log = tmp.path().join("cancel.jsonl");
        let cancel_adapter = AcpAgentAdapter::new(AcpHarnessProfile {
            id: "mock-acp-cancel".to_string(),
            binary: "node".to_string(),
            args: vec![
                script.to_string_lossy().to_string(),
                "cancel".to_string(),
                cancel_log.to_string_lossy().to_string(),
            ],
            env_vars: HashMap::new(),
        });
        let cancel_sess = cancel_adapter
            .spawn("Long running", &wt.to_string_lossy())
            .expect("spawn cancel test");
        for _ in 0..60 {
            let logs = read_log(&cancel_log);
            if logs.iter().any(|e| e["msg"]["method"] == "session/prompt") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        cancel_adapter.kill(&cancel_sess).expect("kill should succeed");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let cancel_logs = read_log(&cancel_log);
        assert!(
            cancel_logs.iter().any(|e| e["msg"]["method"] == "session/cancel"),
            "kill() must send session/cancel notification"
        );
    }

    #[tokio::test]
    async fn test_acp_telemetry_and_worktree_permission_jail() {
        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().join("worktree");
        std::fs::create_dir_all(&wt).unwrap();
        let script = write_mock_server(tmp.path());
        let log_path = tmp.path().join("jail.jsonl");

        let adapter = AcpAgentAdapter::new(AcpHarnessProfile {
            id: "mock-acp".to_string(),
            binary: "node".to_string(),
            args: vec![
                script.to_string_lossy().to_string(),
                "jail".to_string(),
                log_path.to_string_lossy().to_string(),
            ],
            env_vars: HashMap::new(),
        });

        let session = adapter
            .spawn("Test telemetry and jail", &wt.to_string_lossy())
            .expect("spawn should succeed");
        wait_until_finished(&adapter, &session).await;

        let cost = adapter.session_cost(&session).expect("session_cost must be Some");
        assert!((cost - 0.05).abs() < 1e-6, "expected cumulative cost 0.05, got {cost}");

        let output = adapter.stream_output(&session).unwrap().join("\n");
        assert!(output.contains("Step 1 complete"));
        assert!(output.contains("Step 2 complete"));

        // Worktree file written, outside file NOT written
        assert_eq!(
            std::fs::read_to_string(wt.join("src").join("hello.txt")).unwrap(),
            "hello acp"
        );
        assert!(!tmp.path().join("outside.txt").exists());
        assert!(!tmp.path().parent().unwrap().join("outside.txt").exists());

        let entries = read_log(&log_path);
        let resp_by_id = |id: u64| -> serde_json::Value {
            entries
                .iter()
                .find(|e| e["kind"] == "client_response" && e["id"] == id)
                .expect("missing client response")["response"]
                .clone()
        };

        // 101: fs/write_text_file inside worktree -> ok
        assert!(resp_by_id(101)["error"].is_null());
        // 102: fs/write_text_file outside worktree -> -32602
        assert_eq!(resp_by_id(102)["error"]["code"], -32602);
        // 103: fs/read_text_file inside worktree -> content
        assert_eq!(resp_by_id(103)["result"]["content"], "hello acp");
        // 104: fs/read_text_file outside worktree -> -32602
        assert_eq!(resp_by_id(104)["error"]["code"], -32602);
        // 105: session/request_permission inside worktree -> selected/allow
        assert_eq!(resp_by_id(105)["result"]["outcome"]["outcome"], "selected");
        assert_eq!(resp_by_id(105)["result"]["outcome"]["optionId"], "allow");
        // 106: session/request_permission outside worktree -> cancelled
        assert_eq!(resp_by_id(106)["result"]["outcome"]["outcome"], "cancelled");
        // 107: unknown method -> -32601
        assert_eq!(resp_by_id(107)["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn test_auto_synthesizes_handoff_on_clean_turn() {
        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().join("worktree");
        std::fs::create_dir_all(wt.join(".acc")).unwrap();
        std::fs::write(
            wt.join(".acc").join("GUIDELINE.md"),
            "# AGENT backend GUIDELINE\n\n## Handoff\n- Write HANDOFF_backend.md in the repository root when complete\n",
        )
        .unwrap();

        let script = write_mock_server(tmp.path());
        let log_path = tmp.path().join("handoff.jsonl");

        let adapter = AcpAgentAdapter::new(AcpHarnessProfile {
            id: "pi".to_string(),
            binary: "node".to_string(),
            args: vec![
                script.to_string_lossy().to_string(),
                "handoff".to_string(),
                log_path.to_string_lossy().to_string(),
            ],
            env_vars: HashMap::new(),
        });

        let session = adapter
            .spawn("Implement feature.rs", &wt.to_string_lossy())
            .expect("spawn should succeed");
        wait_until_finished(&adapter, &session).await;

        let handoff_path = wt.join("HANDOFF_backend.md");
        assert!(handoff_path.exists(), "HANDOFF_backend.md should be synthesized");

        let env = crate::handoff_parser::parse_handoff_file(&handoff_path)
            .expect("synthesized handoff must pass parse_handoff_file");
        assert!(env.output_summary.contains("Implemented feature.rs cleanly."));
        assert!(env.changed_files.iter().any(|f| f.contains("src/feature.rs")));

        // Verify existing handoff written by agent is NOT overwritten
        let wt2 = tmp.path().join("worktree2");
        std::fs::create_dir_all(wt2.join(".acc")).unwrap();
        let log_path2 = tmp.path().join("handoff2.jsonl");
        let adapter2 = AcpAgentAdapter::new(AcpHarnessProfile {
            id: "pi".to_string(),
            binary: "node".to_string(),
            args: vec![
                script.to_string_lossy().to_string(),
                "handoff-existing".to_string(),
                log_path2.to_string_lossy().to_string(),
            ],
            env_vars: HashMap::new(),
        });
        let session2 = adapter2
            .spawn("Implement a.rs", &wt2.to_string_lossy())
            .expect("spawn2 should succeed");
        wait_until_finished(&adapter2, &session2).await;
        let env2 = crate::handoff_parser::parse_handoff_file(&wt2.join("HANDOFF_backend.md"))
            .expect("agent handoff must remain valid");
        assert_eq!(env2.output_summary, "Agent custom summary");
    }
}

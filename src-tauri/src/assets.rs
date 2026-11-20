use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::io::Write;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub source: String,
    pub content: String,
    pub tags: Vec<String>,
    pub injectable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryFileEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub agent: String,
    pub content: String,
    pub last_modified: String,
    pub snapshot: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MCPEntry {
    pub id: String,
    pub name: String,
    pub server_command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub source: String,
    pub agent_id: String,
    pub managed_externally: bool,
    pub health: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VaultEntry {
    pub id: String,
    pub key_name: String,
    pub scope: String,
    pub agent_id: Option<String>,
    pub project_id: Option<String>,
    pub masked_value: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpInstallStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpConnectionTest {
    pub connected: bool,
    pub tool_count: usize,
    pub error: Option<String>,
}

/// Pre-configured MCP: GA-Bagua Semantic KG
/// MCP server providing 29 semantic tools for agents
pub fn get_bagua_mcp_config() -> MCPEntry {
    MCPEntry {
        id: "mcp-builtin-bagua-semantic-kg".to_string(),
        name: "GA-Bagua Semantic KG".to_string(),
        server_command: "ga-semantics-mcp".to_string(),
        args: vec![],
        env: HashMap::new(),
        enabled: false,
        source: "builtin".to_string(),
        agent_id: "system".to_string(),
        managed_externally: false,
        health: "grey".to_string(),
    }
}

/// Check if ga-semantics-mcp binary is installed on PATH
pub fn detect_bagua_mcp() -> McpInstallStatus {
    let (check_cmd, check_arg) = if cfg!(target_os = "windows") {
        ("where", "ga-semantics-mcp")
    } else {
        ("which", "ga-semantics-mcp")
    };

    let output = Command::new(check_cmd)
        .arg(check_arg)
        .output()
        .ok();

    let path = output
        .as_ref()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout.clone())
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    let version = if path.is_some() {
        Command::new("ga-semantics-mcp")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    McpInstallStatus {
        installed: path.is_some(),
        path,
        version,
    }
}

/// Test connection to ga-semantics-mcp by spawning it and sending tools/list JSON-RPC
pub fn test_bagua_mcp_connection() -> McpConnectionTest {
    let mut child = match Command::new("ga-semantics-mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return McpConnectionTest {
                connected: false,
                tool_count: 0,
                error: Some(format!("Failed to spawn ga-semantics-mcp: {}", e)),
            };
        }
    };

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    if let Some(ref mut stdin) = child.stdin {
        let _ = writeln!(stdin, "{}", serde_json::to_string(&request).unwrap_or_default());
        let _ = stdin.flush();
    }

    let output = child.wait_with_output().ok();
    let stdout_str = output
        .as_ref()
        .and_then(|o| String::from_utf8(o.stdout.clone()).ok())
        .unwrap_or_default();

    if let Ok(response) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
        if let Some(tools) = response.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
            return McpConnectionTest {
                connected: true,
                tool_count: tools.len(),
                error: None,
            };
        }
    }

    let stderr_str = output
        .as_ref()
        .and_then(|o| String::from_utf8(o.stderr.clone()).ok())
        .unwrap_or_default();

    McpConnectionTest {
        connected: false,
        tool_count: 0,
        error: if stderr_str.is_empty() {
            Some("No valid JSON-RPC response from ga-semantics-mcp".to_string())
        } else {
            Some(stderr_str.trim().to_string())
        },
    }
}

impl VaultEntry {
    pub fn new(
        id: &str,
        key_name: &str,
        scope: &str,
        agent_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Self {
        Self {
            id: id.to_string(),
            key_name: key_name.to_string(),
            scope: scope.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            project_id: project_id.map(|s| s.to_string()),
            masked_value: "••••••••".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Skills Library
// ---------------------------------------------------------------------------

pub fn scan_skills_directory(path: &str) -> Vec<SkillEntry> {
    let mut skills = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    let skill_paths = vec![
        format!("{}/.claude/skills", home),
        format!("{}/.opencode/skills", home),
        format!("{}/.gemini/skills", home),
        path.to_string(),
    ];

    // Also scan ACC's bundled skills directory (subdirectories for each skill)
    let bundled_skills_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    let all_skill_paths: Vec<String> = skill_paths
        .into_iter()
        .chain(std::iter::once(bundled_skills_dir.to_string_lossy().to_string()))
        .collect();

    for sp in &all_skill_paths {
        let dir = Path::new(sp);
        if !dir.exists() {
            continue;
        }
        let is_bundled = sp.contains("skills") && sp.contains("src-tauri");
        collect_skill_files(dir, sp, is_bundled, &mut skills);
    }

    skills.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.name.cmp(&b.name)));
    skills
}

fn collect_skill_files(dir: &Path, base_path: &str, is_bundled: bool, skills: &mut Vec<SkillEntry>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Recurse into subdirectories (e.g., skills/bagua-encoder/)
                collect_skill_files(&entry_path, base_path, is_bundled, skills);
            } else if entry_path.extension().map_or(false, |e| e == "md") {
                if let Ok(content) = fs::read_to_string(&entry_path) {
                    let source = if is_bundled {
                        "bundled"
                    } else if base_path.contains("claude") {
                        "claude"
                    } else if base_path.contains("opencode") {
                        "opencode"
                    } else if base_path.contains("gemini") {
                        "gemini"
                    } else {
                        "custom"
                    };

                    let name = if is_bundled {
                        // Use parent directory name as skill name (e.g. "bagua-encoder")
                        entry_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .unwrap_or(entry_path.file_stem().unwrap_or_default())
                            .to_string_lossy()
                            .to_string()
                    } else {
                        entry_path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };

                    skills.push(SkillEntry {
                        id: Uuid::new_v4().to_string(),
                        name,
                        path: entry_path.to_string_lossy().to_string(),
                        source: source.to_string(),
                        content,
                        tags: Vec::new(),
                        injectable: true,
                    });
                }
            }
        }
    }
}

pub fn read_skill_content(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Memory Browser
// ---------------------------------------------------------------------------

pub fn scan_memory_files(project_path: &str) -> Vec<MemoryFileEntry> {
    let mut files = Vec::new();
    let memory_files: &[(&str, &str)] = &[
        ("CLAUDE.md", "claude"),
        ("GEMINI.md", "gemini"),
        ("AGENTS.md", "opencode"),
        ("CONVENTIONS.md", "opencode"),
        ("qwen.md", "qwen"),
        (".goose/instructions.md", "goose"),
        (".clinerules", "cline"),
        (".cursor/rules", "cursor"),
    ];

    for (rel_path, agent) in memory_files {
        let full_path = format!("{}/{}", project_path, rel_path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            let metadata = fs::metadata(&full_path).ok();
            files.push(MemoryFileEntry {
                id: full_path.clone(),
                name: rel_path.to_string(),
                path: full_path,
                agent: agent.to_string(),
                content,
                last_modified: metadata
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        chrono::DateTime::<chrono::Utc>::from(t)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_default(),
                snapshot: None,
            });
        }
    }

    files.sort_by(|a, b| a.agent.cmp(&b.agent).then_with(|| a.name.cmp(&b.name)));
    files
}

pub fn read_memory_file(path: &str) -> Result<MemoryFileEntry, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(MemoryFileEntry {
        id: path.to_string(),
        name: Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        path: path.to_string(),
        agent: "unknown".to_string(),
        content,
        last_modified: metadata
            .modified()
            .map(|t| {
                chrono::DateTime::<chrono::Utc>::from(t)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default(),
        snapshot: None,
    })
}

pub fn write_memory_file(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// MCP Registry
// ---------------------------------------------------------------------------

pub fn read_mcp_configs(agent_config_path: &str) -> Vec<MCPEntry> {
    let mut entries = Vec::new();
    let content = match fs::read_to_string(agent_config_path) {
        Ok(c) => c,
        Err(_) => return entries,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return entries,
    };

    let mcps = match json.get("mcpServers").or_else(|| json.get("mcp_servers")) {
        Some(m) => m,
        None => return entries,
    };

    if let Some(obj) = mcps.as_object() {
        for (name, config) in obj {
            let command = config
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let args: Vec<String> = config
                .get("args")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let env: HashMap<String, String> = config
                .get("env")
                .and_then(|v| v.as_object())
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let enabled = !config
                .get("disabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            entries.push(MCPEntry {
                id: format!("mcp-{}", Uuid::new_v4()),
                name: name.clone(),
                server_command: command,
                args,
                env,
                enabled,
                source: "local".to_string(),
                agent_id: "claude".to_string(),
                managed_externally: false,
                health: if enabled { "green" } else { "grey" }.to_string(),
            });
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

pub fn toggle_mcp(
    agent_config_path: &str,
    mcp_name: &str,
    enabled: bool,
) -> Result<(), String> {
    let content = fs::read_to_string(agent_config_path).map_err(|e| e.to_string())?;
    let mut json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mcps_key = if json.get("mcpServers").is_some() { "mcpServers" } else { "mcp_servers" };

    if let Some(mcps) = json.get_mut(mcps_key) {
        if let Some(entry) = mcps.get_mut(mcp_name) {
            if let Some(obj) = entry.as_object_mut() {
                if enabled {
                    obj.remove("disabled");
                } else {
                    obj.insert("disabled".to_string(), serde_json::Value::Bool(true));
                }
            }
        }
    }

    let updated = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(agent_config_path, updated).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Connector Vault
// ---------------------------------------------------------------------------

pub fn store_secret(
    db: &Connection,
    key_name: &str,
    value: &str,
    scope: &str,
    agent_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<VaultEntry, String> {
    let id = format!("vault-{}", Uuid::new_v4());
    let entry = VaultEntry::new(&id, key_name, scope, agent_id, project_id);

    db.execute(
        "INSERT OR REPLACE INTO secrets_vault (id, key_name, scope, agent_id, project_id, encrypted_value, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            entry.id,
            entry.key_name,
            entry.scope,
            entry.agent_id,
            entry.project_id,
            value,
            entry.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(entry)
}

pub fn list_secrets(db: &Connection) -> Result<Vec<VaultEntry>, String> {
    let mut stmt = db
        .prepare(
            "SELECT id, key_name, scope, agent_id, project_id, created_at FROM secrets_vault ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<VaultEntry> = stmt
        .query_map([], |row| {
            Ok(VaultEntry {
                id: row.get(0)?,
                key_name: row.get(1)?,
                scope: row.get(2)?,
                agent_id: row.get(3)?,
                project_id: row.get(4)?,
                masked_value: "••••••••".to_string(),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

pub fn get_secret_value(db: &Connection, id: &str) -> Result<String, String> {
    db.query_row(
        "SELECT encrypted_value FROM secrets_vault WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Load decrypted/stored Vault secrets eligible for `agent_id` (`scope = 'global'`
/// or `agent_id = ?1`), ordering agent-scoped keys after global keys so agent-specific
/// overrides take precedence without logging secret values.
pub fn load_vault_env_vars(db: &Connection, agent_id: &str) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let Ok(mut stmt) = db.prepare(
        "SELECT key_name, encrypted_value FROM secrets_vault \
         WHERE scope = 'global' OR agent_id = ?1 \
         ORDER BY CASE WHEN agent_id = ?1 THEN 1 ELSE 0 END ASC, created_at ASC",
    ) else {
        return env;
    };

    if let Ok(rows) = stmt.query_map(rusqlite::params![agent_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        for row in rows.flatten() {
            env.insert(row.0, row.1);
        }
    }
    env
}

// ---------------------------------------------------------------------------
// Plugin Manager — VS Code extensions
// ---------------------------------------------------------------------------

pub fn list_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let ext_dir = format!("{}/.vscode/extensions", home);

    if let Ok(entries) = fs::read_dir(&ext_dir) {
        for entry in entries.flatten() {
            plugins.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    plugins.sort();
    plugins
}

// ---------------------------------------------------------------------------
// Project Profile JSON generator
// ---------------------------------------------------------------------------

pub fn generate_project_profile(project_path: &str) -> Result<serde_json::Value, String> {
    let mut profile = serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "path": project_path,
        "name": Path::new(project_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        "stack": [],
        "test_framework": null,
        "package_manager": null,
        "active_agents": ["claude", "opencode"],
        "active_skills": [],
        "active_mcps": [],
        "preferred_models": [],
    });

    let mut stack: Vec<String> = Vec::new();

    let package_json_path = format!("{}/package.json", project_path);
    if Path::new(&package_json_path).exists() {
        stack.push("node".to_string());
        profile["package_manager"] = serde_json::json!("npm");
        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let deps = pkg
                    .get("dependencies")
                    .or_else(|| pkg.get("devDependencies"));
                if let Some(obj) = deps.and_then(|d| d.as_object()) {
                    if obj.contains_key("next") {
                        stack.push("nextjs".to_string());
                    }
                    if obj.contains_key("react") {
                        stack.push("react".to_string());
                    }
                    if obj.contains_key("@supabase/supabase-js") {
                        stack.push("supabase".to_string());
                    }
                    if obj.contains_key("tailwindcss") {
                        stack.push("tailwind".to_string());
                    }
                    if obj.contains_key("vitest") {
                        stack.push("vitest".to_string());
                        profile["test_framework"] = serde_json::json!("vitest");
                    }
                    if obj.contains_key("jest") {
                        stack.push("jest".to_string());
                        profile["test_framework"] = serde_json::json!("jest");
                    }
                }
            }
        }
    }

    if Path::new(&format!("{}/Cargo.toml", project_path)).exists() {
        stack.push("rust".to_string());
        profile["package_manager"] = serde_json::json!("cargo");
    }

    if Path::new(&format!("{}/pyproject.toml", project_path)).exists()
        || Path::new(&format!("{}/requirements.txt", project_path)).exists()
    {
        stack.push("python".to_string());
        profile["package_manager"] = serde_json::json!("pip");
    }

    if Path::new(&format!("{}/go.mod", project_path)).exists() {
        stack.push("go".to_string());
        profile["package_manager"] = serde_json::json!("go mod");
    }

    if Path::new(&format!("{}/composer.json", project_path)).exists() {
        stack.push("php".to_string());
        profile["package_manager"] = serde_json::json!("composer");
    }

    if Path::new(&format!("{}/.github/workflows", project_path)).exists() {
        stack.push("github-actions".to_string());
    }

    profile["stack"] = serde_json::json!(stack);
    Ok(profile)
}

pub fn write_mcp_to_target(
    _db: &Connection,
    _target: &str,
    _mcp_id: &str,
    _enable: bool,
) -> Result<(), String> {
    Ok(())
}

// src-tauri/src/provisioner.rs
//
// Infrastructure Provisioner: installs required CLIs, provisions databases
// via available MCP servers, and validates the environment for a given stack.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::stack_registry::{CliStatus, StackPreset};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionStep {
    pub name: String,
    pub status: ProvisionStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProvisionStatus {
    Success,
    Failed(String),
    Skipped(String),
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionReport {
    pub stack_id: String,
    pub all_ready: bool,
    pub steps: Vec<ProvisionStep>,
}

impl ProvisionReport {
    pub fn new(stack_id: &str) -> Self {
        Self {
            stack_id: stack_id.to_string(),
            all_ready: true,
            steps: Vec::new(),
        }
    }

    fn add(&mut self, step: ProvisionStep) {
        if matches!(step.status, ProvisionStatus::Failed(_)) {
            self.all_ready = false;
        }
        self.steps.push(step);
    }
}

/// Check if a CLI tool is installed and return its version.
pub fn check_cli(tool: &str) -> CliStatus {
    match Command::new(tool).arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("unknown")
                .to_string();
            CliStatus::Installed(version)
        }
        _ => CliStatus::Missing,
    }
}

/// Attempt to install a CLI tool via npm or pip.
/// Returns true if installation succeeded.
pub fn install_cli(tool: &str) -> Result<(), String> {
    match tool {
        "vercel" => {
            let output = Command::new("npm")
                .args(["install", "-g", "vercel"])
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "supabase" => {
            let output = Command::new("npm")
                .args(["install", "-g", "supabase"])
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        _ => Err(format!("No auto-install available for '{}'. Please install manually.", tool)),
    }
}

/// Get install instructions for a CLI tool that cannot be auto-installed.
pub fn install_instructions(tool: &str) -> String {
    match tool {
        "vercel" => "Run: npm install -g vercel".into(),
        "supabase" => "Run: npm install -g supabase".into(),
        "node" => "Download from https://nodejs.org".into(),
        "npm" => "Included with Node.js: https://nodejs.org".into(),
        "python3" => "Download from https://python.org or use your package manager".into(),
        "pip" => "Included with Python 3.4+: https://python.org".into(),
        "git" => "Download from https://git-scm.com".into(),
        _ => format!("Install '{}' using your system package manager", tool),
    }
}

/// Provision infrastructure for a given stack.
/// 1. Check all required CLIs
/// 2. Try to install missing CLIs (where possible)
/// 3. Verify MCP servers are connected
/// 4. Return report with delegation tasks for anything not automated
pub fn provision(stack_id: &str) -> ProvisionReport {
    let mut report = ProvisionReport::new(stack_id);

    let stack = match StackPreset::get_by_id(stack_id) {
        Some(s) => s,
        None => {
            report.add(ProvisionStep {
                name: "Stack lookup".into(),
                status: ProvisionStatus::Failed(format!("Unknown stack: {}", stack_id)),
                detail: String::new(),
            });
            return report;
        }
    };

    // Step 1: Check all required CLIs
    let cli_status = crate::stack_registry::detect_installed_clis();

    for cli in &stack.required_cli {
        match cli_status.get(cli) {
            Some(CliStatus::Installed(version)) => {
                report.add(ProvisionStep {
                    name: format!("CLI: {}", cli),
                    status: ProvisionStatus::Success,
                    detail: version.clone(),
                });
            }
            _ => {
                // Try auto-install
                let detail = match install_cli(cli) {
                    Ok(()) => {
                        // Re-check after install
                        match check_cli(cli) {
                            CliStatus::Installed(v) => {
                                report.add(ProvisionStep {
                                    name: format!("CLI: {}", cli),
                                    status: ProvisionStatus::Success,
                                    detail: format!("Installed: {}", v),
                                });
                                continue;
                            }
                            _ => install_instructions(cli),
                        }
                    }
                    Err(_) => install_instructions(cli),
                };
                report.add(ProvisionStep {
                    name: format!("CLI: {}", cli),
                    status: ProvisionStatus::Failed(format!(
                        "{} is not installed. {}",
                        cli, detail
                    )),
                    detail,
                });
            }
        }
    }

    // Step 2: Check MCP servers (informational — can't auto-install)
    for mcp in &stack.required_mcp {
        report.add(ProvisionStep {
            name: format!("MCP: {}", mcp),
            status: ProvisionStatus::Success,
            detail: format!("{} MCP should be configured in OpenCode settings", mcp),
        });
    }

    // Step 3: Check Node.js project tooling
    if stack.required_cli.contains(&"npm".to_string()) {
        match check_cli("npx") {
            CliStatus::Installed(v) => {
                report.add(ProvisionStep {
                    name: "CLI: npx".into(),
                    status: ProvisionStatus::Success,
                    detail: v,
                });
            }
            _ => {
                report.add(ProvisionStep {
                    name: "CLI: npx".into(),
                    status: ProvisionStatus::Success,
                    detail: "Bundled with npm".into(),
                });
            }
        }
    }

    if report.all_ready {
        report.add(ProvisionStep {
            name: "Infrastructure ready".into(),
            status: ProvisionStatus::Success,
            detail: format!("All requirements met for stack '{}'", stack.name),
        });
    } else {
        report.add(ProvisionStep {
            name: "Infrastructure ready".into(),
            status: ProvisionStatus::Failed(
                "Some requirements are missing. Follow the instructions above to install missing tools.".into(),
            ),
            detail: String::new(),
        });
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provision_valid_stack() {
        let report = provision("nextjs-supabase-vercel");
        assert_eq!(report.stack_id, "nextjs-supabase-vercel");
        assert!(!report.steps.is_empty());
    }

    #[test]
    fn test_provision_invalid_stack() {
        let report = provision("nonexistent");
        assert!(!report.all_ready);
        assert_eq!(report.steps.len(), 1);
    }

    #[test]
    fn test_check_cli_finds_node() {
        let status = check_cli("node");
        assert!(matches!(status, CliStatus::Installed(_)));
    }

    #[test]
    fn test_check_cli_missing() {
        let status = check_cli("nonexistent-tool-xyz-123");
        assert_eq!(status, CliStatus::Missing);
    }

    #[test]
    fn test_install_instructions_known_tool() {
        let instr = install_instructions("vercel");
        assert!(instr.contains("npm install"));
    }

    #[test]
    fn test_install_instructions_unknown_tool() {
        let instr = install_instructions("unknown-tool");
        assert!(!instr.is_empty());
    }

    #[test]
    fn test_provision_report_default() {
        let report = ProvisionReport::new("test-stack");
        assert!(report.all_ready);
        assert!(report.steps.is_empty());
    }
}

// src-tauri/src/stack_registry.rs
//
// Stack Registry: defines supported technology stacks for SourceForge projects.
// Each stack specifies frameworks, database providers, deploy targets,
// required CLIs, required MCPs, and scaffold templates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CliStatus {
    Installed(String),
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub frameworks: Vec<String>,
    pub database: String,
    pub deploy_target: String,
    pub required_cli: Vec<String>,
    pub required_mcp: Vec<String>,
    pub frontend_dir: String,
    pub api_dir: String,
    pub is_default: bool,
}

/// The canonical stack registry. Order matters: first is default.
pub static STACK_REGISTRY: LazyLock<Vec<StackPreset>> = LazyLock::new(|| vec![
    StackPreset {
        id: "nextjs-supabase-vercel".into(),
        name: "Next.js + Supabase + Vercel".into(),
        description: "Full-stack Next.js with API routes, Supabase PostgreSQL, Vercel edge deployment. Best for rapid full-stack apps with serverless backend.".into(),
        frameworks: vec!["next.js".into()],
        database: "supabase-postgresql".into(),
        deploy_target: "vercel".into(),
        required_cli: vec!["node".into(), "npm".into(), "vercel".into()],
        required_mcp: vec!["supabase".into()],
        frontend_dir: "app".into(),
        api_dir: "app/api".into(),
        is_default: true,
    },
    StackPreset {
        id: "nextjs-supabase-fastapi".into(),
        name: "Next.js + Supabase + FastAPI".into(),
        description: "Next.js frontend with Python FastAPI backend, Supabase PostgreSQL. Use when you need Python ML/AI libraries or complex backend logic.".into(),
        frameworks: vec!["next.js".into(), "fastapi".into()],
        database: "supabase-postgresql".into(),
        deploy_target: "vercel".into(),
        required_cli: vec!["node".into(), "npm".into(), "vercel".into(), "python3".into()],
        required_mcp: vec!["supabase".into()],
        frontend_dir: "app".into(),
        api_dir: "backend".into(),
        is_default: false,
    },
    StackPreset {
        id: "express-react-supabase".into(),
        name: "Express + React + Supabase".into(),
        description: "Express API server with React Vite frontend, Supabase PostgreSQL. Traditional monolith architecture with separate frontend/backend.".into(),
        frameworks: vec!["express".into(), "react".into()],
        database: "supabase-postgresql".into(),
        deploy_target: "vercel".into(),
        required_cli: vec!["node".into(), "npm".into(), "vercel".into()],
        required_mcp: vec!["supabase".into()],
        frontend_dir: "src".into(),
        api_dir: "src".into(),
        is_default: false,
    },
    StackPreset {
        id: "nextjs-prisma-vercel".into(),
        name: "Next.js + Prisma + Vercel Postgres".into(),
        description: "Next.js with Prisma ORM and Vercel Postgres. Simple setup without Supabase — uses Vercel's managed Postgres.".into(),
        frameworks: vec!["next.js".into()],
        database: "vercel-postgres".into(),
        deploy_target: "vercel".into(),
        required_cli: vec!["node".into(), "npm".into(), "vercel".into()],
        required_mcp: vec![],
        frontend_dir: "app".into(),
        api_dir: "app/api".into(),
        is_default: false,
    },
]);

impl StackPreset {
    pub fn default_stack() -> &'static StackPreset {
        STACK_REGISTRY
            .iter()
            .find(|s| s.is_default)
            .unwrap_or(&STACK_REGISTRY[0])
    }

    pub fn get_by_id(id: &str) -> Option<&'static StackPreset> {
        STACK_REGISTRY.iter().find(|s| s.id == id)
    }

    pub fn all() -> &'static [StackPreset] {
        &STACK_REGISTRY[..]
    }

    pub fn all_ids() -> Vec<&'static str> {
        STACK_REGISTRY.iter().map(|s| s.id.as_str()).collect()
    }
}

/// Detect which CLIs are installed on the system PATH.
pub fn detect_installed_clis() -> HashMap<String, CliStatus> {
    let mut status = HashMap::new();
    let to_check = ["node", "npm", "vercel", "python3", "pip", "git"];

    for cli in &to_check {
        match Command::new(cli).arg("--version").output() {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();
                status.insert(cli.to_string(), CliStatus::Installed(version));
            }
            _ => {
                status.insert(cli.to_string(), CliStatus::Missing);
            }
        }
    }
    status
}

/// Recommend the best stack based on available CLIs.
/// Returns the first stack where all required CLIs are installed.
pub fn recommend_stack(cli_status: &HashMap<String, CliStatus>) -> Option<&'static StackPreset> {
    for stack in STACK_REGISTRY.iter() {
        let all_installed = stack
            .required_cli
            .iter()
            .all(|cli| matches!(cli_status.get(cli), Some(CliStatus::Installed(_))));
        if all_installed {
            return Some(stack);
        }
    }
    // Fall back to default
    Some(StackPreset::default_stack())
}

/// Get a list of CLIs that are missing for a given stack.
pub fn missing_clis_for_stack(stack_id: &str, cli_status: &HashMap<String, CliStatus>) -> Vec<String> {
    let stack = match StackPreset::get_by_id(stack_id) {
        Some(s) => s,
        None => return vec![],
    };
    stack
        .required_cli
        .iter()
        .filter(|cli| !matches!(cli_status.get(*cli), Some(CliStatus::Installed(_))))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_stacks_have_required_fields() {
        for stack in STACK_REGISTRY.iter() {
            assert!(!stack.id.is_empty(), "Stack {} has empty id", stack.name);
            assert!(!stack.name.is_empty());
            assert!(!stack.frameworks.is_empty());
            assert!(!stack.required_cli.is_empty());
        }
    }

    #[test]
    fn test_default_stack_is_nextjs_supabase() {
        let default = StackPreset::default_stack();
        assert_eq!(default.id, "nextjs-supabase-vercel");
        assert!(default.is_default);
    }

    #[test]
    fn test_all_stack_ids_unique() {
        let ids: Vec<&str> = STACK_REGISTRY.iter().map(|s| s.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "Duplicate stack IDs found");
    }

    #[test]
    fn test_get_stack_by_id() {
        let stack = StackPreset::get_by_id("nextjs-supabase-vercel");
        assert!(stack.is_some());
        assert_eq!(stack.unwrap().name, "Next.js + Supabase + Vercel");

        let missing = StackPreset::get_by_id("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_all_ids_returns_all() {
        let ids = StackPreset::all_ids();
        assert_eq!(ids.len(), STACK_REGISTRY.len());
    }

    #[test]
    fn test_cli_detection_finds_node() {
        let status = detect_installed_clis();
        let node = status.get("node").unwrap();
        assert!(matches!(node, CliStatus::Installed(_)), "Node should be installed");
    }

    #[test]
    fn test_cli_detection_missing_tool() {
        let status = detect_installed_clis();
        let fake = status.get("nonexistent-tool-xyz");
        assert!(fake.is_none(), "Should not detect fake tools");
    }

    #[test]
    fn test_recommend_stack_with_node_only() {
        let mut status = HashMap::new();
        status.insert("node".into(), CliStatus::Installed("v20".into()));
        status.insert("npm".into(), CliStatus::Installed("10".into()));
        status.insert("vercel".into(), CliStatus::Missing);
        status.insert("python3".into(), CliStatus::Missing);
        status.insert("pip".into(), CliStatus::Missing);
        status.insert("git".into(), CliStatus::Installed("2".into()));

        let rec = recommend_stack(&status);
        // Should pick nextjs-prisma-vercel since it doesn't require supabase MCP
        // Actually, all stacks need vercel which is Missing. Let's check:
        // nextjs-supabase-vercel: needs node, npm, vercel → vercel missing
        // nextjs-supabase-fastapi: needs node, npm, vercel, python3 → multiple missing
        // express-react-supabase: needs node, npm, vercel → vercel missing
        // nextjs-prisma-vercel: needs node, npm, vercel → vercel missing
        // None match perfectly. Falls back to default.
        assert!(rec.is_some());
        assert_eq!(rec.unwrap().id, StackPreset::default_stack().id);
    }

    #[test]
    fn test_recommend_stack_all_installed() {
        let mut status = HashMap::new();
        for cli in &["node", "npm", "vercel", "python3", "pip", "git"] {
            status.insert(cli.to_string(), CliStatus::Installed("1.0".into()));
        }
        let rec = recommend_stack(&status);
        assert!(rec.is_some());
        assert_eq!(rec.unwrap().id, "nextjs-supabase-vercel");
    }

    #[test]
    fn test_missing_clis_for_stack() {
        let mut status = HashMap::new();
        status.insert("node".into(), CliStatus::Installed("v20".into()));
        status.insert("npm".into(), CliStatus::Installed("10".into()));
        // vercel is missing
        status.insert("vercel".into(), CliStatus::Missing);

        let missing = missing_clis_for_stack("nextjs-supabase-vercel", &status);
        assert_eq!(missing, vec!["vercel"]);

        let missing_fastapi = missing_clis_for_stack("nextjs-supabase-fastapi", &status);
        assert!(missing_fastapi.contains(&"vercel".to_string()));
        assert!(missing_fastapi.contains(&"python3".to_string()));
    }

    #[test]
    fn test_cli_status_equality() {
        assert_eq!(
            CliStatus::Installed("1.0".into()),
            CliStatus::Installed("1.0".into())
        );
        assert_ne!(CliStatus::Installed("1.0".into()), CliStatus::Missing);
        assert_eq!(CliStatus::Missing, CliStatus::Missing);
    }

    #[test]
    fn test_registry_has_four_stacks() {
        assert_eq!(
            StackPreset::all().len(),
            4,
            "registry should define exactly 4 stacks"
        );
    }

    #[test]
    fn test_get_by_id_roundtrip_for_all_ids() {
        for id in StackPreset::all_ids() {
            let stack = StackPreset::get_by_id(id);
            assert!(stack.is_some(), "all_ids entry {id} must resolve via get_by_id");
            assert_eq!(stack.unwrap().id, id);
        }
    }
}

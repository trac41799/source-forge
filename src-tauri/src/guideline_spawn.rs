// src-tauri/src/guideline_spawn.rs
//
// T3: Guideline + Spawn helper (Closes G6)
// The "guideline" is the per-agent task spec. Convention:
// 1. Generate the guideline markdown via orchestrator::generate_agent_guideline
// 2. Write to <worktree>/.acc/GUIDELINE.md
// 3. Pass --guideline <path> as a CLI arg when spawning the agent
//
// This module wires 1+2+3 together so the caller (execute_wave) just calls
// one function: `prepare_and_spawn(...)` which returns the spawn args.

use std::fs;
use std::path::{Path, PathBuf};

use crate::orchestrator;

pub const GUIDELINE_REL_DIR: &str = ".acc";
pub const GUIDELINE_FILENAME: &str = "GUIDELINE.md";
pub const GUIDELINE_ARG_FLAG: &str = "--guideline";

/// Write the guideline markdown to <worktree>/.acc/GUIDELINE.md.
/// Returns the absolute path to the written file.
#[allow(clippy::too_many_arguments)]
pub fn write_guideline_to_worktree(
    worktree_path: &str,
    agent_ref: &str,
    task: &str,
    objective: &str,
    depends_on: Option<&str>,
    models: &[&str],
    files_to_create: &[&str],
    files_not_touch: &[&str],
) -> Result<PathBuf, String> {
    let guideline = orchestrator::generate_agent_guideline(
        agent_ref,
        task,
        objective,
        depends_on,
        models,
        files_to_create,
        files_not_touch,
    );

    let dir = Path::new(worktree_path).join(GUIDELINE_REL_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create .acc dir: {}", e))?;

    let path = dir.join(GUIDELINE_FILENAME);
    fs::write(&path, &guideline).map_err(|e| format!("Failed to write guideline: {}", e))?;

    Ok(path)
}

/// Build the full argv for spawning an agent CLI that reads --guideline.
/// `base_args` are the agent's normal args (e.g., ["--model", "mimo-v2.5"]).
/// Returns: base_args + ["--guideline", <abs_path>]
pub fn build_spawn_args(base_args: &[String], guideline_path: &Path) -> Vec<String> {
    let mut args = base_args.to_vec();
    args.push(GUIDELINE_ARG_FLAG.to_string());
    args.push(guideline_path.to_string_lossy().to_string());
    args
}

/// Convenience: write guideline + return spawn args in one call.
#[allow(clippy::too_many_arguments)]
pub fn prepare_spawn(
    worktree_path: &str,
    agent_ref: &str,
    task: &str,
    objective: &str,
    depends_on: Option<&str>,
    models: &[&str],
    files_to_create: &[&str],
    files_not_touch: &[&str],
    base_args: &[String],
) -> Result<(PathBuf, Vec<String>), String> {
    let guideline_path = write_guideline_to_worktree(
        worktree_path,
        agent_ref,
        task,
        objective,
        depends_on,
        models,
        files_to_create,
        files_not_touch,
    )?;
    let args = build_spawn_args(base_args, &guideline_path);
    Ok((guideline_path, args))
}

/// Query top project/wave memory facts and stack anti-pattern warnings and append a
/// `## Project Memory & Anti-Pattern Guardrails` section to `<worktree_path>/.acc/GUIDELINE.md`
/// when at least one item is present.
pub fn append_project_context_to_guideline(
    db: &rusqlite::Connection,
    worktree_path: &str,
    project_id: &str,
    agent_ref: &str,
    stack_tags: &[&str],
) -> Result<(), String> {
    let mut sections: Vec<String> = Vec::new();

    let mut memory_lines: Vec<String> = Vec::new();
    match crate::memory::get_context(db, agent_ref, "wave", None, Some(500)) {
        Ok(ctx) => {
            let trimmed = ctx.trim();
            if !trimmed.is_empty() && trimmed != "Prior knowledge: []" {
                if let Some(json_part) = trimmed.strip_prefix("Prior knowledge: ") {
                    if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(json_part) {
                        for item in items {
                            let ftype = item.get("type").and_then(|v| v.as_str()).unwrap_or("fact");
                            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            if !content.is_empty() {
                                memory_lines.push(format!("- [{}] {}", ftype, content));
                            }
                        }
                    } else {
                        memory_lines.push(format!("- {}", trimmed));
                    }
                } else {
                    memory_lines.push(format!("- {}", trimmed));
                }
            }
        }
        Err(e) if e.contains("no such table") => {}
        Err(e) => return Err(e),
    }

    if memory_lines.is_empty() && !project_id.is_empty() {
        let query = crate::memory::MemoryQuery {
            agent_id: None,
            session_id: None,
            org_id: Some(project_id.to_string()),
            fact_type: None,
            min_confidence: Some(0.5),
            q: None,
            limit: Some(10),
            offset: Some(0),
        };
        match crate::memory::get_memory_facts(db, &query) {
            Ok(facts) => {
                for f in facts {
                    memory_lines.push(format!("- [{}] {}", f.fact_type, f.content));
                }
            }
            Err(e) if e.contains("no such table") => {}
            Err(e) => return Err(e),
        }
    }

    if !memory_lines.is_empty() {
        sections.push(format!("### Project Memory\n{}", memory_lines.join("\n")));
    }

    let mut seen_ids = std::collections::HashSet::new();
    let mut warnings: Vec<crate::knowledge::PreflightWarning> = Vec::new();
    let tag_queries: Vec<&str> = if stack_tags.is_empty() {
        vec![""]
    } else {
        stack_tags.to_vec()
    };
    for tag in tag_queries {
        match crate::knowledge::get_preflight_warnings(db, tag, 10) {
            Ok(items) => {
                for w in items {
                    if seen_ids.insert(w.id.clone()) {
                        warnings.push(w);
                    }
                }
            }
            Err(e) if e.contains("no such table") => {}
            Err(e) => return Err(e),
        }
    }
    warnings.truncate(10);

    if !warnings.is_empty() {
        let warning_lines: Vec<String> = warnings
            .iter()
            .map(|w| {
                format!(
                    "- **{}** (confidence {:.2}): {}",
                    w.title, w.confidence, w.content
                )
            })
            .collect();
        sections.push(format!(
            "### Anti-Pattern Guardrails\n{}",
            warning_lines.join("\n")
        ));
    }

    if sections.is_empty() {
        return Ok(());
    }

    let dir = Path::new(worktree_path).join(GUIDELINE_REL_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create .acc dir: {}", e))?;
    let path = dir.join(GUIDELINE_FILENAME);

    let existing = if path.exists() {
        fs::read_to_string(&path).map_err(|e| format!("Failed to read guideline: {}", e))?
    } else {
        String::new()
    };

    let block = format!(
        "## Project Memory & Anti-Pattern Guardrails\n\n{}",
        sections.join("\n\n")
    );
    let updated = if existing.trim().is_empty() {
        block
    } else if existing.contains("## Project Memory & Anti-Pattern Guardrails") {
        existing
    } else {
        format!("{}\n\n{}\n", existing.trim_end(), block)
    };

    fs::write(&path, updated).map_err(|e| format!("Failed to write guideline: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_guideline_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = write_guideline_to_worktree(
            tmp.path().to_str().unwrap(),
            "agent-1",
            "Write tests for X",
            "Cover edge cases",
            None,
            &["mimo-v2.5"],
            &["src/x.test.ts"],
            &["package.json"],
        )
        .unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("agent-1"));
        assert!(content.contains("Write tests for X"));
        assert!(content.contains("Cover edge cases"));
        assert!(content.contains("mimo-v2.5"));
        assert!(content.contains("src/x.test.ts"));
        assert!(content.contains("package.json"));
    }

    #[test]
    fn test_write_guideline_creates_dot_acc_dir() {
        let tmp = TempDir::new().unwrap();
        let path = write_guideline_to_worktree(
            tmp.path().to_str().unwrap(),
            "a",
            "t", "o", None,
            &["m"], &[], &[],
        )
        .unwrap();

        assert!(path.parent().unwrap().ends_with(GUIDELINE_REL_DIR));
    }

    #[test]
    fn test_write_guideline_depends_on_in_output() {
        let tmp = TempDir::new().unwrap();
        let path = write_guideline_to_worktree(
            tmp.path().to_str().unwrap(),
            "a",
            "t", "o",
            Some("agent-0"),
            &["m"], &[], &[],
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("agent-0"));
    }

    #[test]
    fn test_build_spawn_args_adds_guideline_flag() {
        let base = vec!["--model".to_string(), "mimo-v2.5".to_string()];
        let guideline = PathBuf::from("/tmp/wt/.acc/GUIDELINE.md");

        let args = build_spawn_args(&base, &guideline);

        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--model");
        assert_eq!(args[1], "mimo-v2.5");
        assert_eq!(args[2], "--guideline");
        assert!(args[3].contains("GUIDELINE.md"));
    }

    #[test]
    fn test_build_spawn_args_empty_base() {
        let base: Vec<String> = vec![];
        let guideline = PathBuf::from("/tmp/wt/.acc/GUIDELINE.md");

        let args = build_spawn_args(&base, &guideline);

        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--guideline");
    }

    #[test]
    fn test_prepare_spawn_returns_both() {
        let tmp = TempDir::new().unwrap();
        let base = vec!["--model".to_string(), "mimo-v2.5".to_string()];

        let (path, args) = prepare_spawn(
            tmp.path().to_str().unwrap(),
            "agent-x",
            "do the thing",
            "achieve the goal",
            None,
            &["mimo-v2.5"],
            &[],
            &[],
            &base,
        )
        .unwrap();

        assert!(path.exists());
        assert_eq!(args.len(), 4);
        assert_eq!(args[2], "--guideline");
    }

    #[test]
    fn test_append_project_context_to_guideline() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/010_knowledge_graph.sql"))
            .unwrap();
        let _ = conn.execute_batch(include_str!("../migrations/011_memory.sql"));
        conn.execute_batch(include_str!("../migrations/014_bagua_semantics.sql"))
            .unwrap();

        // Seed memory fact for (agent_ref, "wave")
        crate::memory::create_memory_fact(
            &conn,
            &crate::memory::MemoryFactInput {
                agent_id: "agent-rust".to_string(),
                session_id: "wave".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj-1".to_string(),
                fact_type: "constraint".to_string(),
                content: "Always run cargo test with MSVC toolchain".to_string(),
                metadata: None,
                confidence: Some(0.95),
            },
        )
        .unwrap();

        // Seed anti-pattern warning for stack "rust"
        conn.execute(
            "INSERT INTO knowledge_items (id, type, title, content, confidence, confirmation_count, is_global, status, stack_tags, first_seen, last_confirmed)
             VALUES ('ap-1', 'antipattern', 'Never use format! in SQL queries', 'Use rusqlite::params! instead', 0.92, 4, 0, 'active', 'rust', datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();

        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().to_str().unwrap();
        let guideline_path = write_guideline_to_worktree(
            wt,
            "agent-rust",
            "Fix memory.rs",
            "Parameterize SQL",
            None,
            &["mimo-v2.5"],
            &["src-tauri/src/memory.rs"],
            &[],
        )
        .unwrap();

        append_project_context_to_guideline(&conn, wt, "proj-1", "agent-rust", &["rust"]).unwrap();

        let content = fs::read_to_string(&guideline_path).unwrap();
        assert!(content.contains("## Project Memory & Anti-Pattern Guardrails"));
        assert!(content.contains("Always run cargo test with MSVC toolchain"));
        assert!(content.contains("Never use format! in SQL queries"));
    }

    #[test]
    fn test_append_project_context_skips_when_empty_and_supports_org_fallback() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/010_knowledge_graph.sql"))
            .unwrap();
        let _ = conn.execute_batch(include_str!("../migrations/011_memory.sql"));
        conn.execute_batch(include_str!("../migrations/014_bagua_semantics.sql"))
            .unwrap();

        let tmp = TempDir::new().unwrap();
        let wt = tmp.path().to_str().unwrap();
        let guideline_path = write_guideline_to_worktree(
            wt,
            "agent-rust",
            "Task",
            "Objective",
            None,
            &["mimo-v2.5"],
            &[],
            &[],
        )
        .unwrap();

        // Empty DB: must not append the guardrails section
        append_project_context_to_guideline(&conn, wt, "proj-org", "agent-rust", &["rust"]).unwrap();
        let before = fs::read_to_string(&guideline_path).unwrap();
        assert!(!before.contains("## Project Memory & Anti-Pattern Guardrails"));

        // Project-level fact stored with org_id = "proj-org" (different agent/session)
        crate::memory::create_memory_fact(
            &conn,
            &crate::memory::MemoryFactInput {
                agent_id: "prior-agent".to_string(),
                session_id: "s-prev".to_string(),
                user_id: "u1".to_string(),
                org_id: "proj-org".to_string(),
                fact_type: "decision".to_string(),
                content: "Prefer parameterized SQLite queries".to_string(),
                metadata: None,
                confidence: Some(0.9),
            },
        )
        .unwrap();

        append_project_context_to_guideline(&conn, wt, "proj-org", "agent-rust", &["rust"]).unwrap();
        let after = fs::read_to_string(&guideline_path).unwrap();
        assert!(after.contains("## Project Memory & Anti-Pattern Guardrails"));
        assert!(after.contains("Prefer parameterized SQLite queries"));
    }
}

// src-tauri/src/handoff_parser.rs
//
// T4: Handoff Watcher Parser (Closes G2)
// Parses HANDOFF_<agent_ref>.md files written by agents when they complete.
// Validates the schema, extracts structured fields, and exposes
// update_handoff_status() to mark plan_agents accordingly.

use std::fs;
use std::path::Path;

use crate::orchestrator;
use rusqlite::Connection;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandoffEnvelope {
    pub original_task: String,
    pub completed_by: String,
    pub model_used: String,
    pub output_summary: String,
    pub changed_files: Vec<String>,
    pub handoff_instruction: String,
    pub raw_path: String,
}

/// Parse a HANDOFF_<agent_ref>.md file.
/// Returns Err if file doesn't exist, is empty, or has missing sections.
pub fn parse_handoff_file(path: &Path) -> Result<HandoffEnvelope, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read handoff file: {}", e))?;

    if content.trim().is_empty() {
        return Err("Handoff file is empty".to_string());
    }

    let (valid, missing) = orchestrator::validate_handoff_schema(&content);
    if !valid {
        return Err(format!("Missing required sections: {}", missing.join(", ")));
    }

    let envelope = HandoffEnvelope {
        original_task: extract_section(&content, "Original Task").unwrap_or_default(),
        completed_by: extract_section(&content, "Completed By").unwrap_or_default(),
        model_used: extract_section(&content, "Model Used").unwrap_or_default(),
        output_summary: extract_section(&content, "Output Summary").unwrap_or_default(),
        changed_files: extract_list(&content, "Files Changed"),
        handoff_instruction: extract_section(&content, "Handoff Instructions").unwrap_or_default(),
        raw_path: path.to_string_lossy().to_string(),
    };

    Ok(envelope)
}

/// Extract the content of a markdown section: `## Header\n...\n## Other\n...`
fn extract_section(content: &str, header: &str) -> Option<String> {
    let needle = format!("## {}", header);
    let start = content.find(&needle)? + needle.len();
    let rest = &content[start..];
    // Find next `## ` at start of line
    let end = rest
        .lines()
        .position(|l| l.starts_with("## "))
        .map(|p| {
            let mut idx = 0;
            for (i, l) in rest.lines().enumerate() {
                if i == p {
                    break;
                }
                idx += l.len() + 1; // +1 for newline
            }
            idx
        })
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// Extract a markdown list under `## Header` (one item per `- ` line).
fn extract_list(content: &str, header: &str) -> Vec<String> {
    if let Some(section) = extract_section(content, header) {
        section
            .lines()
            .filter_map(|l| l.trim_start().strip_prefix("- ").map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

/// Detect if a path looks like a handoff file.
/// Convention: `HANDOFF_<agent_ref>.md` (case-sensitive, .md extension).
#[allow(dead_code)]
pub fn is_handoff_file(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    name.starts_with("HANDOFF_") && name.ends_with(".md")
}

/// Extract the agent_ref from a handoff file name.
/// `HANDOFF_frontend.md` -> `"frontend"`.
#[allow(dead_code)]
pub fn agent_ref_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name().and_then(|n| n.to_str())?;
    if !is_handoff_file(path) {
        return None;
    }
    let stem = name.strip_prefix("HANDOFF_")?.strip_suffix(".md")?;
    Some(stem.to_string())
}

/// M3 (spec R30): semantic handoff confidence in [0,1] via a `noul`, or `None`
/// when no backend is configured. Callers route review-band results to the
/// `decision_reviews` queue.
pub fn semantic_handoff_confidence(db: &Connection, env: &HandoffEnvelope) -> Option<f64> {
    let cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let key = std::env::var("OPENROUTER_API_KEY").ok();
    if key.is_none() && cfg.backend != "local" {
        return None;
    }
    let transport = crate::decision::UreqTransport { timeout_ms: cfg.timeout_ms };
    let state = serde_json::json!({
        "original_task": env.original_task,
        "output_summary": env.output_summary,
        "changed_files": env.changed_files,
        "handoff_instruction": env.handoff_instruction,
    });
    crate::decision::judge(
        &cfg,
        &transport,
        key.as_deref(),
        &state,
        "Does `output_summary` plausibly complete `original_task`, and is `handoff_instruction` actionable for the next agent?",
        "ok",
        Some(db),
    )
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const VALID_HANDOFF: &str = r#"# HANDOFF frontend

## Original Task
Build the dashboard widget.

## Completed By
frontend-agent

## Model Used
mimo-v2.5

## Output Summary
Created a responsive widget with 3 panels.

## Completed Work
Implemented Widget with responsive grid and data fetching.

## Test Results
- Widget.test.tsx: 6 passed, 0 failed

## Files Changed
- src/components/Widget.tsx
- src/styles/widget.css
- src/__tests__/Widget.test.tsx

## Files NOT Modified
- package.json
- tauri.conf.json

## Design Decisions
Used grid layout for responsiveness.

## Interface Contracts Exposed
- <Widget data={...} />

## Handoff Instructions
The next agent should integrate with the auth flow.
"#;

    #[test]
    fn test_parse_valid_handoff() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("HANDOFF_frontend.md");
        fs::write(&path, VALID_HANDOFF).unwrap();

        let env = parse_handoff_file(&path).expect("should parse");
        assert_eq!(env.original_task, "Build the dashboard widget.");
        assert_eq!(env.completed_by, "frontend-agent");
        assert_eq!(env.model_used, "mimo-v2.5");
        assert!(env.output_summary.contains("responsive"));
        assert_eq!(env.changed_files.len(), 3);
        assert!(env.changed_files.contains(&"src/components/Widget.tsx".to_string()));
        assert!(env.handoff_instruction.contains("auth flow"));
    }

    #[test]
    fn test_parse_empty_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("HANDOFF_x.md");
        fs::write(&path, "").unwrap();

        let result = parse_handoff_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_missing_sections() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("HANDOFF_x.md");
        fs::write(
            &path,
            r#"# HANDOFF
## Original Task
Do the thing.
## Completed By
x
## Model Used
mimo
## Output Summary
Done.
## Files NOT Modified
none
## Design Decisions
none
## Handoff Instructions
none
"#,
        )
        .unwrap();

        let result = parse_handoff_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Completed Work"));
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let path = Path::new("/nonexistent/HANDOFF_x.md");
        let result = parse_handoff_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_handoff_file() {
        assert!(is_handoff_file(Path::new("HANDOFF_frontend.md")));
        assert!(is_handoff_file(Path::new("/tmp/HANDOFF_x.md")));
        assert!(!is_handoff_file(Path::new("README.md")));
        assert!(!is_handoff_file(Path::new("handoff_x.md"))); // lowercase
        assert!(!is_handoff_file(Path::new("HANDOFF_x.txt"))); // wrong ext
        assert!(!is_handoff_file(Path::new("OTHER_x.md")));
    }

    #[test]
    fn test_agent_ref_from_filename() {
        assert_eq!(
            agent_ref_from_filename(Path::new("HANDOFF_frontend.md")),
            Some("frontend".to_string())
        );
        assert_eq!(
            agent_ref_from_filename(Path::new("/tmp/HANDOFF_agent-7.md")),
            Some("agent-7".to_string())
        );
        assert_eq!(agent_ref_from_filename(Path::new("README.md")), None);
        assert_eq!(agent_ref_from_filename(Path::new("HANDOFF_x.txt")), None);
    }

    #[test]
    fn test_extract_section_multiline() {
        let content = "## Original Task\nLine 1\nLine 2\nLine 3\n## Other\n";
        let section = extract_section(content, "Original Task").unwrap();
        assert_eq!(section, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_extract_list_filters_non_bullets() {
        let content = "## Files Changed\n- file1.ts\n- file2.ts\nrandom line\n- file3.ts\n## Next\n";
        let list = extract_list(content, "Files Changed");
        assert_eq!(list, vec!["file1.ts", "file2.ts", "file3.ts"]);
    }

    #[test]
    fn test_extract_list_missing_header() {
        let content = "## Other\n- not-in-files-changed\n";
        let list = extract_list(content, "Files Changed");
        assert!(list.is_empty());
    }
}

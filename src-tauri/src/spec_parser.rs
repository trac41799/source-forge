// src-tauri/src/spec_parser.rs
//
// Parse a markdown gap-closure plan into a Vec<SpecTask>.
// Used to seed a wave plan from docs/GAP_CLOSURE_PLAN.md (or similar).
//
// Strategy: parse the document section by section, look for `### Step X.Y`
// or `#### Phase N` headers, and treat each as one task. Extract a short
// title, the task description, and any file allow-list from the body.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecTask {
    pub phase: String,    // "1" | "2" | "3" | "4"
    pub step: String,     // "1.1" | "1.2" | ...
    pub title: String,
    pub objective: String,    // 1-2 line summary
    pub files_create: Vec<String>,
    pub files_modify: Vec<String>,
    pub wave: i64,            // 1 for security, 2 for UX, 3 for docs
    pub depends_on: Option<String>,
    /// If true, this step includes a deployment verification check
    /// after all agents in the wave complete. Runs build check + SPA
    /// routing check + smoke tests.
    pub verify_deploy: bool,
}

/// Parse the GAP_CLOSURE_PLAN.md into structured tasks.
/// Returns a Vec<SpecTask>, one per `#### Step X.Y` or `### Step X.Y` section.
pub fn parse_gap_closure_plan(markdown: &str) -> Vec<SpecTask> {
    let mut tasks = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();

    let mut current_phase: Option<String> = None;
    let mut current_step: Option<(String, String)> = None; // (step_id, title)
    let mut current_body: Vec<String> = Vec::new();

    for line in lines.iter() {
        let trimmed = line.trim_start_matches(' ');

        // Detect phase header: `### Phase 1: Security` (or `## Phase 1` with colon stripped)
        if trimmed.starts_with("### Phase ") || trimmed.starts_with("## Phase ") {
            let rest = trimmed.trim_start_matches('#').trim_start_matches(' ').trim();
            // "Phase 1: Security (Highest Priority)"
            if let Some(after_phase) = rest.strip_prefix("Phase ") {
                let phase_id = after_phase
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>();
                current_phase = Some(phase_id);
            }
            continue;
        }

        // Detect step header: `#### Step 1.1: ErrorBoundary Component (RED)`
        if trimmed.starts_with("#### Step ") || trimmed.starts_with("### Step ") {
            // Flush previous step
            if let (Some(phase), Some((step_id, title))) = (&current_phase, &current_step) {
                if let Some(task) = build_task(phase, step_id, title, &current_body) {
                    tasks.push(task);
                }
            }

            // Parse new step
            let rest = trimmed
                .trim_start_matches('#')
                .trim_start_matches(' ')
                .trim()
                .strip_prefix("Step ")
                .unwrap_or("");
            // "1.1: ErrorBoundary Component (RED)"
            let mut parts = rest.splitn(2, ':');
            let step_id = parts.next().unwrap_or("").trim().to_string();
            let title = parts.next().unwrap_or("").trim().to_string();
            current_step = Some((step_id, title));
            current_body.clear();
        } else if current_step.is_some() {
            // Accumulate body lines
            current_body.push(line.to_string());
        }
    }

    // Flush final step
    if let (Some(phase), Some((step_id, title))) = (&current_phase, &current_step) {
        if let Some(task) = build_task(phase, step_id, title, &current_body) {
            tasks.push(task);
        }
    }

    tasks
}

fn build_task(phase: &str, step_id: &str, title: &str, body: &[String]) -> Option<SpecTask> {
    let body_text = body.join("\n");

    // Wave: 1=Security, 2=UX, 3=Docs
    let wave = match phase {
        "1" => 1,
        "2" => 2,
        "3" => 3,
        _ => 99, // Validation etc.
    };

    // Files to create: look for `**Create:** \`path\`` or `**Create:** file`
    let files_create = extract_paths_after(&body_text, &["**Create:**", "**Write failing test first:**", "**Create file:**", "**Create or modify:**"]);

    // Files to modify: look for `**Modify:** \`path\`` etc.
    let files_modify = extract_paths_after(&body_text, &["**Modify:**", "**Write failing test first:**", "**Modify:**"]);

    // Objective: first non-empty line of body, or first `**Acceptance Criteria:**` block stripped
    let objective = body
        .iter()
        .find(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with('#')
                && !t.starts_with("**")
                && !t.starts_with("-")
                && !t.starts_with("```")
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Depends-on: any "1.1" or "1.2" reference in the body
    let depends_on = if body_text.contains("1.1") && step_id != "1.1" {
        Some("1.1".to_string())
    } else if body_text.contains("1.2") && step_id != "1.2" && !body_text.contains("1.1") {
        Some("1.2".to_string())
    } else {
        None
    };

    let verify_deploy = phase == "5" || title.to_lowercase().contains("deploy") || title.to_lowercase().contains("verify");

    Some(SpecTask {
        phase: phase.to_string(),
        step: step_id.to_string(),
        title: title.to_string(),
        objective,
        files_create,
        files_modify,
        wave,
        depends_on,
        verify_deploy,
    })
}

fn extract_paths_after(text: &str, markers: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for marker in markers {
        if let Some(idx) = text.find(marker) {
            // Take next 3 lines and extract anything that looks like a file path
            let tail: String = text[idx..]
                .lines()
                .take(5)
                .collect::<Vec<_>>()
                .join("\n");
            for token in tail.split_whitespace() {
                let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '/' && c != '-');
                if (clean.contains('/') || clean.contains(".tsx") || clean.contains(".ts") || clean.contains(".md") || clean.contains(".json"))
                    && !out.contains(&clean.to_string())
                {
                    out.push(clean.to_string());
                }
            }
        }
    }
    out
}

/// Create a markdown spec preview of the parsed tasks.
#[allow(dead_code)]
pub fn tasks_to_markdown(tasks: &[SpecTask]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Parsed Spec: {} tasks\n\n", tasks.len()));
    for t in tasks {
        out.push_str(&format!("## Phase {} / Step {}: {}\n", t.phase, t.step, t.title));
        if !t.objective.is_empty() {
            out.push_str(&format!("**Objective:** {}\n", t.objective));
        }
        if !t.files_create.is_empty() {
            out.push_str(&format!("**Files to create:** {}\n", t.files_create.join(", ")));
        }
        if !t.files_modify.is_empty() {
            out.push_str(&format!("**Files to modify:** {}\n", t.files_modify.join(", ")));
        }
        if let Some(dep) = &t.depends_on {
            out.push_str(&format!("**Depends on:** {}\n", dep));
        }
        out.push_str(&format!("**Wave:** {}\n\n", t.wave));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# Gap-Closure Plan

## Phase 1: Security

### Step 1.1: ErrorBoundary Component (RED)
Write failing test first: `src/__tests__/components/ErrorBoundary.test.tsx`
Acceptance: 5 test cases

### Step 1.2: ErrorBoundary Implementation (GREEN)
**Create:** `src/components/ErrorBoundary.tsx`
Acceptance: All 5 tests pass

### Step 1.3: Wrap All 15 Pages
**Modify:** `src/App.tsx` (lines 58-95)

## Phase 2: UX

### Step 2.1: Runner Page States
**Modify:** `src/pages/Runner.tsx`
"#;

    #[test]
    fn test_parse_returns_4_tasks() {
        let tasks = parse_gap_closure_plan(SAMPLE);
        assert_eq!(tasks.len(), 4, "expected 4 tasks, got {}", tasks.len());
    }

    #[test]
    fn test_parse_phase_assignment() {
        let tasks = parse_gap_closure_plan(SAMPLE);
        assert_eq!(tasks[0].phase, "1");
        assert_eq!(tasks[0].step, "1.1");
        assert_eq!(tasks[1].phase, "1");
        assert_eq!(tasks[2].phase, "1");
        assert_eq!(tasks[3].phase, "2");
    }

    #[test]
    fn test_parse_wave_number() {
        let tasks = parse_gap_closure_plan(SAMPLE);
        assert_eq!(tasks[0].wave, 1);
        assert_eq!(tasks[3].wave, 2);
    }

    #[test]
    fn test_parse_extracts_files_modify() {
        let tasks = parse_gap_closure_plan(SAMPLE);
        let t = &tasks[2];
        assert!(
            t.files_modify.iter().any(|f| f.contains("App.tsx")),
            "expected App.tsx in files_modify, got {:?}",
            t.files_modify
        );
    }

    #[test]
    fn test_parse_extracts_files_create() {
        let tasks = parse_gap_closure_plan(SAMPLE);
        let t = &tasks[1];
        assert!(
            t.files_create.iter().any(|f| f.contains("ErrorBoundary.tsx")),
            "expected ErrorBoundary.tsx in files_create, got {:?}",
            t.files_create
        );
    }

    #[test]
    fn test_parse_empty() {
        let tasks = parse_gap_closure_plan("");
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_no_phases() {
        let md = "## Some Other Section\n### Step 1.1: A Task\nbody";
        let tasks = parse_gap_closure_plan(md);
        // No phases parsed, so no tasks built
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_tasks_to_markdown() {
        let tasks = vec![SpecTask {
            phase: "1".to_string(),
            step: "1.1".to_string(),
            title: "Test".to_string(),
            objective: "Test obj".to_string(),
            files_create: vec!["foo.tsx".to_string()],
            files_modify: vec!["bar.ts".to_string()],
            wave: 1,
            depends_on: None,
        }];
        let md = tasks_to_markdown(&tasks);
        assert!(md.contains("Phase 1"));
        assert!(md.contains("Step 1.1"));
        assert!(md.contains("Test obj"));
        assert!(md.contains("foo.tsx"));
    }
}

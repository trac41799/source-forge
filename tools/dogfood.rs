// Self-dogfooding experiment entry point.
// Uses the orchestrator's primitive operations (worktree creation, guideline
// generation, handoff schema) to run 5 tasks from GAP_CLOSURE_PLAN.md in
// parallel worktrees. I (opencode via this session) act as the "agent" in
// each worktree.
//
// This script does NOT spawn external agent CLIs. It uses the orchestrator
// code we already built (T1+T3+T4) directly: create worktree, write
// guideline, simulate handoff, finalize report.
//
// Usage: cargo run --manifest-path tools/Cargo.toml -- <base_repo> <spec_path>

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Agent probe mode (Wave D Step 4.4):
    //   dogfood --agent "<command with args>" <fixture_repo>
    // Spawns a real (or stub) agent, waits for a valid HANDOFF_*.md, exits 0/1.
    if args.get(1).map(|a| a.as_str()) == Some("--agent") {
        let agent_cmd = args.get(2).cloned().unwrap_or_default();
        let fixture = args.get(3).cloned().unwrap_or_else(|| ".".to_string());
        std::process::exit(run_agent_probe(&agent_cmd, &fixture));
    }

    let base_repo = args.get(1).cloned().unwrap_or_else(|| ".".to_string());
    let spec_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "docs/GAP_CLOSURE_PLAN.md".to_string());

    println!("[dogfood] base_repo: {}", base_repo);
    println!("[dogfood] spec_path: {}", spec_path);

    // 1. Parse spec (inline minimal parser — the full one is in spec_parser.rs)
    let content = fs::read_to_string(&spec_path).expect("Failed to read spec");
    let tasks = parse_minimal(&content);
    println!("[dogfood] parsed {} tasks from spec", tasks.len());

    // 2. Pick 5 representative tasks
    let target_steps: HashSet<&str> = ["1.1", "2.1", "1.4", "3.1"]
        .iter()
        .copied()
        .collect();
    let scope: Vec<&Task> = tasks.iter().filter(|t| target_steps.contains(t.step.as_str())).collect();

    println!("[dogfood] selected {} tasks for wave", scope.len());
    for t in &scope {
        println!(
            "  - {}-{}: {}",
            t.phase, t.step, t.title
        );
    }

    // 3. For each task: create worktree + write guideline
    let mut report: Vec<(String, String, String, String)> = Vec::new();
    for task in &scope {
        println!("\n[dogfood] === TASK {}-{}: {} ===", task.phase, task.step, task.title);

        let worktree_path = format!(".worktrees/dogfood-{}-{}", task.phase, task.step);
        let branch = format!("dogfood/{}-{}", task.phase, task.step);

        // 3a. Create worktree
        let status = Command::new("git")
            .args([
                "-C", &base_repo,
                "worktree", "add",
                "-b", &branch,
                &worktree_path,
                "main",
            ])
            .status();
        match status {
            Ok(s) if s.success() => println!("[dogfood]   worktree created: {}", worktree_path),
            Ok(s) => {
                eprintln!("[dogfood]   worktree create failed: {:?}", s);
                continue;
            }
            Err(e) => {
                eprintln!("[dogfood]   worktree create error: {}", e);
                continue;
            }
        }

        // 3b. Write guideline
        let guideline_dir = format!("{}/.acc", worktree_path);
        fs::create_dir_all(&guideline_dir).expect("create .acc dir");
        let guideline = format!(
            "# AGENT {}-{} GUIDELINE\n\n\
             ## Objective\n{}\n\n\
             ## Task\nImplement step {} of the GAP_CLOSURE_PLAN.\n\n\
             ## Files to Create\n{}\n\n\
             ## Files to Modify\n{}\n\n\
             ## Files NOT to Touch\n- package.json\n- src-tauri/Cargo.toml\n\n\
             ## Handoff\n\
             Write HANDOFF_{}-{}.md when done.\n",
            task.phase, task.step,
            if task.objective.is_empty() { "(see plan)".to_string() } else { task.objective.clone() },
            task.step,
            if task.files_create.is_empty() { "(none)".to_string() } else { task.files_create.join("\n- ") },
            if task.files_modify.is_empty() { "(none)".to_string() } else { task.files_modify.join("\n- ") },
            task.phase, task.step,
        );
        let guideline_path = format!("{}/GUIDELINE.md", guideline_dir);
        fs::write(&guideline_path, &guideline).expect("write guideline");
        println!("[dogfood]   guideline: {}", guideline_path);

        // 3c. Write a stub handoff
        let handoff_path = format!("{}/HANDOFF_{}-{}.md", worktree_path, task.phase, task.step);
        let handoff = format!(
            "# HANDOFF {}-{}\n\n\
             ## Original Task\nImplement step {} of GAP_CLOSURE_PLAN.\n\n\
             ## Completed By\ndogfood-runner\n\n\
             ## Model Used\nmimo-v2.5 (planned) / opencode/big-pickle (actual)\n\n\
             ## Output Summary\nWorktree {}, guideline written. Awaiting actual implementation.\n\n\
             ## Files Changed\n- {}\n\n\
             ## Files NOT Modified\n- package.json\n- src-tauri/Cargo.toml\n\n\
             ## Design Decisions\nUsed orchestrator primitives to set up worktree isolation.\n\n\
             ## Interface Contracts Exposed\n- (none)\n\n\
             ## Handoff Instructions\nImplement per the guideline in .acc/GUIDELINE.md.\n",
            task.phase, task.step, task.step, worktree_path, guideline_path
        );
        fs::write(&handoff_path, &handoff).expect("write handoff");
        println!("[dogfood]   handoff: {}", handoff_path);

        report.push((
            format!("{}-{}", task.phase, task.step),
            task.title.clone(),
            worktree_path,
            handoff_path,
        ));
    }

    // 4. Final report
    println!("\n[dogfood] === WAVE REPORT ===");
    println!("Total tasks: {}", report.len());
    for (id, title, wt, ho) in &report {
        println!("  [done] {} '{}'\n         wt: {}\n         handoff: {}", id, title, wt, ho);
    }

    println!("\n[dogfood] All worktrees created. Next step: human agent implements each task in its worktree, then calls finalize_wave.");

    // 5. Cleanup hint
    println!("\n[dogfood] To clean up:");
    for (_, _, wt, _) in &report {
        println!("  git worktree remove --force {}", wt);
    }
    println!("  git worktree prune");
    println!("  git branch -D $(git branch | grep 'dogfood/')");
}

#[derive(Debug, Clone)]
struct Task {
    phase: String,
    step: String,
    title: String,
    objective: String,
    files_create: Vec<String>,
    files_modify: Vec<String>,
}

fn parse_minimal(markdown: &str) -> Vec<Task> {
    let mut tasks = Vec::new();
    let mut current_phase = String::new();
    let mut current_step: Option<(String, String)> = None;
    let mut current_body: Vec<String> = Vec::new();
    let mut current_files_create: Vec<String> = Vec::new();
    let mut current_files_modify: Vec<String> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();

        // Phase header: `## Phase 1: Security`
        if trimmed.starts_with("## Phase ") {
            let rest = trimmed.trim_start_matches('#').trim();
            if let Some(after) = rest.strip_prefix("Phase ") {
                let phase_id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                current_phase = phase_id;
            }
            continue;
        }

        // Step header: `#### Step 1.1: ErrorBoundary Component (RED)`
        if trimmed.starts_with("#### Step ") || trimmed.starts_with("### Step ") {
            // Flush previous
            if let Some((step_id, title)) = current_step.take() {
                let obj = current_body
                    .iter()
                    .find(|l| {
                        let t = l.trim();
                        !t.is_empty() && !t.starts_with('#') && !t.starts_with("**") && !t.starts_with("-")
                    })
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                tasks.push(Task {
                    phase: current_phase.clone(),
                    step: step_id,
                    title,
                    objective: obj,
                    files_create: current_files_create.clone(),
                    files_modify: current_files_modify.clone(),
                });
                current_body.clear();
                current_files_create.clear();
                current_files_modify.clear();
            }

            // Parse new step
            let rest = trimmed
                .trim_start_matches('#')
                .trim_start_matches(' ')
                .trim()
                .strip_prefix("Step ")
                .unwrap_or("");
            let mut parts = rest.splitn(2, ':');
            let step_id = parts.next().unwrap_or("").trim().to_string();
            let title = parts.next().unwrap_or("").trim().to_string();
            current_step = Some((step_id, title));
        } else if current_step.is_some() {
            current_body.push(line.to_string());
            // Detect **Create:** and **Modify:** markers
            if line.contains("**Create:**") || line.contains("**Create file:**") {
                if let Some(idx) = line.find("**Create") {
                    let tail: String = line[idx..]
                        .chars()
                        .skip_while(|c| *c != ':')
                        .skip(1)
                        .take_while(|c| *c != '\n')
                        .collect();
                    for token in tail.split_whitespace() {
                        let clean: String = token
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '/' || *c == '_' || *c == '-')
                            .collect();
                        if clean.contains('.') || clean.contains('/') {
                            current_files_create.push(clean);
                        }
                    }
                }
            }
            if line.contains("**Modify:**") {
                if let Some(idx) = line.find("**Modify") {
                    let tail: String = line[idx..]
                        .chars()
                        .skip_while(|c| *c != ':')
                        .skip(1)
                        .take_while(|c| *c != '\n')
                        .collect();
                    for token in tail.split_whitespace() {
                        let clean: String = token
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '/' || *c == '_' || *c == '-')
                            .collect();
                        if clean.contains('.') || clean.contains('/') {
                            current_files_modify.push(clean);
                        }
                    }
                }
            }
        }
    }

    // Flush final
    if let Some((step_id, title)) = current_step.take() {
        let obj = current_body
            .iter()
            .find(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#') && !t.starts_with("**") && !t.starts_with("-")
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        tasks.push(Task {
            phase: current_phase.clone(),
            step: step_id,
            title,
            objective: obj,
            files_create: current_files_create.clone(),
            files_modify: current_files_modify.clone(),
        });
    }

    tasks
}

// -----------------------------------------------------------------------------
// Agent probe (Wave D Step 4.4)
// -----------------------------------------------------------------------------

/// Spawn `<agent_cmd> <prompt>` in the fixture repo, then wait for a valid
/// HANDOFF_*.md. Returns process exit code (0 = valid handoff produced).
fn run_agent_probe(agent_cmd: &str, fixture: &str) -> i32 {
    println!("[dogfood] agent probe: '{}' in {}", agent_cmd, fixture);
    if agent_cmd.trim().is_empty() {
        eprintln!("[dogfood] empty agent command");
        return 2;
    }

    let parts = split_command(agent_cmd);
    if parts.is_empty() {
        eprintln!("[dogfood] empty agent command");
        return 2;
    }
    let mut cmd = Command::new(&parts[0]);
    for arg in &parts[1..] {
        cmd.arg(arg);
    }
    cmd.arg("Implement the task in .acc/GUIDELINE.md and write HANDOFF_<agent>.md when done.");
    cmd.current_dir(fixture);

    match cmd.status() {
        Ok(status) => println!("[dogfood] agent exited with {}", status),
        Err(e) => {
            eprintln!("[dogfood] failed to spawn agent: {}", e);
            return 1;
        }
    }

    let timeout_secs: u64 = std::env::var("ACC_AGENT_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        if let Some(path) = find_handoff(fixture) {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let required = [
                "Completed Work",
                "Test Results",
                "Interface Contracts Exposed",
                "Files NOT Modified",
                "Design Decisions",
                "Handoff Instructions",
            ];
            let missing: Vec<&str> = required
                .iter()
                .copied()
                .filter(|section| !content.contains(section))
                .collect();
            if missing.is_empty() {
                println!("[dogfood] valid handoff: {}", path);
                return 0;
            }
            eprintln!("[dogfood] handoff {} missing sections: {:?}", path, missing);
            return 1;
        }

        if std::time::Instant::now() >= deadline {
            eprintln!(
                "[dogfood] timeout ({}s) waiting for HANDOFF_*.md in {}",
                timeout_secs, fixture
            );
            return 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn find_handoff(dir: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("HANDOFF_") && name.ends_with(".md") {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}

/// Split a command string into program + args, respecting double quotes
/// (needed for Windows paths containing spaces).
fn split_command(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

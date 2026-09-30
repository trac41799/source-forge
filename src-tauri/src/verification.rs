// src-tauri/src/verification.rs
//
// Post-build deployment verification phase for the orchestrator pipeline.
// Closes gap: after agents build code, verify it actually works before
// marking a wave as complete.
//
// Checks performed:
//   Project:  package.json, node_modules, git remote, README, .env
//   Build:    dist/ exists, tsc passes, npm build passes
//   Deploy:   vercel.json with /api exclusion, SPA routing
//   Quality:  ErrorBoundary, loading states, API client production config
//   Security: CORS origin, JWT not hardcoded, npm audit
//   E2E:      Start server, HTTP health check, auth flow smoke test

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckStatus {
    Pass,
    Fail(String),
    Skip(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub project_path: String,
    pub passed: bool,
    pub total: usize,
    pub passed_count: usize,
    pub checks: Vec<BuildCheck>,
}

impl VerificationReport {
    pub fn new(project_path: &str) -> Self {
        Self {
            project_path: project_path.to_string(),
            passed: true,
            total: 0,
            passed_count: 0,
            checks: Vec::new(),
        }
    }

    fn add(&mut self, check: BuildCheck) {
        let is_pass = matches!(check.status, CheckStatus::Pass);
        if is_pass {
            self.passed_count += 1;
        } else if matches!(check.status, CheckStatus::Fail(_)) {
            self.passed = false;
        }
        self.total += 1;
        self.checks.push(check);
    }

    /// M3 (spec R31): append additive semantic checks without disturbing the
    /// deterministic checks already recorded.
    pub fn extend_with(&mut self, checks: Vec<BuildCheck>) {
        for check in checks {
            self.add(check);
        }
    }
}

/// M3 (spec R31): additive semantic checks via the decision layer. Returns an
/// empty vec when no backend is configured (deterministic checks are unaffected).
pub fn semantic_checks(
    db: &rusqlite::Connection,
    readme: Option<&str>,
    wave_diff: &str,
) -> Vec<BuildCheck> {
    let cfg = crate::decision::get_decision_config(db).unwrap_or_default();
    let key = std::env::var("OPENROUTER_API_KEY").ok();
    if key.is_none() && cfg.backend != "local" {
        return Vec::new();
    }
    let transport = crate::decision::UreqTransport { timeout_ms: cfg.timeout_ms };
    semantic_checks_with(&cfg, &transport, key.as_deref(), db, readme, wave_diff)
}

/// R-16: testable core — the transport is injected. Review-band results enqueue a
/// `decision_reviews` row (consistent with routing/handoff/contradiction).
pub fn semantic_checks_with(
    cfg: &crate::decision::DecisionConfig,
    transport: &dyn crate::decision::DecisionTransport,
    key: Option<&str>,
    db: &rusqlite::Connection,
    readme: Option<&str>,
    wave_diff: &str,
) -> Vec<BuildCheck> {
    use crate::decision::{policy_action, PolicyAction};
    let mut out = Vec::new();

    if let Some(rd) = readme {
        if let Ok(Some(p)) = crate::decision::judge(
            cfg,
            transport,
            key,
            &serde_json::json!(rd),
            "Does this README explain how to set up and run the project?",
            "explains",
            Some(db),
        ) {
            let action = policy_action(cfg, p);
            if action == PolicyAction::Review {
                let _ = crate::decision::enqueue_review(
                    db,
                    "verification.readme",
                    "explains",
                    "README.md",
                    p,
                    &serde_json::json!({ "confidence": p }).to_string(),
                );
            }
            let status = match action {
                PolicyAction::Apply => CheckStatus::Pass,
                PolicyAction::Review => {
                    CheckStatus::Skip(format!("needs review (confidence {p:.2})"))
                }
                PolicyAction::Skip => CheckStatus::Fail(format!(
                    "semantic confidence {p:.2} below review {:.2}",
                    cfg.review_threshold
                )),
            };
            out.push(BuildCheck {
                name: "README explains setup (semantic)".into(),
                status,
                detail: format!("confidence {p:.2}"),
            });
        }
    }

    if !wave_diff.is_empty() {
        if let Ok(Some(p)) = crate::decision::judge(
            cfg,
            transport,
            key,
            &serde_json::json!({ "diff": wave_diff }),
            "Does this diff introduce hardcoded secrets or credentials?",
            "has_secrets",
            Some(db),
        ) {
            // p = probability that secrets ARE present.
            let action = policy_action(cfg, p);
            if action == PolicyAction::Review {
                let _ = crate::decision::enqueue_review(
                    db,
                    "verification.secrets",
                    "has_secrets",
                    "diff",
                    p,
                    &serde_json::json!({ "confidence": p }).to_string(),
                );
            }
            let status = match action {
                PolicyAction::Apply => CheckStatus::Fail(format!(
                    "secret exposure confidence {p:.2}"
                )),
                PolicyAction::Review => {
                    CheckStatus::Skip(format!("secret check needs review (confidence {p:.2})"))
                }
                PolicyAction::Skip => CheckStatus::Pass,
            };
            out.push(BuildCheck {
                name: "No secrets in diff (semantic)".into(),
                status,
                detail: format!("has_secrets confidence {p:.2}"),
            });
        }
    }

    out
}

// ── Helpers ────────────────────────────────────────────────────────────

fn read_file(base: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(base.join(relative)).ok()
}

fn file_contains(base: &Path, relative: &str, needle: &str) -> bool {
    read_file(base, relative)
        .map(|c| c.contains(needle))
        .unwrap_or(false)
}

const REQUIRED_ENV_VARS: &[&str] = &[
    "JWT_SECRET",
    "DATABASE_URL",
    "PORT",
    "CLIENT_URL",
];

const REQUIRED_PACKAGE_SCRIPTS: &[&str] = &[
    "dev", "build", "start", "typecheck", "test",
];

// ── Core Verification ──────────────────────────────────────────────────

pub fn verify_project(base: &Path) -> VerificationReport {
    verify_project_with(base, VerifyMode::Lenient)
}

/// How strictly checks that need an installed toolchain are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyMode {
    /// Missing tooling → `Skip` (hermetic tests, projects without deps).
    Lenient,
    /// Missing tooling → `Fail`: "verification passed" must mean the build and
    /// runtime checks actually ran. Used by the pipeline when it installed the
    /// project's dependencies.
    RequireBuild,
}

/// Checks that must have executed for a `RequireBuild` verification to pass.
const BUILD_GATED_CHECKS: [&str; 3] = [
    "node_modules installed",
    "npm run build passes",
    "E2E runtime test",
];

pub fn verify_project_with(base: &Path, mode: VerifyMode) -> VerificationReport {
    let mut report = verify_project_lenient(base);
    if mode == VerifyMode::RequireBuild {
        let mut downgraded = false;
        for check in report.checks.iter_mut() {
            if BUILD_GATED_CHECKS.contains(&check.name.as_str()) {
                if let CheckStatus::Skip(detail) = &check.status {
                    let message = format!("required build step did not run: {detail}");
                    check.status = CheckStatus::Fail(message.clone());
                    check.detail = message;
                    downgraded = true;
                }
            }
        }
        if downgraded {
            report.passed = false;
        }
    }
    report
}

fn verify_project_lenient(base: &Path) -> VerificationReport {
    let mut report = VerificationReport::new(&base.to_string_lossy());

    // ── 1. Project structure ──────────────────────────────────────
    check_package_json(base, &mut report);
    check_node_modules(base, &mut report);
    check_git_remote(base, &mut report);
    check_readme(base, &mut report);
    check_env_file(base, &mut report);
    check_package_scripts(base, &mut report);

    // ── 2. Build output ──────────────────────────────────────────
    // Run the build first: the artifact checks below inspect its output, so
    // checking them beforehand can never pass on a fresh project.
    check_typescript(base, &mut report);
    check_npm_build(base, &mut report);
    check_build_output(base, &mut report);
    check_index_html(base, &mut report);

    // ── 3. Deployment config ─────────────────────────────────────
    check_spa_config(base, &mut report);
    check_docker_vercel_conflict(base, &mut report);

    // ── 4. Code quality ──────────────────────────────────────────
    check_error_boundary(base, &mut report);
    check_loading_states(base, &mut report);
    check_api_client_production(base, &mut report);
    check_cors_production(base, &mut report);
    check_security(base, &mut report);

    // ── 5. E2E runtime verification ─────────────────────────────
    check_e2e_runtime(base, &mut report);

    // ── 6. Database (local container or cloud) ─────────────────────
    check_database(base, &mut report);

    report
}

// ── Individual Checks ──────────────────────────────────────────────────

/// The database behind the app's DATABASE_URL: reachable, and carrying the
/// tables the Prisma schema declares. Skipped when the project has no Prisma
/// schema (nothing to check against).
fn check_database(base: &Path, report: &mut VerificationReport) {
    match crate::database::check_database(&base.to_string_lossy()) {
        Ok(None) => report.add(BuildCheck {
            name: "database reachable + schema applied".into(),
            status: CheckStatus::Skip("no prisma schema — no database to verify".into()),
            detail: String::new(),
        }),
        Ok(Some(detail)) => report.add(BuildCheck {
            name: "database reachable + schema applied".into(),
            status: CheckStatus::Pass,
            detail,
        }),
        Err(error) => report.add(BuildCheck {
            name: "database reachable + schema applied".into(),
            status: CheckStatus::Fail(error),
            detail: String::new(),
        }),
    }
}

fn check_package_json(base: &Path, report: &mut VerificationReport) {
    if base.join("package.json").exists() {
        report.add(BuildCheck {
            name: "package.json exists".into(),
            status: CheckStatus::Pass,
            detail: String::new(),
        });
    } else {
        report.add(BuildCheck {
            name: "package.json exists".into(),
            status: CheckStatus::Fail("No package.json — not a Node.js project".into()),
            detail: String::new(),
        });
    }
}

fn check_node_modules(base: &Path, report: &mut VerificationReport) {
    if base.join("node_modules").is_dir() {
        report.add(BuildCheck {
            name: "node_modules installed".into(),
            status: CheckStatus::Pass,
            detail: String::new(),
        });
    } else if base.join("package-lock.json").exists() || base.join("yarn.lock").exists() || base.join("pnpm-lock.yaml").exists() {
        report.add(BuildCheck {
            name: "node_modules installed".into(),
            status: CheckStatus::Fail("Lock file exists but node_modules/ missing — run npm install".into()),
            detail: String::new(),
        });
    } else {
        report.add(BuildCheck {
            name: "node_modules installed".into(),
            status: CheckStatus::Skip("No lock file — may not have run npm install yet".into()),
            detail: String::new(),
        });
    }
}

fn check_git_remote(base: &Path, report: &mut VerificationReport) {
    if !base.join(".git").exists() {
        report.add(BuildCheck {
            name: "git repo initialized".into(),
            status: CheckStatus::Fail("No .git directory — run git init".into()),
            detail: String::new(),
        });
        return;
    }
    let output = Command::new("git")
        .args(["-C", &base.to_string_lossy(), "remote", "get-url", "origin"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
            report.add(BuildCheck {
                name: "git remote configured".into(),
                status: CheckStatus::Pass,
                detail: url,
            });
        }
        _ => {
            report.add(BuildCheck {
                name: "git remote configured".into(),
                status: CheckStatus::Fail("No git remote 'origin' — run git remote add origin <url>".into()),
                detail: String::new(),
            });
        }
    }
}

fn check_readme(base: &Path, report: &mut VerificationReport) {
    match read_file(base, "README.md") {
        Some(content) if content.len() >= 500 => {
            let has_setup = content.contains("install") || content.contains("setup") || content.contains("npm install");
            let has_env = content.contains("env") || content.contains("JWT_SECRET") || content.contains("DATABASE_URL");
            let has_arch = content.contains("Architecture") || content.contains("Stack") || content.contains("## ");
            report.add(BuildCheck {
                name: "README complete".into(),
                status: if has_setup && has_env { CheckStatus::Pass } else { CheckStatus::Fail("README exists but missing setup instructions or env vars section".into()) },
                detail: format!("{} chars, setup={}, env={}, architecture={}", content.len(), has_setup, has_env, has_arch),
            });
        }
        Some(content) => {
            report.add(BuildCheck {
                name: "README complete".into(),
                status: CheckStatus::Fail(format!("README too short ({} chars) — need 500+ chars with setup, env vars, architecture", content.len())),
                detail: String::new(),
            });
        }
        None => {
            report.add(BuildCheck {
                name: "README complete".into(),
                status: CheckStatus::Fail("No README.md found".into()),
                detail: String::new(),
            });
        }
    }
}

fn check_env_file(base: &Path, report: &mut VerificationReport) {
    match read_file(base, ".env.example") {
        Some(content) => {
            let mut missing = Vec::new();
            for var in REQUIRED_ENV_VARS {
                if !content.contains(var) {
                    missing.push(*var);
                }
            }
            if missing.is_empty() {
                report.add(BuildCheck {
                    name: ".env.example complete".into(),
                    status: CheckStatus::Pass,
                    detail: format!("All {} required vars present", REQUIRED_ENV_VARS.len()),
                });
            } else {
                report.add(BuildCheck {
                    name: ".env.example complete".into(),
                    status: CheckStatus::Fail(format!("Missing required vars: {}", missing.join(", "))),
                    detail: String::new(),
                });
            }
        }
        None => {
            report.add(BuildCheck {
                name: ".env.example exists".into(),
                status: CheckStatus::Fail("No .env.example — create one listing all required environment variables".into()),
                detail: String::new(),
            });
        }
    }
}

fn check_package_scripts(base: &Path, report: &mut VerificationReport) {
    match read_file(base, "package.json") {
        Some(content) => {
            let mut missing = Vec::new();
            for script in REQUIRED_PACKAGE_SCRIPTS {
                if !content.contains(&format!("\"{}\"", script)) {
                    missing.push(*script);
                }
            }
            if missing.is_empty() {
                report.add(BuildCheck {
                    name: "package.json scripts".into(),
                    status: CheckStatus::Pass,
                    detail: "All required scripts present".into(),
                });
            } else {
                report.add(BuildCheck {
                    name: "package.json scripts".into(),
                    status: CheckStatus::Fail(format!("Missing scripts: {}", missing.join(", "))),
                    detail: String::new(),
                });
            }
        }
        None => {}
    }
}

fn check_build_output(base: &Path, report: &mut VerificationReport) {
    let has_dist = base.join("dist").is_dir();
    let has_build = base.join("build").is_dir();
    // Next.js emits .next/ (BUILD_ID appears once the build finished).
    let has_next = base.join(".next").join("BUILD_ID").exists();
    if has_dist || has_build || has_next {
        let dir = if has_dist { "dist/" } else if has_build { "build/" } else { ".next/" };
        report.add(BuildCheck {
            name: "build output exists".into(),
            status: CheckStatus::Pass,
            detail: format!("Found {dir}"),
        });
    } else {
        report.add(BuildCheck {
            name: "build output exists".into(),
            status: CheckStatus::Fail("No dist/ or build/ directory — run npm run build first".into()),
            detail: String::new(),
        });
    }
}

fn check_index_html(base: &Path, report: &mut VerificationReport) {
    let index = base.join("dist").join("index.html");
    if !index.exists() {
        report.add(BuildCheck {
            name: "dist/index.html valid".into(),
            status: CheckStatus::Skip("No dist/index.html".into()),
            detail: String::new(),
        });
        return;
    }
    match std::fs::read_to_string(&index) {
        Ok(content) => {
            let has_root = content.contains("root") || content.contains("app");
            let has_script = content.contains("<script");
            if has_root && has_script {
                report.add(BuildCheck {
                    name: "dist/index.html valid".into(),
                    status: CheckStatus::Pass,
                    detail: format!("{} bytes, root mount + script tag present", content.len()),
                });
            } else {
                report.add(BuildCheck {
                    name: "dist/index.html valid".into(),
                    status: CheckStatus::Fail("dist/index.html missing root mount or script tag".into()),
                    detail: String::new(),
                });
            }
        }
        Err(e) => {
            report.add(BuildCheck {
                name: "dist/index.html readable".into(),
                status: CheckStatus::Fail(format!("Cannot read dist/index.html: {e}")),
                detail: String::new(),
            });
        }
    }
}

/// `npm`/`npx` are `.cmd` shims on Windows, which `Command::new` cannot
/// execute: they must go through `cmd /C` (the same fix the agent adapter
/// needed). Without this every build/typecheck check reported
/// "program not found" and could never actually run.
pub(crate) fn npm_command(program: &str) -> std::process::Command {
    if cfg!(windows) {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/C").arg(program);
        cmd
    } else {
        std::process::Command::new(program)
    }
}

fn check_typescript(base: &Path, report: &mut VerificationReport) {
    if !base.join("tsconfig.json").exists() {
        report.add(BuildCheck {
            name: "TypeScript compiles".into(),
            status: CheckStatus::Skip("No tsconfig.json — not a TypeScript project".into()),
            detail: String::new(),
        });
        return;
    }
    let output = npm_command("npx")
        .args(["tsc", "--noEmit"])
        .current_dir(base)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            report.add(BuildCheck {
                name: "TypeScript compiles".into(),
                status: CheckStatus::Pass,
                detail: "tsc --noEmit: 0 errors".into(),
            });
        }
        Ok(out) => {
            // `tsc` writes diagnostics to stdout, not stderr. Counting only
            // stderr reported "0 errors" for a failing typecheck.
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let err_count = stdout
                .lines()
                .chain(stderr.lines())
                .filter(|line| line.contains("error TS"))
                .count();
            let mut detail: Vec<&str> = stdout
                .lines()
                .chain(stderr.lines())
                .filter(|line| line.contains("error TS"))
                .take(3)
                .collect();
            report.add(BuildCheck {
                name: "TypeScript compiles".into(),
                status: CheckStatus::Fail(format!(
                    "tsc --noEmit: {err_count} errors{}",
                    if detail.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", detail.drain(..).collect::<Vec<_>>().join(" | "))
                    }
                )),
                detail: String::new(),
            });
        }
        Err(e) => {
            report.add(BuildCheck {
                name: "TypeScript compiles".into(),
                status: CheckStatus::Fail(format!("Cannot run tsc: {e}")),
                detail: String::new(),
            });
        }
    }
}

fn check_npm_build(base: &Path, report: &mut VerificationReport) {
    if !base.join("package.json").exists() {
        return;
    }
    if !base.join("node_modules").exists() {
        report.add(BuildCheck {
            name: "npm run build passes".into(),
            status: CheckStatus::Skip(
                "node_modules not installed — run npm install before build verification".into(),
            ),
            detail: String::new(),
        });
        return;
    }
    let output = npm_command("npm")
        .args(["run", "build"])
        .current_dir(base)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            report.add(BuildCheck {
                name: "npm run build passes".into(),
                status: CheckStatus::Pass,
                detail: "Build succeeded".into(),
            });
        }
        Ok(out) => {
            // A failed build with no output tail is undiagnosable.
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            let tail: Vec<&str> = text.lines().rev().take(6).collect();
            let tail = tail.into_iter().rev().collect::<Vec<_>>().join(" | ");
            report.add(BuildCheck {
                name: "npm run build passes".into(),
                status: CheckStatus::Fail(format!("npm run build failed: {tail}")),
                detail: String::new(),
            });
        }
        Err(e) => {
            report.add(BuildCheck {
                name: "npm run build passes".into(),
                status: CheckStatus::Fail(format!("Cannot run npm build: {e}")),
                detail: String::new(),
            });
        }
    }
}

fn check_spa_config(base: &Path, report: &mut VerificationReport) {
    let vercel = base.join("vercel.json");
    let redirects = base.join("public").join("_redirects");
    let netlify = base.join("netlify.toml");
    let is_spa = base.join("index.html").exists() || base.join("dist").is_dir();

    if !is_spa {
        report.add(BuildCheck {
            name: "SPA routing config".into(),
            status: CheckStatus::Skip("Not a SPA project".into()),
            detail: String::new(),
        });
        return;
    }

    if !vercel.exists() && !redirects.exists() && !netlify.exists() {
        report.add(BuildCheck {
            name: "SPA routing config".into(),
            status: CheckStatus::Fail(
                "Missing SPA routing config. Add vercel.json with rewrite rules.".into(),
            ),
            detail: r#"Create vercel.json: {"rewrites":[{"source":"/((?!api/).*)","destination":"/index.html"}]}"#.into(),
        });
        return;
    }

    if vercel.exists() {
        match read_file(base, "vercel.json") {
            Some(content) => {
                let catches_all = content.contains("\"/(.*)\"") && !content.contains("api");
                if catches_all {
                    report.add(BuildCheck {
                        name: "SPA rewrite excludes /api".into(),
                        status: CheckStatus::Fail(
                            "vercel.json rewrite catches /api/* — API POST requests will return 405".into(),
                        ),
                        detail: "Change source regex to \"/((?!api/).*)\"".into(),
                    });
                } else {
                    report.add(BuildCheck {
                        name: "SPA rewrite excludes /api".into(),
                        status: CheckStatus::Pass,
                        detail: "Rewrite correctly excludes /api paths".into(),
                    });
                }
            }
            None => {}
        }
    }
}

fn check_docker_vercel_conflict(base: &Path, report: &mut VerificationReport) {
    let has_dockerfile = base.join("Dockerfile").exists();
    let has_vercel = base.join("vercel.json").exists() || base.join(".vercel").is_dir();
    let has_vercelignore = base.join(".vercelignore").exists();

    if has_dockerfile && has_vercel && !has_vercelignore {
        report.add(BuildCheck {
            name: "Docker+Vercel no conflict".into(),
            status: CheckStatus::Fail(
                "Dockerfile present with Vercel config but no .vercelignore — Docker may override Vite framework detection, breaking SPA routing. Add .vercelignore to exclude Docker files.".into(),
            ),
            detail: "Create .vercelignore containing: Dockerfile\ndocker-compose.yml\n.dockerignore".into(),
        });
    } else if has_dockerfile && has_vercel && has_vercelignore {
        report.add(BuildCheck {
            name: "Docker+Vercel no conflict".into(),
            status: CheckStatus::Pass,
            detail: ".vercelignore present — Docker files excluded from Vercel deploy".into(),
        });
    } else {
        report.add(BuildCheck {
            name: "Docker+Vercel no conflict".into(),
            status: CheckStatus::Skip("No Docker+Vercel conflict risk".into()),
            detail: String::new(),
        });
    }
}

fn check_error_boundary(base: &Path, report: &mut VerificationReport) {
    let app_paths = ["src/App.tsx", "src/App.jsx", "src/main.tsx", "src/index.tsx"];
    let mut found = false;
    for p in &app_paths {
        if file_contains(base, p, "ErrorBoundary") || file_contains(base, p, "error-boundary") {
            found = true;
            break;
        }
    }
    if found {
        report.add(BuildCheck {
            name: "Error boundary present".into(),
            status: CheckStatus::Pass,
            detail: String::new(),
        });
    } else if base.join("src/App.tsx").exists() || base.join("src/main.tsx").exists() {
        report.add(BuildCheck {
            name: "Error boundary present".into(),
            status: CheckStatus::Fail("No ErrorBoundary in App.tsx or main.tsx — uncaught React errors will crash the app with a blank page".into()),
            detail: "Wrap routes in an ErrorBoundary component".into(),
        });
    } else {
        report.add(BuildCheck {
            name: "Error boundary present".into(),
            status: CheckStatus::Skip("No React entry point found".into()),
            detail: String::new(),
        });
    }
}

fn check_loading_states(base: &Path, report: &mut VerificationReport) {
    let pages_dir = base.join("src").join("pages");
    if !pages_dir.is_dir() {
        report.add(BuildCheck {
            name: "Loading states in pages".into(),
            status: CheckStatus::Skip("No src/pages directory".into()),
            detail: String::new(),
        });
        return;
    }
    let mut pages_without_loading = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&pages_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "tsx" || e == "jsx") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let has_loading = content.to_lowercase().contains("loading") || content.contains("isLoading");
                    let has_error = content.to_lowercase().contains("error") || content.contains("setError");
                    if !has_loading && !has_error {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            pages_without_loading.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    if pages_without_loading.is_empty() {
        report.add(BuildCheck {
            name: "Loading states in pages".into(),
            status: CheckStatus::Pass,
            detail: "All pages have loading or error state handling".into(),
        });
    } else {
        report.add(BuildCheck {
            name: "Loading states in pages".into(),
            status: CheckStatus::Fail(format!(
                "Pages missing loading/error states: {}. Add isLoading/setError patterns.",
                pages_without_loading.join(", ")
            )),
            detail: String::new(),
        });
    }
}

fn check_api_client_production(base: &Path, report: &mut VerificationReport) {
    let client_paths = ["src/api/client.ts", "src/api/client.tsx", "src/utils/api.ts", "src/lib/api.ts"];
    let mut found_client = false;
    for p in &client_paths {
        if let Some(content) = read_file(base, p) {
            found_client = true;
            let has_prod_override = content.contains("VITE_API_URL")
                || content.contains("import.meta.env")
                || content.contains("process.env")
                || content.contains("API_URL");
            if has_prod_override {
                report.add(BuildCheck {
                    name: "API client production config".into(),
                    status: CheckStatus::Pass,
                    detail: "Has production API URL override".into(),
                });
            } else {
                report.add(BuildCheck {
                    name: "API client production config".into(),
                    status: CheckStatus::Fail("API client always uses relative /api path — no production backend URL override. Add VITE_API_URL env var support.".into()),
                    detail: "Use: baseURL: import.meta.env.VITE_API_URL || '/api'".into(),
                });
            }
            break;
        }
    }
    if !found_client {
        report.add(BuildCheck {
            name: "API client production config".into(),
            status: CheckStatus::Skip("No API client file found in standard locations".into()),
            detail: String::new(),
        });
    }
}

fn check_cors_production(base: &Path, report: &mut VerificationReport) {
    let server_files = ["src/server.ts", "src/index.ts", "src/app.ts", "server.ts"];
    for sf in &server_files {
        if let Some(content) = read_file(base, sf) {
            let has_localhost = content.contains("localhost:5173")
                || content.contains("localhost:3000");
            if has_localhost {
                report.add(BuildCheck {
                    name: "CORS origin production-safe".into(),
                    status: CheckStatus::Fail(format!(
                        "{} contains hardcoded localhost CORS origin. Use CLIENT_URL env var in production.",
                        sf
                    )),
                    detail: "Use: origin: process.env.CLIENT_URL || 'http://localhost:5173'".into(),
                });
            } else {
                report.add(BuildCheck {
                    name: "CORS origin production-safe".into(),
                    status: CheckStatus::Pass,
                    detail: format!("{} uses env var for CORS origin", sf),
                });
            }
            return;
        }
    }
    report.add(BuildCheck {
        name: "CORS origin production-safe".into(),
        status: CheckStatus::Skip("No server file found".into()),
        detail: String::new(),
    });
}

fn check_security(base: &Path, report: &mut VerificationReport) {
    let sensitive_files = ["src/config.ts", "src/config.js", ".env.example", ".env"];
    let mut found_hardcoded = false;
    for sf in &sensitive_files {
        if let Some(content) = read_file(base, sf) {
            if content.contains("JWT_SECRET") && content.contains("= \"") && !content.contains("process.env") {
                found_hardcoded = true;
                break;
            }
        }
    }
    if found_hardcoded {
        report.add(BuildCheck {
            name: "Secrets in env vars".into(),
            status: CheckStatus::Fail("Hardcoded JWT_SECRET or other secret found in source — use env vars only".into()),
            detail: String::new(),
        });
    } else {
        report.add(BuildCheck {
            name: "Secrets in env vars".into(),
            status: CheckStatus::Pass,
            detail: "No hardcoded secrets detected".into(),
        });
    }
}

fn server_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("sourceforge-e2e-{}.log", std::process::id()))
}

/// Server stdout/stderr → log file (never a pipe nobody drains). Falls back to
/// the null device if the temp file cannot be opened.
fn server_log_stdio() -> Stdio {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(server_log_path())
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null())
}

fn server_log_tail() -> String {
    let Ok(text) = std::fs::read_to_string(server_log_path()) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().rev().take(6).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

/// Resolve the `next` entry that Node can actually execute.
///
/// `node_modules/.bin/next` is a POSIX shell script, which on Windows makes
/// Node die with `SyntaxError: missing ) after argument list` on its second
/// line (`basedir=$(dirname ...)`). The real JS entry works on every platform,
/// so it is preferred; the `.bin` shim is only a fallback.
fn resolve_next_bin(base: &Path) -> std::path::PathBuf {
    let dist = base
        .join("node_modules")
        .join("next")
        .join("dist")
        .join("bin")
        .join("next");
    if dist.exists() {
        dist
    } else {
        base.join("node_modules").join(".bin").join("next")
    }
}

fn check_e2e_runtime(base: &Path, report: &mut VerificationReport) {
    let is_nextjs = base.join("next.config.ts").exists() || base.join("next.config.js").exists();
    let is_express = base.join("src").join("server.ts").exists();
    let pkg = base.join("package.json");

    if !is_nextjs && !is_express && !pkg.exists() {
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: CheckStatus::Skip("No server entry point found".into()),
            detail: String::new(),
        });
        return;
    }

    if !base.join("node_modules").exists() {
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: CheckStatus::Skip("node_modules not installed — cannot start server".into()),
            detail: String::new(),
        });
        return;
    }

    let port: u16 = 4199;
    let node_cmd = find_node_command();
    let mut child: Option<Child> = None;

    if is_nextjs {
        let next_bin = resolve_next_bin(base);
        if !next_bin.exists() {
            report.add(BuildCheck {
                name: "E2E runtime test".into(),
                status: CheckStatus::Fail("next binary not found in node_modules".into()),
                detail: String::new(),
            });
            return;
        }
        child = Command::new(&node_cmd)
            .arg(&next_bin)
            .arg("dev")
            .arg("--port").arg(port.to_string())
            .env("JWT_SECRET", "e2e-test-secret")
            .env("DATABASE_URL", "postgresql://none:none@localhost:5432/none")
            .env("NEXT_PUBLIC_SUPABASE_URL", "https://placeholder.supabase.co")
            .env("NEXT_PUBLIC_SUPABASE_ANON_KEY", "placeholder")
            .current_dir(base)
            .stdout(server_log_stdio())
            .stderr(server_log_stdio())
            .spawn()
            .ok();
    } else if is_express {
        let tsx = base.join("node_modules").join("tsx").join("dist").join("cli.mjs");
        if tsx.exists() {
            child = Command::new(&node_cmd)
                .arg(&tsx).arg("src/server.ts")
                .env("PORT", port.to_string())
                .env("JWT_SECRET", "e2e-test-secret")
                .env("JWT_REFRESH_SECRET", "e2e-test-refresh")
                .env("DATABASE_URL", "postgresql://none:none@localhost:5432/none")
                .env("NODE_ENV", "test")
                .current_dir(base)
                .stdout(server_log_stdio()).stderr(server_log_stdio())
                .spawn().ok();
        }
    }

    let mut server = match child {
        Some(c) => c,
        None => {
            report.add(BuildCheck {
                name: "E2E runtime test".into(),
                status: CheckStatus::Fail("Cannot start dev server".into()),
                detail: String::new(),
            });
            return;
        }
    };

    // Wait for boot
    let mut booted = false;
    for _ in 0..120 {
        // Next dev cold-compiles on first request: allow up to 60s.
        std::thread::sleep(Duration::from_millis(500));
        // Probe a route any app has; `/api/auth/*` is template-specific.
        if http_get(port, "/api/health").is_ok() || http_get(port, "/").is_ok() {
            booted = true;
            break;
        }
    }

    if !booted {
        let _ = server.kill();
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: CheckStatus::Fail(format!(
                "Server failed to boot within 60 seconds: {}",
                server_log_tail()
            )),
            detail: String::new(),
        });
        return;
    }

    // The auth-flow probes below belong to the reference template. An app
    // without auth routes would be failed for endpoints it never had, so the
    // runtime check verifies what the app does expose: that it serves traffic.
    let has_auth_routes = base.join("app").join("api").join("auth").exists()
        || base.join("src").join("routes").join("auth").exists();
    if !has_auth_routes {
        let responds = http_get(port, "/api/health").is_ok() || http_get(port, "/").is_ok();
        let _ = server.kill();
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: if responds {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail("server booted but no route responded".into())
            },
            detail: if responds {
                "Server booted and served a response (no auth routes in this app — auth-flow probes not applicable)".into()
            } else {
                String::new()
            },
        });
        return;
    }

    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Health or root
    if is_nextjs {
        if http_get(port, "/").is_ok() { passed += 1; } else { failed += 1; }
    } else {
        if http_get(port, "/api/health") == Ok(200) { passed += 1; } else { failed += 1; }
    }

    // Test 2: Auth middleware (no cookie → 401)
    if http_get(port, "/api/auth/me") == Ok(401) { passed += 1; } else { failed += 1; }

    // Test 3: Validation (bad register → 400)
    if http_post_json(port, "/api/auth/register", r#"{"email":"bad","password":"short","name":"x"}"#) == Ok(400) { passed += 1; } else { failed += 1; }

    // Test 4: Register with valid data → 201 + extract Set-Cookie
    let email = format!("e2e-{}@test.local", std::process::id());
    let reg_body = format!(r#"{{"email":"{}","password":"Test1234!","name":"E2E"}}"#, email);
    let reg_resp = http_post_json_full(port, "/api/auth/register", &reg_body);
    let mut cookie: Option<String> = None;

    if let Ok((status, headers, _body)) = &reg_resp {
        if *status == 201 {
            // Extract Set-Cookie header
            for h in headers {
                if h.to_lowercase().starts_with("set-cookie:") {
                    if let Some(val) = h.splitn(2, ':').nth(1) {
                        if let Some(c) = val.trim().split(';').next() {
                            cookie = Some(c.to_string());
                        }
                    }
                }
            }
            passed += 1;
        } else {
            failed += 1;
        }
    } else {
        failed += 1;
    }

    // Test 5: Me with cookie → 200 (verifies cookie auth flow works)
    if let Some(ref c) = cookie {
        if http_get_with_cookie(port, "/api/auth/me", c) == Ok(200) {
            passed += 1;
        } else {
            failed += 1;
        }
    } else {
        // Cookie not set — this IS a failure if register passed but no cookie
        failed += 1;
    }

    // Test 6: 404 structured error
    if http_get(port, "/api/bogus-nonexistent") == Ok(404) { passed += 1; } else { failed += 1; }

    let _ = server.kill();

    if failed == 0 {
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: CheckStatus::Pass,
            detail: format!("Server booted, {}/6 HTTP tests passed (reg+cookie auth flow verified)", passed),
        });
    } else {
        let cookie_note = if cookie.is_none() { " (Set-Cookie header missing — cookie auth broken!)" } else { "" };
        report.add(BuildCheck {
            name: "E2E runtime test".into(),
            status: CheckStatus::Fail(format!("{}/6 tests failed{cookie_note}. Full auth flow (register → cookie → me) must pass.", failed)),
            detail: format!("{} tests passed, {} failed", passed, failed),
        });
    }
}

fn find_node_command() -> String {
    for cmd in &["node", "node.exe"] {
        if Command::new(cmd).arg("--version").output().is_ok() {
            return cmd.to_string();
        }
    }
    "node".to_string()
}

fn http_get(port: u16, path: &str) -> Result<u16, String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(2),
    )
    .map_err(|e| e.to_string())?;
    let req = format!("GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", path, port);
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let resp = String::from_utf8_lossy(&buf);
    if let Some(line) = resp.lines().next() {
        if let Some(code) = line.split_whitespace().nth(1) {
            return code.parse::<u16>().map_err(|e| e.to_string());
        }
    }
    Err("No status line".into())
}

fn http_post_json(port: u16, path: &str, body: &str) -> Result<u16, String> {
    http_post_json_full(port, path, body).map(|(s, _, _)| s)
}

fn http_post_json_full(port: u16, path: &str, body: &str) -> Result<(u16, Vec<String>, String), String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(3),
    )
    .map_err(|e| e.to_string())?;
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, port, body.len(), body
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let resp = String::from_utf8_lossy(&buf);
    let mut headers = Vec::new();
    let mut status = 0u16;
    let mut in_body = false;
    let mut body_lines = Vec::new();
    for line in resp.lines() {
        if !in_body {
            if line.is_empty() { in_body = true; continue; }
            if let Some(code) = line.split_whitespace().nth(1) {
                status = code.parse().unwrap_or(0);
            }
            headers.push(line.to_string());
        } else {
            body_lines.push(line.to_string());
        }
    }
    Ok((status, headers, body_lines.join("\n")))
}

fn http_get_with_cookie(port: u16, path: &str, cookie: &str) -> Result<u16, String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(3),
    )
    .map_err(|e| e.to_string())?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nCookie: {}\r\nConnection: close\r\n\r\n",
        path, port, cookie
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let resp = String::from_utf8_lossy(&buf);
    if let Some(line) = resp.lines().next() {
        if let Some(code) = line.split_whitespace().nth(1) {
            return code.parse::<u16>().map_err(|e| e.to_string());
        }
    }
    Err("No status line".into())
}

/// Generate a vercel.json with SPA rewrites that exclude /api paths.
pub fn generate_vercel_config(base: &Path) -> Result<PathBuf, String> {
    let vercel_json = base.join("vercel.json");
    if vercel_json.exists() {
        return Err("vercel.json already exists".into());
    }
    let config = r#"{"rewrites":[{"source":"/((?!api/).*)","destination":"/index.html"}]}"#;
    std::fs::write(&vercel_json, config)
        .map_err(|e| format!("Failed to write vercel.json: {e}"))?;
    Ok(vercel_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_project(name: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        let pkg = serde_json::json!({
            "name": name,
            "scripts": {
                "dev": "vite",
                "build": "vite build",
                "start": "vite preview",
                "typecheck": "tsc --noEmit",
                "test": "vitest run"
            }
        });
        std::fs::write(
            base.join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        std::fs::create_dir_all(base.join("dist")).unwrap();
        std::fs::write(
            base.join("dist").join("index.html"),
            "<html><body><div id=\"root\"></div><script src=\"/assets/index.js\"></script></body></html>",
        )
        .unwrap();

        std::fs::write(
            base.join("vercel.json"),
            r#"{"rewrites":[{"source":"/((?!api/).*)","destination":"/index.html"}]}"#,
        )
        .unwrap();

        dir
    }

    fn make_project_with_src(name: &str) -> TempDir {
        let dir = make_project(name);
        let base = dir.path();
        std::fs::create_dir_all(base.join("src").join("pages")).unwrap();
        std::fs::create_dir_all(base.join("src").join("api")).unwrap();
        std::fs::write(
            base.join("src").join("App.tsx"),
            "import { ErrorBoundary } from 'react-error-boundary';\nexport default function App() { return <div/>; }",
        )
        .unwrap();
        std::fs::write(
            base.join("src").join("pages").join("Home.tsx"),
            "export default function Home() { const [loading, setLoading] = useState(true); return <div/>; }",
        )
        .unwrap();
        std::fs::write(
            base.join("src").join("api").join("client.ts"),
            "const api = axios.create({ baseURL: import.meta.env.VITE_API_URL || '/api' });",
        )
        .unwrap();
        std::fs::create_dir_all(base.join("src").join("pages")).unwrap();

        // Fixture completeness for the "all deterministic checks pass" test:
        // README, env example, and a git repo with a remote are all required
        // by verify_project; node_modules stays absent so build/E2E are Skip.
        std::fs::write(
            base.join("README.md"),
            "# Fixture App\n\n\
             ## Setup\n\n\
             Run `npm install` to set up the project, then `npm run dev` for local development.\n\
             Build for production with `npm run build` and preview with `npm start`.\n\n\
             ## Environment Variables\n\n\
             Copy `.env.example` to `.env` and fill in the required values.\n\
             Required variables: JWT_SECRET, DATABASE_URL, PORT, CLIENT_URL.\n\n\
             ## Architecture\n\n\
             Vite + React single page application with an Express-compatible API surface.\n\
             The frontend is built with TypeScript and tested with Vitest.\n\
             Deployment targets Vercel with SPA rewrites excluding /api routes.\n\n\
             ## Testing\n\n\
             Run `npm test` for the unit suite and `npm run typecheck` for static analysis.\n\
             The verification pipeline checks build output, SPA rewrites, and runtime smoke tests.\n",
        )
        .unwrap();

        std::fs::write(
            base.join(".env.example"),
            "JWT_SECRET=change-me\nDATABASE_URL=postgres://localhost:5432/app\nPORT=3001\nCLIENT_URL=http://localhost:5173\n",
        )
        .unwrap();

        let _ = Command::new("git")
            .args(["init", "-q"])
            .current_dir(base)
            .output();
        let _ = Command::new("git")
            .args(["remote", "add", "origin", "https://example.com/fixture-app.git"])
            .current_dir(base)
            .output();

        dir
    }

    #[test]
    fn test_verify_project_all_pass() {
        let project = make_project_with_src("passing-app");
        let base = project.path();
        let report = verify_project(base);
        assert!(report.passed, "Should pass all checks, got failures: {:?}", report.checks.iter().filter(|c| matches!(c.status, CheckStatus::Fail(_))).map(|c| &c.name).collect::<Vec<_>>());
    }

    #[test]
    fn test_spa_rewrite_catches_api_should_fail() {
        let project = make_project("bad-spa");
        let base = project.path();
        std::fs::write(
            base.join("vercel.json"),
            r#"{"rewrites":[{"source":"/(.*)","destination":"/index.html"}]}"#,
        )
        .unwrap();
        let report = verify_project(base);
        let api_check = report.checks.iter().find(|c| c.name == "SPA rewrite excludes /api").unwrap();
        assert!(matches!(api_check.status, CheckStatus::Fail(_)), "Should catch /api rewrite");
    }

    #[test]
    fn test_missing_spa_config_fails() {
        let project = make_project("no-spa-config");
        let base = project.path();
        std::fs::remove_file(base.join("vercel.json")).unwrap();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == "SPA routing config").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_missing_readme_fails() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == "README complete").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_readme_too_short_fails() {
        let project = make_project("short-readme");
        let base = project.path();
        std::fs::write(base.join("README.md"), "hi").unwrap();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == "README complete").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_missing_env_example_fails() {
        let project = make_project("no-env");
        let base = project.path();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == ".env.example exists").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_api_client_no_prod_override_fails() {
        let project = make_project_with_src("no-prod-api");
        let base = project.path();
        std::fs::write(
            base.join("src").join("api").join("client.ts"),
            "const api = axios.create({ baseURL: '/api' });",
        )
        .unwrap();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == "API client production config").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_no_error_boundary_fails() {
        let project = make_project_with_src("no-error-boundary");
        let base = project.path();
        std::fs::write(
            base.join("src").join("App.tsx"),
            "export default function App() { return <div/>; }",
        )
        .unwrap();
        let report = verify_project(base);
        let check = report.checks.iter().find(|c| c.name == "Error boundary present").unwrap();
        assert!(matches!(check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_generate_vercel_config() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let result = generate_vercel_config(base);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(base.join("vercel.json")).unwrap();
        assert!(content.contains("(?!api/)"));
    }

    #[test]
    fn test_verification_report_counting() {
        let mut report = VerificationReport::new("/test");
        report.add(BuildCheck { name: "pass".into(), status: CheckStatus::Pass, detail: String::new() });
        report.add(BuildCheck { name: "fail".into(), status: CheckStatus::Fail("x".into()), detail: String::new() });
        report.add(BuildCheck { name: "skip".into(), status: CheckStatus::Skip("n/a".into()), detail: String::new() });
        assert_eq!(report.total, 3);
        assert_eq!(report.passed_count, 1);
        assert!(!report.passed);
    }

    // ── R31: the wave diff drives the semantic secrets check ────────────
    #[test]
    fn wave_diff_review_band_checks_secrets() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/019_decision_reviews_unique.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/020_decision_usage_hash.sql"))
            .unwrap();
        let cfg = crate::decision::DecisionConfig::default();
        let t = SemTransport {
            payload: serde_json::json!({
                "model": "m",
                "answers": { "has_secrets": { "type": "noul", "noul": 0.5 } },
                "usage": { "input_tokens": 1, "cost": 0.0 }
            }),
        };
        // readme = None → only the secrets check runs, and only because the
        // wave diff is non-empty (a project-tree diff must NOT be used here).
        let checks = semantic_checks_with(
            &cfg,
            &t,
            Some("k"),
            &conn,
            None,
            "diff --git a/.env b/.env\n+TOKEN=abc",
        );
        assert!(
            checks.iter().any(|c| c.name.contains("secrets")),
            "secrets check must run on a non-empty wave diff"
        );
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM decision_reviews WHERE consumer = 'verification.secrets'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "review-band secrets → enqueued");

        // With no wave diff the secrets check must not run at all.
        let none = semantic_checks_with(&cfg, &t, Some("k"), &conn, None, "");
        assert!(!none.iter().any(|c| c.name.contains("secrets")));
    }

    // ── R-16: verification review-band enqueue ─────────────────────────────
    struct SemTransport {
        payload: serde_json::Value,
    }
    impl crate::decision::DecisionTransport for SemTransport {
        fn post(
            &self,
            _url: &str,
            _key: Option<&str>,
            _body: &serde_json::Value,
        ) -> Result<serde_json::Value, crate::decision::DecisionError> {
            Ok(self.payload.clone())
        }
    }

    #[test]
    fn review_band_readme_enqueues_review() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/016_decision_usage.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/019_decision_reviews_unique.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/020_decision_usage_hash.sql"))
            .unwrap();
        let cfg = crate::decision::DecisionConfig::default();
        let t = SemTransport {
            payload: serde_json::json!({
                "model": "m",
                "answers": { "explains": { "type": "noul", "noul": 0.5 } },
                "usage": { "input_tokens": 1, "cost": 0.0 }
            }),
        };
        let checks = semantic_checks_with(&cfg, &t, Some("k"), &conn, Some("readme body"), "");
        assert!(
            checks.iter().any(|c| matches!(c.status, CheckStatus::Skip(_))),
            "review band → skip status"
        );
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM decision_reviews WHERE consumer = 'verification.readme'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "review-band README check enqueues a review");
    }


    #[test]
    fn test_require_build_turns_tooling_skips_into_failures() {
        // A Node project without installed dependencies: lenient verification
        // Skips the build/runtime checks (so "passed" could mean "never built").
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"x","scripts":{"build":"vite build"}}"#,
        )
        .unwrap();

        let gated = [
            "node_modules installed",
            "npm run build passes",
            "E2E runtime test",
        ];

        let lenient = verify_project(dir.path());
        let skipped = lenient
            .checks
            .iter()
            .filter(|check| {
                gated.contains(&check.name.as_str())
                    && matches!(check.status, CheckStatus::Skip(_))
            })
            .count();
        assert!(skipped >= 1, "fixture must exercise the skip path");

        let strict = verify_project_with(dir.path(), VerifyMode::RequireBuild);
        assert!(!strict.passed, "un-run build steps must fail the run");
        for name in gated {
            let check = strict.checks.iter().find(|c| c.name == name).unwrap();
            assert!(
                !matches!(check.status, CheckStatus::Skip(_)),
                "{name} must not silently skip in RequireBuild mode: {:?}",
                check.status
            );
        }
        let build = strict
            .checks
            .iter()
            .find(|c| c.name == "npm run build passes")
            .unwrap();
        assert!(
            format!("{:?}", build.status).contains("required build step did not run"),
            "failure should explain why: {:?}",
            build.status
        );
    }

    #[test]
    fn test_resolve_next_bin_prefers_the_real_js_entry() {
        // `node_modules/.bin/next` is a POSIX shell script, which Node cannot
        // execute on Windows (SyntaxError on `basedir=$(...)`). The resolver
        // must prefer the real JS entry whenever it exists.
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let dist = base.join("node_modules").join("next").join("dist").join("bin");
        let shim = base.join("node_modules").join(".bin");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::create_dir_all(&shim).unwrap();
        std::fs::write(dist.join("next"), "console.log('next');").unwrap();
        std::fs::write(shim.join("next"), "#!/bin/sh\nbasedir=$(dirname x)\n").unwrap();

        assert_eq!(resolve_next_bin(base), dist.join("next"));

        std::fs::remove_file(dist.join("next")).unwrap();
        assert_eq!(resolve_next_bin(base), shim.join("next"));
    }
}

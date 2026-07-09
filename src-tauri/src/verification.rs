// src-tauri/src/verification.rs
//
// Post-build deployment verification phase for the orchestrator pipeline.
// Closes gap: after agents build code, verify it actually works before
// marking a wave as complete.
//
// Checks performed:
//   Project: package.json, node_modules, git remote, README, .env
//   Build:   dist/ exists, tsc passes, npm build passes
//   Deploy:  vercel.json with /api exclusion, SPA routing
//   Quality: ErrorBoundary, loading states, API client production config
//   Security: CORS origin, JWT not hardcoded, npm audit

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let mut report = VerificationReport::new(&base.to_string_lossy());

    // ── 1. Project structure ──────────────────────────────────────
    check_package_json(base, &mut report);
    check_node_modules(base, &mut report);
    check_git_remote(base, &mut report);
    check_readme(base, &mut report);
    check_env_file(base, &mut report);
    check_package_scripts(base, &mut report);

    // ── 2. Build output ──────────────────────────────────────────
    check_build_output(base, &mut report);
    check_index_html(base, &mut report);
    check_typescript(base, &mut report);
    check_npm_build(base, &mut report);

    // ── 3. Deployment config ─────────────────────────────────────
    check_spa_config(base, &mut report);
    check_docker_vercel_conflict(base, &mut report);

    // ── 4. Code quality ──────────────────────────────────────────
    check_error_boundary(base, &mut report);
    check_loading_states(base, &mut report);
    check_api_client_production(base, &mut report);
    check_cors_production(base, &mut report);
    check_security(base, &mut report);

    report
}

// ── Individual Checks ──────────────────────────────────────────────────

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
    if has_dist || has_build {
        let dir = if has_dist { "dist/" } else { "build/" };
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

fn check_typescript(base: &Path, report: &mut VerificationReport) {
    if !base.join("tsconfig.json").exists() {
        report.add(BuildCheck {
            name: "TypeScript compiles".into(),
            status: CheckStatus::Skip("No tsconfig.json — not a TypeScript project".into()),
            detail: String::new(),
        });
        return;
    }
    let output = Command::new("npx")
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
            let stderr = String::from_utf8_lossy(&out.stderr);
            let err_count = stderr.lines().filter(|l| l.contains("error TS")).count();
            report.add(BuildCheck {
                name: "TypeScript compiles".into(),
                status: CheckStatus::Fail(format!("tsc --noEmit: {} errors", err_count)),
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
    let output = Command::new("npm")
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
        Ok(_) => {
            report.add(BuildCheck {
                name: "npm run build passes".into(),
                status: CheckStatus::Fail("npm run build failed — check build errors".into()),
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
            "const api = axios.create({ baseURL: '/api' });",
        )
        .unwrap();
        std::fs::create_dir_all(base.join("src").join("pages")).unwrap();
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
}

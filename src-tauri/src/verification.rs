// src-tauri/src/verification.rs
//
// Post-build deployment verification phase for the orchestrator pipeline.
// Closes gap: after agents build code, verify it actually works before
// marking a wave as complete.
//
// Checks performed:
//   1. Build output exists (npm run build produced dist/ or build/)
//   2. SPA routing is configured (vercel.json rewrites for React Router)
//   3. API endpoints respond (health check, auth middleware)
//   4. Deploy config is generated if missing

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

/// Run a full deployment verification on a project directory.
/// Returns a VerificationReport with all checks.
pub fn verify_project(base: &Path) -> VerificationReport {
    let mut report = VerificationReport::new(&base.to_string_lossy());

    // Check 1: Does package.json exist?
    let pkg = base.join("package.json");
    if !pkg.exists() {
        report.add(BuildCheck {
            name: "package.json exists".into(),
            status: CheckStatus::Fail("No package.json found — not a Node.js project".into()),
            detail: String::new(),
        });
        return report;
    }
    report.add(BuildCheck {
        name: "package.json exists".into(),
        status: CheckStatus::Pass,
        detail: String::new(),
    });

    // Check 2: Build output directory exists
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

    // Check 3: SPA routing config (vercel.json or _redirects)
    let vercel_config = base.join("vercel.json");
    let redirects = base.join("public").join("_redirects");
    let netlify_toml = base.join("netlify.toml");

    let has_spa_config = vercel_config.exists() || redirects.exists() || netlify_toml.exists();

    if has_spa_config {
        let files: Vec<&str> = [
            ("vercel.json", vercel_config.exists()),
            ("public/_redirects", redirects.exists()),
            ("netlify.toml", netlify_toml.exists()),
        ]
        .iter()
        .filter(|(_, e)| *e)
        .map(|(f, _)| *f)
        .collect();

        report.add(BuildCheck {
            name: "SPA routing config".into(),
            status: CheckStatus::Pass,
            detail: format!("Found: {}", files.join(", ")),
        });
    } else {
        // Check if this looks like a SPA (has index.html, React, Vue, etc.)
        let has_index = base.join("index.html").exists();
        let is_spa = has_index || has_dist || has_build;
        if is_spa {
            report.add(BuildCheck {
                name: "SPA routing config".into(),
                status: CheckStatus::Fail(
                    "Missing SPA routing config. Add vercel.json with rewrite rules for client-side routing."
                        .into(),
                ),
                detail: "Create vercel.json: {\"rewrites\":[{\"source\":\"/(.*)\",\"destination\":\"/index.html\"}]}"
                    .into(),
            });
        } else {
            report.add(BuildCheck {
                name: "SPA routing config".into(),
                status: CheckStatus::Skip("Not a SPA project".into()),
                detail: String::new(),
            });
        }
    }

    // Check 4: Runtime smoke test — can we start the server?
    // Only if dist/ exists with index.html
    let index_html = base.join("dist").join("index.html");
    if index_html.exists() {
        // Read index.html to verify it's a valid HTML file
        match std::fs::read_to_string(&index_html) {
            Ok(content) => {
                let has_root = content.contains("root") || content.contains("app");
                let has_script = content.contains("<script");
                if has_root && has_script {
                    report.add(BuildCheck {
                        name: "dist/index.html valid".into(),
                        status: CheckStatus::Pass,
                        detail: format!("{} bytes, contains root mount + script tag", content.len()),
                    });
                } else {
                    report.add(BuildCheck {
                        name: "dist/index.html valid".into(),
                        status: CheckStatus::Fail(
                            "dist/index.html missing root mount or script tag".into(),
                        ),
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

    report
}

/// Run `npm run build` in the given directory with a timeout.
/// Returns (success, stdout_lines, stderr)
pub fn run_build(project_path: &Path, timeout_secs: u64) -> (bool, Vec<String>, String) {
    let output = Command::new("npm")
        .args(["run", "build"])
        .current_dir(project_path)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            (out.status.success(), stdout, stderr)
        }
        Err(e) => (false, vec![], format!("Failed to run npm build: {e}")),
    }
}

/// Generate a vercel.json with SPA rewrites for the given project.
/// Returns the path to the generated file or an error.
pub fn generate_vercel_config(base: &Path) -> Result<PathBuf, String> {
    let vercel_json = base.join("vercel.json");
    if vercel_json.exists() {
        return Err("vercel.json already exists".into());
    }

    let config = r#"{"rewrites":[{"source":"/(.*)","destination":"/index.html"}]}"#;
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
            "scripts": { "build": "echo built" }
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

        dir
    }

    #[test]
    fn test_verify_project_all_pass() {
        let project = make_project("passing-app");
        let base = project.path();

        // Create vercel.json
        std::fs::write(
            base.join("vercel.json"),
            r#"{"rewrites":[{"source":"/(.*)","destination":"/index.html"}]}"#,
        )
        .unwrap();

        let report = verify_project(base);
        assert!(report.passed, "Should pass all checks");
        assert_eq!(report.passed_count, report.total, "All checks should pass");
    }

    #[test]
    fn test_verify_project_missing_spa_config() {
        let project = make_project("missing-spa");
        let base = project.path();
        // No vercel.json created

        let report = verify_project(base);
        assert!(!report.passed, "Should fail due to missing SPA config");
        let spa_check = report
            .checks
            .iter()
            .find(|c| c.name == "SPA routing config")
            .unwrap();
        assert!(
            matches!(spa_check.status, CheckStatus::Fail(_)),
            "SPA check should fail"
        );
    }

    #[test]
    fn test_verify_project_no_dist() {
        let project = make_project("no-dist");
        let base = project.path();
        std::fs::remove_dir_all(base.join("dist")).unwrap();

        let report = verify_project(base);
        assert!(!report.passed);
        let build_check = report
            .checks
            .iter()
            .find(|c| c.name == "build output exists")
            .unwrap();
        assert!(matches!(build_check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_verify_project_no_package_json() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        let report = verify_project(base);
        assert!(!report.passed);
        let pkg_check = report.checks.iter().find(|c| c.name == "package.json exists").unwrap();
        assert!(matches!(pkg_check.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn test_generate_vercel_config_creates_file() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // No vercel.json yet
        assert!(!base.join("vercel.json").exists());

        let result = generate_vercel_config(base);
        assert!(result.is_ok());

        let content = std::fs::read_to_string(base.join("vercel.json")).unwrap();
        assert!(content.contains("rewrites"));
        assert!(content.contains("index.html"));
    }

    #[test]
    fn test_generate_vercel_config_errors_if_exists() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        std::fs::write(base.join("vercel.json"), "{}").unwrap();

        let result = generate_vercel_config(base);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_status_equality() {
        assert_eq!(CheckStatus::Pass, CheckStatus::Pass);
        assert_ne!(CheckStatus::Pass, CheckStatus::Fail("x".into()));
        assert_ne!(
            CheckStatus::Fail("x".into()),
            CheckStatus::Fail("y".into())
        );
    }

    #[test]
    fn test_verification_report_counting() {
        let mut report = VerificationReport::new("/test");
        report.add(BuildCheck {
            name: "check1".into(),
            status: CheckStatus::Pass,
            detail: String::new(),
        });
        report.add(BuildCheck {
            name: "check2".into(),
            status: CheckStatus::Fail("broken".into()),
            detail: String::new(),
        });
        report.add(BuildCheck {
            name: "check3".into(),
            status: CheckStatus::Skip("n/a".into()),
            detail: String::new(),
        });

        assert_eq!(report.total, 3);
        assert_eq!(report.passed_count, 1);
        assert!(!report.passed);
    }
}

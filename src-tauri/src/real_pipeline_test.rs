//! Full real pipeline run — the acceptance proof that SourceForge's promised
//! outcome (spec → provision → scaffold → **real agents** → verify → deploy)
//! actually works end to end.
//!
//! Ignored by default: it spawns a REAL agent CLI and takes minutes.
//!
//!   $env:ACC_AGENT_MODEL = "opencode-go/gpt-5.6-luna"
//!   $env:ACC_AGENT_TIMEOUT = "240"
//!   cargo test -p sourceforge --lib -- --ignored --nocapture real_pipeline
//!
//! Set `ACC_REAL_DEPLOY=1` to also run the real `vercel deploy --prod`
//! (otherwise a mock deployer stands in and the deploy stage is a no-op).

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::deployer::{Deployer, MockDeployer, VercelDeployer};
use crate::pipeline::{AdapterWaveRunner, NoopEventSink, PipelineAdapters, PipelineOptions};
use crate::stack_registry::CliStatus;

const STACK: &str = "nextjs-prisma-vercel";

/// A git repo that satisfies every deterministic verification check, so a pass
/// proves the pipeline stages work (not that the fixture was lucky).
fn make_fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    let base = dir.path();

    std::fs::write(
        base.join("package.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "real-pipeline-fixture",
            "scripts": {
                "dev": "vite",
                "build": "vite build",
                "start": "vite preview",
                "typecheck": "tsc --noEmit",
                "test": "vitest run"
            }
        }))
        .unwrap(),
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
    std::fs::write(
        base.join("README.md"),
        "# Real Pipeline Fixture\n\n## Setup\n\nRun `npm install` then `npm run dev` for local development.\n\
         Build with `npm run build` and preview with `npm start`.\n\n\
         ## Environment Variables\n\nCopy `.env.example` to `.env`. Required: JWT_SECRET, DATABASE_URL, PORT, CLIENT_URL.\n\n\
         ## Architecture\n\nVite + React application with an API surface, tested with Vitest \
         and deployed to Vercel with SPA rewrites. The build pipeline verifies build output, \
         SPA routing, runtime smoke tests, and API client production configuration.\n\n\
         ## Testing\n\nRun `npm test` for unit tests and `npm run typecheck` for static analysis.\n",
    )
    .unwrap();
    std::fs::write(
        base.join(".env.example"),
        "JWT_SECRET=change-me\nDATABASE_URL=postgres://localhost/app\nPORT=3001\nCLIENT_URL=http://localhost:5173\n",
    )
    .unwrap();
    std::fs::create_dir_all(base.join("src").join("api")).unwrap();
    std::fs::write(
        base.join("src").join("App.tsx"),
        "import { ErrorBoundary } from 'react-error-boundary';\nexport default function App() { return <div/>; }",
    )
    .unwrap();
    std::fs::write(
        base.join("src").join("api").join("client.ts"),
        "const api = axios.create({ baseURL: import.meta.env.VITE_API_URL || '/api' });",
    )
    .unwrap();

    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(base)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "pipeline@test.local"]);
    git(&["config", "user.name", "Pipeline Test"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://example.com/real-pipeline-fixture.git",
    ]);

    dir
}

/// Two independent steps → two agents running concurrently in their own
/// worktrees (exercises the parallel wave model, not just a single agent).
fn make_spec(dir: &Path) -> String {
    let path = dir.join("real-plan.md");
    let body = "# Plan\n\n\
        ## Phase 1: Build\n\n\
        ### Step 1.1: Add a greeting module\n\
        **Wave:** A \u{00b7} **Depends on:** \u{2014}\n\
        Create a file named src/greeting.ts in this repository that exports \
        `export function greet(name: string): string { return `Hello ${name}`; }` \
        as dependency-free TypeScript.\n\n\
        ### Step 1.2: Add a farewell module\n\
        **Wave:** A \u{00b7} **Depends on:** \u{2014}\n\
        Create a file named src/farewell.ts in this repository that exports \
        `export function farewell(name: string): string { return `Bye ${name}`; }` \
        as dependency-free TypeScript.\n";
    std::fs::write(&path, body).unwrap();
    path.to_string_lossy().to_string()
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn all_clis_installed() -> HashMap<String, CliStatus> {
    [
        "node", "npm", "vercel", "python3", "pip", "git", "cargo", "docker",
    ]
    .iter()
    .map(|cli| (cli.to_string(), CliStatus::Installed("1.0".to_string())))
    .collect()
}

#[test]
#[ignore = "spawns a real agent CLI; run explicitly with --ignored"]
fn test_real_pipeline_end_to_end() {
    let timeout_secs: u64 = std::env::var("ACC_AGENT_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(240);
    let model = std::env::var("ACC_AGENT_MODEL").unwrap_or_default();
    let real_deploy = std::env::var("ACC_REAL_DEPLOY").as_deref() == Ok("1");
    println!(
        "[real] agent=opencode model={:?} timeout={}s real_deploy={}",
        model, timeout_secs, real_deploy
    );

    let project = make_fixture_repo();
    let spec_dir = tempfile::TempDir::new().unwrap();
    let spec = make_spec(spec_dir.path());

    let db_dir = tempfile::TempDir::new().unwrap();
    let conn: Connection = crate::db::init_db_path(&db_dir.path().join("real.db")).expect("init db");
    let db = Mutex::new(conn);

    let run = {
        let conn = db.lock().unwrap();
        crate::pipeline_store::create_run(
            &conn,
            None,
            &spec,
            &project.path().to_string_lossy(),
            Some(STACK),
        )
        .unwrap()
    };

    let opts = PipelineOptions {
        run_id: run.id.clone(),
        project_id: None,
        spec_path: spec.clone(),
        project_path: project.path().to_string_lossy().to_string(),
        stack_id: Some(STACK.to_string()),
        agent_command: "opencode".to_string(),
        base_branch: "main".to_string(),
        allow_deploy_on_failed_verification: false,
        generate_dockerfile: true,
        agent_timeout_secs: timeout_secs,
    };

    let registry = crate::agent_adapters::AdapterRegistry::new();
    let wave_runner = AdapterWaveRunner {
        registry,
        agent_command: "opencode".to_string(),
        base_branch: "main".to_string(),
        deadline_secs: None,
        cost_cap_usd: None,
    };
    let mock_deployer = MockDeployer {
        url: "https://mock-vercel.example.app".to_string(),
        fail: false,
    };
    let vercel_deployer = VercelDeployer;
    let deployer: &dyn Deployer = if real_deploy {
        &vercel_deployer
    } else {
        &mock_deployer
    };
    let llm = crate::compounder_llm::LlmProvider::Static("[]".to_string());

    let adapters = PipelineAdapters {
        deployer,
        wave_runner: &wave_runner,
        event_sink: &NoopEventSink,
        llm: &llm,
        cli_status: Some(all_clis_installed()),
        run_compounder: false,
    };

    let report = crate::pipeline::run_pipeline(&db, &opts, &adapters).expect("pipeline ran");

    println!("\n===== BUILD REPORT =====");
    println!("status : {}", report.status);
    println!("error  : {:?}", report.error);
    println!("plan_id: {:?}", report.plan_id);
    for stage in &report.stages {
        println!(
            "  stage {:<20} {:<14} {}",
            stage.name, stage.status, stage.message
        );
    }
    if let Some(wave) = &report.wave {
        println!("wave agents:");
        for agent in &wave.agents {
            let worktree = Path::new(&agent.worktree_path);
            let handoff = worktree.join(format!("HANDOFF_{}.md", agent.agent_ref));
            println!(
                "  {:<6} status={:<8} handoff_exists={}  worktree={}",
                agent.agent_ref,
                agent.status,
                handoff.exists(),
                agent.worktree_path
            );
            println!(
                "         worktree files: {:?}",
                std::fs::read_dir(worktree)
                    .map(|entries| entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect::<Vec<_>>())
                    .unwrap_or_default()
            );
            match std::fs::read_to_string(&handoff) {
                Ok(content) => {
                    let (valid, missing) = crate::orchestrator::validate_handoff_schema(&content);
                    println!(
                        "         handoff valid={} missing={:?} bytes={}",
                        valid,
                        missing,
                        content.len()
                    );
                    for line in content.lines().take(40) {
                        println!("         | {line}");
                    }
                }
                Err(error) => println!("         handoff unreadable: {error}"),
            }
        }
    }
    if let Some(v) = &report.verification {
        println!("verification passed={}", v["passed"]);
        if let Some(checks) = v["checks"].as_array() {
            for check in checks {
                // `status` is an enum (Pass | Fail(String) | Skip(String) | ...):
                // print it raw so failures are not collapsed to "?".
                println!(
                    "  check {:<42} {}",
                    check["name"].as_str().unwrap_or("?"),
                    check["status"]
                );
            }
        }
    }
    println!(
        "deploy: {:?}",
        report.deploy.as_ref().map(|d| d.url.clone())
    );
    let agent_greeting = report
        .wave
        .as_ref()
        .and_then(|w| w.agents.first())
        .map(|a| Path::new(&a.worktree_path).join("src/greeting.ts").exists())
        .unwrap_or(false);
    println!(
        "artifacts: dist/index.html={} Dockerfile={} agent_wrote_greeting_in_worktree={}",
        project.path().join("dist/index.html").exists(),
        project.path().join("Dockerfile").exists(),
        agent_greeting
    );
    println!("========================\n");

    // Preserve artifacts for inspection: the worktrees live inside the fixture
    // TempDir, which is deleted when it drops.
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/real-run-artifacts");
    let _ = std::fs::remove_dir_all(&artifacts);
    if copy_dir_all(project.path(), &artifacts).is_ok() {
        println!("artifacts preserved at {}", artifacts.display());
    }

    // Best-effort cleanup of worktrees created relative to the crate dir (H2).
    let _ = std::fs::remove_dir_all(Path::new(env!("CARGO_MANIFEST_DIR")).join(".worktrees"));

    assert_eq!(
        report.status,
        crate::pipeline_store::STATUS_SUCCEEDED,
        "pipeline did not succeed: {:?}",
        report.error
    );
    let wave = report.wave.as_ref().expect("wave report");
    assert!(!wave.agents.is_empty(), "no agents were spawned");
    let with_handoff = wave
        .agents
        .iter()
        .filter(|a| {
            Path::new(&a.worktree_path)
                .join(format!("HANDOFF_{}.md", a.agent_ref))
                .exists()
        })
        .count();
    assert_eq!(
        with_handoff,
        wave.agents.len(),
        "agents without a valid handoff: {:?}",
        wave.agents
            .iter()
            .map(|a| (a.agent_ref.clone(), a.status.clone()))
            .collect::<Vec<_>>()
    );
}

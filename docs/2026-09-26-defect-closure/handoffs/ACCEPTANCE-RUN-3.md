# Acceptance Run 3 — fully green real pipeline (post-hardening)

**Date:** 2026-09-29 · branch `hardening/production`

```powershell
$env:ACC_AGENT_MODEL="opencode-go/gpt-5.6-luna"; $env:ACC_AGENT_TIMEOUT="420"
cargo test -p sourceforge --lib -- --ignored --nocapture real_pipeline
```

## Result

```
status : succeeded                    (finished in 63.65s)
  1-1.1  status=done  handoff_exists=true  handoff valid=true missing=[] (725 bytes)
  agent_wrote_greeting_in_worktree=true
  verification passed=true
  deploy: mock → https://mock-vercel.example.app (+ Dockerfile artifact)
```

Every stage ran, the real agent completed the task (`src/greeting.ts` + a test), wrote a
schema-valid handoff with all six required sections, verification passed, and the deploy
stage produced its artifact. Previously the same run took 481–1202 s and **failed** — the
agent's work was never collected.

## What this loop changed

| Fix | Why it mattered |
|---|---|
| **P0 — incomplete agents can no longer report success** (`pipeline.rs`) | A wave with zero completed agents reported `succeeded` because verification passed independently. Now the run pauses as `awaiting_user` with an explanatory error, and the deploy stage is skipped. Test: `test_incomplete_agents_do_not_report_success`. |
| **Fail-fast supervision** (`wave_supervisor.rs`, `agent_adapters/*`) | The supervisor polled for a handoff until the deadline even after the agent process had **exited**, burning every attempt's full budget. Adapters now expose `is_running` (opencode marks a session finished on stream EOF); the supervisor fails/retries immediately. Test: `test_exited_agent_fails_fast_instead_of_waiting_for_deadline`. |
| **Pointer prompt instead of inlined guideline** (`wave_executor.rs`) | The spec-derived guideline text was passed as a command-line argument through `cmd /C` — a command-injection vector and a command-line-length risk for large specs. Spawn now sends a short constant that points at `.acc/GUIDELINE.md` (the phrasing proven by the dogfood probe). |
| **Resume no longer blocked by an existing branch** (`worktree.rs`) | `create_worktree` errored with "Branch already exists", so resuming a run always failed. It now re-attaches idempotently. Test renamed to `..._is_reused_on_resume`. |
| **Resume reuses the run's options** (`pipeline.rs`, `pipeline_commands.rs`) | The effective options are persisted in `report.artifacts` and restored by `resume_build_app_cmd`, instead of silently reverting to defaults (losing a pinned agent or a raised timeout). |
| **User-visible mojibake fixed** (4 files, 25 strings) | Stage/UI messages contained `â€”` / `â†’` (the earlier encoding fixer used the wrong byte mapping), e.g. the provision and deploy messages shown to users. |

## Gates

Rust **185 lib + 42 integration**; Web **338**; `tsc`/`lint` clean. The real run stays
`#[ignore]`d, so CI remains hermetic.

## Still open (honest)

- **Cost cap (M9) is still inert**: agents' cost is never populated on the adapter path, so
  `SupervisionConfig::cost_cap_usd` cannot trigger. Needs output→cost accounting.
- **Multi-agent concurrency** has not been run for real (1 agent, 1 task).
- **Cancel/Resume UI**: backend commands exist (`cancel_build_app_cmd`,
  `resume_build_app_cmd`) but the sync command does not surface `run_id` until completion,
  so the app has no cancel button yet.
- **Deploy/Supabase unproven**: the acceptance run uses a mock deployer (`ACC_REAL_DEPLOY=1`
  enables the real `vercel`); provisioning still relies on the DB-row MCP proxy.
- The pointer-prompt change removes spec text from the command line for the pipeline, but
  `AgentAdapter::spawn` is still called with arbitrary strings by other callers (Runner
  page) — those remain exposed to `cmd /C` quoting quirks.

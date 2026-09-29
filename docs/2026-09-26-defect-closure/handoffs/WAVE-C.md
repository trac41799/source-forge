# WAVE-C HANDOFF — Supervised Build Pipeline

**Date:** 2026-09-26
**Plan:** `docs/2026-09-26-defect-closure/TDD_PLAN.md` (Phase 3, Steps 3.1–3.9; 3.10 deferred)
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (DG-1; §6 decisions resolved 2026-09-26)
**Branch/worktree:** `wave/A/unblock` @ `.worktrees/wave-A`

## Original Task

Build the missing core promise: a supervised, resumable, cancellable `build_app` pipeline — parse spec → resolve stack → provision → scaffold → seed plan → execute waves → finalize+verify → deploy → report — with injectable seams so the full flow is testable hermetically, plus the command surface and a Build App UI.

## Completed By

opencode (single agent; Integrator + Feature + Verifier roles inline)

## Model Used

opencode-go/deepseek-v4.1-flash

## Output Summary

The pipeline exists end-to-end and passes a hermetic E2E: a fixture project with one spec task runs all 9 stages, spawns 1 mock agent (handoff parsed), attaches a **passing** verification report, deploys via mock (`https://mock.example.app`), writes a Dockerfile artifact, and compounds 1+ knowledge item. Resume works (`awaiting_user` on missing CLI → second run completes, no duplicate plan); hard failures mark the run failed; a pre-cancelled run returns `cancelled`. The command surface (`build_app_cmd`, `resume_build_app_cmd`, `cancel_build_app_cmd`, `get_build_app_status_cmd`) runs the sync pipeline on `spawn_blocking` with `Arc<Mutex<Connection>>`, emits `build-app-progress` events, and a new `/build` page drives it (form → progress → report with verification checks + deploy URL).

**Deferred:** Step 3.10 (separate `verification_reports` table + Orchestrate wave badge) — verification is persisted inside `build_runs.report` and surfaced on the Build App page. A separate `tests/pipeline_e2e.rs` was consolidated into `pipeline::tests::test_full_pipeline_with_mock_runner_produces_complete_report` (same coverage, no duplicate fixtures).

## Files Changed

- `src-tauri/migrations/017_build_runs.sql` (new; numbered 017 to avoid collision with the other session's `016_decision_usage`)
- `src-tauri/src/pipeline_store.rs` (new — run CRUD, stage log, resume lookup, cancel)
- `src-tauri/src/deployer.rs` (new — `Deployer` trait, `VercelDeployer`, `MockDeployer`, `write_dockerfile`)
- `src-tauri/src/pipeline.rs` (new — 9-stage machine, hydration-based resume, `WaveRunner`/`EventSink` seams, `BuildReport`, 6 tests incl. full E2E)
- `src-tauri/src/pipeline_commands.rs` (new — 4 commands + `TauriEventSink`)
- `src-tauri/src/commands.rs` — `AppState.db: Mutex<Connection>` → `Arc<Mutex<Connection>>` (shareable for `spawn_blocking`)
- `src-tauri/src/db.rs` — register migration 017; `src-tauri/src/lib.rs` — modules + 4 handler entries
- `src/pages/BuildApp.tsx` + `src/__tests__/pages/BuildApp.test.tsx` (new), `src/App.tsx` route, `src/components/layout/Sidebar.tsx` WORK entry
- `src/lib/ipc/commands.ts` + `ipc-unused.snapshot.json` (4 new constants; snapshot regenerated deliberately)
- `.github/workflows/ci.yml` — optional non-blocking `real-cli-e2e` job (deterministic pipeline E2E always; real opencode run only when `OPENROUTER_API_KEY` secret exists and a fixture is present)
- `docs/2026-09-26-defect-closure/SPEC.md` — §6 decisions recorded

## Handoff Instructions

1. **Wave D starts here.** 4.1/4.2/4.3 wire the handoff watcher, retry/correction, and PTY-kill-on-cancel into `AdapterWaveRunner`; today the pipeline relies on `execute_wave_with_adapters` semantics and cancel only checks between stages.
2. **Production adapter path:** only `mock` and `opencode` adapters exist; `build_app` defaults to `opencode`. Other CLIs need adapters or the PTY executor (out of scope per SPEC DG-5).
3. **Provision pause:** stacks requiring Supabase pause as `awaiting_user` unless a `supabase_configs` row exists for the project; resume re-runs the stage.
4. **Deploy gate:** deploy is skipped (not failed) when verification fails unless `allow_deploy_on_failed_verification` is set; the UI always sends `false`.
5. **Known limitations:** `cancel` between stages only; stage log accumulates entries across resumes; `run_on_blocking_thread` blocks one tokio blocking thread per build (by design).

## Verification Evidence

| Gate | Command | Result |
|---|---|---|
| Pipeline E2E (hermetic) | `cargo test -p sourceforge --lib pipeline::tests` | 6 passed — full run, resume, failure, cancel, stage order |
| Rust lib | `cargo test -p sourceforge --lib` | **159 passed, 0 failed** |
| Rust workspace | `cargo test --workspace` | exit 0 (159 + 42) |
| JS suite | `npx vitest run` | **332 passed, 0 failed** |
| Build App page | `npx vitest run src/__tests__/pages/BuildApp.test.tsx` | 2 passed (form + options/report render) |
| IPC contract | `npx vitest run src/__tests__/contracts/ipc-contract.test.ts` | 4 passed (0 dead invokes, snapshot regenerated) |
| TypeScript / Lint | `npx tsc --noEmit` / `npm run lint` | exit 0 / 0 errors |

# WAVE-D HANDOFF — Agent Reliability & E2E Probe

**Date:** 2026-09-26
**Plan:** `docs/2026-09-26-defect-closure/TDD_PLAN.md` (Phase 4, Steps 4.1–4.4)
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (DG-5)
**Branch/worktree:** `wave/A/unblock` @ `.worktrees/wave-A`

## Original Task

Make agent execution reliable: a handoff watcher (poll → valid handoff / deadline), one retry with correction docs, kill-on-cancel, and a dogfood `--agent` probe (deterministic stub in CI, real CLI optional).

## Completed By

opencode (single agent; Integrator + Feature + Verifier roles inline)

## Model Used

opencode-go/deepseek-v4.1-flash

## Output Summary

`wave_supervisor.rs` supervises running agents: it polls `HANDOFF_<agent_ref>.md` every interval, marks done on a schema-valid handoff, kills + retries once on deadline/cost-cap (then writes a correction doc), and kills + stops on cancellation. The pipeline's production path (`AdapterWaveRunner`) now supervises after spawn and checks `pipeline_store::is_cancelled` during execution, so `cancel_build_app_cmd` can stop a running wave (adapter kill). Hermetic runners (`TestWaveRunner`) return no control and skip supervision, keeping the pipeline E2E fast and deterministic. `dogfood --agent "<cmd>" <fixture>` spawns a real or stub agent, waits for a valid handoff, and exits 0/1 — verified locally with the stub script (exit 0).

## Files Changed

- `src-tauri/src/wave_supervisor.rs` (new — `SupervisionConfig`, `AgentControl`, `supervise_agents`, 5 tests: done / incomplete-handoff / retry / cost-cap / cancel-kill)
- `src-tauri/src/wave_executor.rs` — `AgentExecution.retry_count` (`#[serde(default)]`)
- `src-tauri/src/pipeline.rs` — `WaveRunner::control()`, `AdapterWaveRunner: AgentControl` (kill/respawn via registry), supervision in `stage_execute_waves` with cancel closure, `agent_timeout_secs` option, `run_id` in pipeline state
- `src-tauri/src/pipeline_commands.rs` — `agent_timeout_secs` option plumbing
- `src-tauri/src/integration_tests.rs` — fixture field updates
- `src-tauri/src/lib.rs` — module registration
- `tools/dogfood.rs` — `--agent` probe + quote-aware `split_command`
- `tools/fixtures/stub-agent.ps1` + `tools/fixtures/fixture-repo/` (new)
- CI `real-cli-e2e` job (Wave C) now has a fixture to probe

## Handoff Instructions

1. **Wave E is next** (hardening/release): CSP, error boundaries, capability minimization, docs, release gate, NexusBoard acceptance. Open items from earlier waves: Step 3.10 (verification_reports table + Orchestrate badge), full literal-ratchet migration, sqlite-vec wiring.
2. **Real-CLI E2E remains manual/optional:** CI runs it only with `OPENROUTER_API_KEY`; the release gate should run `dogfood --agent "opencode run" <fixture>` once on a machine with the CLI installed.
3. **Kill semantics:** adapter `kill` is best-effort (mock/opencode adapters); PTY-backed agents killed via `PtyManager` is only used by the legacy `execute_wave_real` path.
4. `retry_count` is persisted in wave reports (serde default keeps old reports readable).

## Verification Evidence

| Gate | Command | Result |
|---|---|---|
| Supervisor tests | `cargo test -p sourceforge --lib wave_supervisor` | 5 passed (done, incomplete, retry, cost-cap, cancel-kill) |
| Rust lib | `cargo test -p sourceforge --lib` | **164 passed, 0 failed** |
| Rust workspace | `cargo test --workspace` | exit 0 (164 + 42) |
| Probe (deterministic) | `cargo run -q -p dogfood -- --agent "powershell -File <stub>" tools/fixtures/fixture-repo` | **exit 0** — valid handoff detected |
| Pipeline regression | `cargo test -p sourceforge --lib pipeline::tests` | 6 passed (supervision skipped for hermetic runner) |

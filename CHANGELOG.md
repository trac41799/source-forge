# Changelog

All notable changes to SourceForge (formerly Agent Control Center).

## [Unreleased] — 0.10.0

Defect-closure and pipeline program (branches `wave/A`–`wave/E`).

### Added
- **Supervised build pipeline** (`build_app`): parse spec → resolve stack →
  provision → scaffold → seed plan → execute waves → finalize + verify →
  deploy → report. Resumable (`awaiting_user`), cancellable, progress events.
- **Build App page** (`/build`) with stage timeline, verification results, and
  deploy URL.
- **Agent supervision**: handoff watcher, deadline/cost-cap kill, one retry,
  correction docs, kill-on-cancel.
- **IPC contract test**: registry vs. `invoke()` cross-check, `IPC` constants,
  inline-literal ratchet, unused-command snapshot.
- **Knowledge Compounder command path** (`run_compounder_cmd`) with an
  injectable LLM provider seam; auto-runs after verified waves.
- **Verification gate on finalize** (`verify_and_finalize_wave_cmd`).
- **CI** (`.github/workflows/ci.yml`): web, Rust (Windows + Linux), Python,
  IPC contract, optional real-CLI E2E.
- **Toolchain pin** (`rust-toolchain.toml`), `dogfood --agent` probe, stub
  agent fixture, Dockerfile artifact generation.
- Docs: `docs/USER_GUIDE.md`, `docs/CONTRIBUTING.md`, this changelog.

### Fixed
- Rust library did not compile (72 × E0015 in the stack registry).
- Frontend build failed (Settings stack selector referenced a missing store method).
- 13 frontend commands were unregistered (backward channel, costs, compounder, preflight) — silently broken.
- `budget.rs` row mapper read the wrong columns — budget queries always failed.
- Migration `011_memory.sql` aborted on a `vec0` table, so memory tables were never created.
- Test suites: Rust integration target used the pre-rebrand crate name; Python suites could not import `webhook_server`; several store/verification fixtures were stale.
- `vitest` excluded every test inside a worktree.

### Security
- Content Security Policy re-enabled in `tauri.conf.json`.
- Removed unused `shell:allow-execute/spawn/stdin-write/kill` and `http:default` capabilities.
- React error boundary prevents white-screen crashes.

## Prior releases

Tags `v0.9.0` … `v0.9.12` (2026-06-07 … 2026-07-07) were early alpha builds of
the Agent Control Center UI, wave orchestration primitives, knowledge graph,
memory layer, and integrations.

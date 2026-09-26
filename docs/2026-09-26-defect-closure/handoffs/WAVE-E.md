# WAVE-E HANDOFF — Hardening & Release Readiness

**Date:** 2026-09-26
**Plan:** `docs/2026-09-26-defect-closure/TDD_PLAN.md` (Phase 5, Steps 5.1–5.5; 5.6 documented)
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (DG-7)
**Branch/worktree:** `wave/A/unblock` @ `.worktrees/wave-A`

## Original Task

Harden for release: restore CSP, add an error boundary, minimize Tauri
capabilities, write the user-facing docs, and gate releases on green gates.

## Completed By

opencode (single agent)

## Model Used

opencode-go/deepseek-v4.1-flash

## Output Summary

- **CSP restored** (`tauri.conf.json`) — the explicit policy from `f4ae90b`
  (`default-src 'self'; … connect-src 'self' ipc: http://ipc.localhost; …`).
  OpenRouter calls are Rust-side, so no webview `connect-src` entry is needed.
- **Error boundary** (`src/components/ErrorBoundary.tsx`) wraps the route tree
  in `App.tsx`; a crashing page renders a fallback with a Reload button instead
  of a white screen.
- **Capabilities minimized**: removed `shell:allow-execute`, `shell:allow-spawn`,
  `shell:allow-stdin-write`, `shell:allow-kill` (PTY spawning is Rust-side) and
  `http:default` (unused by the frontend). `shell:allow-open` retained.
  `gen/schemas/capabilities.json` regenerated accordingly.
- **Docs**: `docs/USER_GUIDE.md`, `CHANGELOG.md` (0.10.0), `docs/CONTRIBUTING.md`
  (gates, IPC contract, wave/worktree workflow, migrations).
- **Release gate**: `release.yml` now has a second preflight (`rust-preflight`
  running `cargo test --workspace` on Linux) and `build` requires both
  preflights, so a tag cannot ship a red build.

**Step 5.6 (final acceptance) is documented, not executed here:** it requires a
machine with the real CLIs and provider keys. Procedure:
`dogfood --agent "opencode run" tools/fixtures/fixture-repo` must exit 0, then
run Build App against a fresh app spec and confirm the deployed `/register`
returns 200 (no SPA 404) with the auth smoke test passing.

## Handoff Instructions

1. **Merge readiness:** the branch is feature-complete for the four bars in the
   original assessment (builds, works as designed, delivers the promised
   pipeline, hardening). Before release: run the 5.6 acceptance on a machine
   with `opencode` + `vercel` + `OPENROUTER_API_KEY`, and merge `main`
   (the other session's `decision` module landed there — expect one migration
   number coexistence: their `016_decision_usage` vs our `017_build_runs`).
2. **Still open (non-blocking):** Step 3.10 verification badge on Orchestrate;
   migrate the remaining ~130 inline `invoke()` literals; wire sqlite-vec for
   vector memory search.
3. **CI note:** the new `rust-preflight` needs the Linux webkit packages (added);
   `real-cli-e2e` stays non-blocking.

## Verification Evidence

| Gate | Command | Result |
|---|---|---|
| Hardening config | `npx vitest run src/__tests__/config/hardening.test.ts` | 2 passed (CSP enabled, capabilities minimal) |
| Error boundary | `npx vitest run src/__tests__/components/ErrorBoundary.test.tsx` | 2 passed |
| JS suite | `npx vitest run` | **336 passed, 0 failed** |
| Rust workspace | `cargo test --workspace` | exit 0 (164 + 42) |
| TypeScript / Lint | `npx tsc --noEmit` / `npm run lint` | exit 0 / 0 errors |
| Probe (Wave D) | `dogfood --agent …` stub | exit 0 |

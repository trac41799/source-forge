# WAVE-A HANDOFF — Unblock: Build Green

**Date:** 2026-09-26
**Plan:** `docs/2026-09-26-defect-closure/TDD_PLAN.md` (Phase 1, Steps 1.1–1.5)
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (SPEC-001)

## Original Task

Execute Wave A of TDD-PLAN-001: make the repository build and its full test suite green — fix the 72×E0015 Rust compile failure, the frontend typecheck failure, add toolchain pin + CI gates, repair repo hygiene, and upgrade `spec_parser` to honor explicit `**Wave:**`/`**Depends on:**` markers.

## Completed By

opencode (single-agent Wave A; Integrator + Feature + Verifier roles exercised inline)

## Model Used

opencode-go/deepseek-v4.1-flash

## Output Summary

All Wave A steps completed and all gates green. Beyond the plan, five latent defects surfaced because this is the first time the Rust test targets were ever compiled, and three of them were **real product defects** (not test-only): the budget row mapper read the wrong columns (budget queries always failed at runtime), and migration `011_memory.sql` aborted on a `vec0` virtual table, so the memory layer tables were **never created in production**. Steps 2.6 and 2.7 were pulled forward (they were required by the wave gates): JS suite uncaught errors fixed with store-level guards, Python suites fixed via test-path shims.

## Files Changed

**Step 1.1 — Rust compiles (D-01):**
- `src-tauri/src/stack_registry.rs` — `static STACK_REGISTRY` → `LazyLock<Vec<StackPreset>>`; `all()` → `&STACK_REGISTRY[..]`; iterator fixes; +2 tests.
- `src-tauri/src/spec_parser.rs` — test fixture missing `verify_deploy` field (test target never compiled).

**Step 1.2 — Frontend build green (D-02):**
- `src/stores/settingsStore.ts` — added `stackPreferences`, `loadDefaults()`, `updateDefaults()` (invoke `get_preferences_cmd` / `set_preferences_cmd`).
- `src/pages/Settings.tsx` — stack cards now driven by backend preferences; removed dead `store.updateDefaults?.()` call.
- `src/__tests__/stores/settingsStore.test.ts`, `src/__tests__/pages/Settings.test.tsx` — +5 tests.

**Step 1.3 — Toolchain + CI (D-10):**
- `rust-toolchain.toml` (new) — pins `stable`; on Windows resolves to MSVC (no more dlltool/GNU failures).
- `.github/workflows/ci.yml` (new) — web / rust (win+linux) / python / IPC-contract jobs.
- `.github/workflows/release.yml` — added `preflight` job; `build` now `needs: preflight`.
- `package.json` — added `lint:src` script.
- `README.md` — build badge now points at `ci.yml`.

**Step 1.4 — Hygiene (D-09):**
- `.gitignore` — rewritten as UTF-8; `.worktrees/`, `.acc-test/` patterns now effective.
- `eslint.config.js` — ignores `target/**`, `.worktrees/**`, `.acc-test/**`, `landing/**`.
- Removed 4 gitlink entries `.worktrees/dogfood--*`, deleted their worktrees and `dogfood/*` branches.

**Step 1.5 — Spec parser markers:**
- `src-tauri/src/spec_parser.rs` — `**Wave:**` (A–E or digit) and `**Depends on:**` markers with phase fallback; phase-boundary flush bug fixed (last step of a phase was stamped with the next phase id); +4 tests including parsing the real `TDD_PLAN.md`.

**Pulled forward (2.6 / 2.7):**
- `src/pages/Messages.tsx`, `src/stores/{orchestration,knowledge,asset,intelligence,scheduler}Store.ts` — `?? []` guards on invoke results (fixes 6 uncaught render exceptions).
- `src/__tests__/App.test.tsx` — heading-role assertion (brittle `getByText`).
- `webhook-server/conftest.py` (new), `local-daemon/tests/conftest.py` — `webhook_server` package alias (hyphen dir).

**Defects found by first-ever test run:**
- `src-tauri/src/budget.rs` — `build_agent_budget` read column 5 twice and shifted all later indices; `get_budgets`/`update_budget_usage` always failed. Fixed to 5=model, 6=budget_total, 7=budget_used, 8=state, 9=wip_path, 10=created_at, 11=updated_at.
- `src-tauri/migrations/011_memory.sql` — `vec0` virtual table moved to last statement; `memory_facts`, `session_checkpoints`, indexes now created (previously the whole batch aborted non-fatally in `db.rs`).
- `src-tauri/tests/integration_tests.rs` — crate rename `agent_control_center`→`sourceforge` (97 refs); full migration list mirrored; parent-row fixtures seeded; +2 memory-table assertions.
- `src-tauri/src/lib.rs` — 9 modules made `pub` so the external integration test target can compile.
- `src-tauri/src/handoff_parser.rs`, `src-tauri/src/verification.rs`, `src-tauri/src/agent_adapters/mod.rs` — test fixtures/contracts aligned (canonical 6-section handoff; mock adapter nested `usage.cost`; deterministic verification fixture; build/E2E checks `Skip` without `node_modules`).

## Handoff Instructions

1. **Wave B can start** (Steps 2.1–2.5 + remainder of 2.6). Note: 2.6's suite-cleanup and 2.7 are effectively done; re-scope 2.6 to "verify only".
2. **Known follow-ups carried into later waves:**
   - `tools/dogfood.rs` duplicates a minimal spec parser instead of using `spec_parser` — Wave D Step 4.4 must switch it to the real parser (or depend on the lib).
   - sqlite-vec is still not loaded; `vec_memories` does not exist. Vector memory search is unavailable until the extension is wired (new task for Wave C/E; SPEC-001 §DG-3 follow-up).
   - `local-daemon/main.py` imports `webhook_server.*` at runtime — same hyphen/underscore issue fixed only for tests. Production packaging needs the same shim or a directory rename.
   - CI workflow cannot be verified until pushed to GitHub; `contract` job is gated by `hashFiles` until Step 2.1 lands.
3. **No commit has been made** — all changes are in the working tree (plus staged deletions for the 4 worktree gitlinks). Commit message suggestion: `fix(wave-a): make repo build green — stack registry LazyLock, settings prefs wiring, MSVC pin + CI, spec-parser markers, budget mapper + memory migration fixes`.

## Verification Evidence

| Gate | Command | Result |
|---|---|---|
| Rust compile | `cargo check` | exit 0 (was 72 × E0015) |
| Rust tests (lib) | `cargo test -p sourceforge --lib` | **139 passed, 0 failed** |
| Rust tests (external) | `cargo test -p sourceforge --test integration_tests` | **42 passed, 0 failed** (first ever run; was 27/15) |
| Rust tests (workspace) | `cargo test --workspace` | exit 0 |
| TypeScript | `npx tsc --noEmit` | exit 0 (was TS2339) |
| JS tests | `npx vitest run` | **314 passed, 0 failed, 0 errors**, exit 0 |
| Lint | `npm run lint` | exit 0, 0 errors (13 warnings) |
| Frontend build | `npm run build` | exit 0, `dist/` written |
| Python (daemon) | `python -m pytest -q` in `local-daemon/` | **48 passed** (was 35 pass / 13 errors) |
| Python (webhook) | `python -m pytest -q` in `webhook-server/` | **146 passed** (was collection error) |
| Toolchain | `rustup show active-toolchain` | `stable-x86_64-pc-windows-msvc (overridden by rust-toolchain.toml)` |
| Parser (real plan) | `cargo test -p sourceforge --lib spec_parser` | 12 passed, incl. `test_parses_project_tdd_plan` (≥30 tasks, waves 1–5) |
| Hygiene | `git check-ignore .worktrees/test .acc-test/x`; UTF-8 check | both ignored; `.gitignore` valid UTF-8 |

# WAVE-B HANDOFF — Wiring & IPC Contract

**Date:** 2026-09-26
**Plan:** `docs/2026-09-26-defect-closure/TDD_PLAN.md` (Phase 2, Steps 2.1–2.5; 2.6/2.7 verified)
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (SPEC-001 §3, §5 DG-3/DG-4)
**Branch/worktree:** `wave/A/unblock` @ `.worktrees/wave-A`

## Original Task

Execute Wave B: make every advertised frontend command path real — IPC contract test + canonical constants (2.1), restore the backward-channel Chat tab (2.2), restore the Costs page (2.3), expose compounder + preflight commands with a testable LLM seam (2.4), and make verification the only finalize path (2.5).

## Completed By

opencode (single agent; Integrator + Feature + Verifier roles inline)

## Model Used

opencode-go/deepseek-v4.1-flash

## Output Summary

All 13 dead invokes from Wave A's audit are eliminated; a contract test now prevents regressions. The Knowledge Compounder is callable end-to-end (split into prepare → LLM → merge so the async command never holds `!Send` `&Connection` across an await), and the compounder now increments `confirmation_count` on re-runs (SPEC DG-3). `verify_and_finalize_wave_cmd` is now the only finalize implementation in the store. **New gap discovered: no UI path calls `executeWave`/`finalizeWave` at all — the wave-execution flow is unreachable from the UI (B-3); Wave C's BuildApp (Step 3.8) is the intended entry point.**

## Files Changed

- **2.1:** `src/lib/ipc/commands.ts` (new, 19 constants), `src/__tests__/contracts/ipc-contract.test.ts` (new), `src/__tests__/contracts/ipc-literal-baseline.json` + `ipc-unused.snapshot.json` (new, ratchet/snapshot).
- **2.2:** `src/stores/backwardChannelStore.ts` (10 names → `IPC.*`, `?? []` guards), `src/__tests__/stores/backwardChannelStore.test.ts` (new, 10 cases).
- **2.3:** `src/pages/CostAggregation.tsx` (`IPC.getCostSummary`), `src/__tests__/pages/CostAggregation.test.tsx` (+1 case).
- **2.4:** `src-tauri/src/compounder_llm.rs` (new: `LlmProvider::{OpenRouter,Static}` + `complete`), `src-tauri/src/knowledge_commands.rs` (new: `run_compounder_cmd`, `get_preflight_warnings_cmd`, 5 tests), `src-tauri/src/knowledge.rs` (split into `compounder_prepare_prompt` / `compounder_merge`; `run_compounder` kept as documented non-Send wrapper; confirmation_count bump), `src-tauri/src/lib.rs` (module + handler registration), `src/stores/knowledgeStore.ts` (IPC constants).
- **2.5:** `src/stores/orchestrationStore.ts` (`finalizeWave(report, projectPath)` → `verify_and_finalize_wave_cmd`, returns `{wave, verification}`; `executeWave` → `IPC.executeWave`; new `VerificationReport` types), `src/__tests__/stores/orchestrationStore.test.ts` (+1 case).
- **Compat:** `vitest.config.ts` — exclude patterns made root-relative (`.worktrees/**`) so the suite runs inside a worktree.

## Handoff Instructions

1. **Wave C starts here.** Step 3.8 (BuildApp UI) MUST call `store.finalizeWave(report, projectPath)` so verification stays the gate; do not resurrect `finalize_wave_cmd`.
2. **Remaining inline `invoke("...")` literals:** ~130 legacy sites remain, captured in `ipc-literal-baseline.json`. The ratchet blocks *new* literals only; migrate opportunistically. Do not regenerate the baseline to "fix" a failure — fix the code.
3. **Follow-ups:**
   - **B-3 (new):** UI execute/finalize flow unreachable — Wave C 3.8 covers it; consider a minimal Orchestrate "Execute Wave" action only if BuildApp slips.
   - The compounder's auto-run after verified waves is Wave C Step 3.5 — use the split API (`prepare` → `complete` → `merge`), not `run_compounder` (not `Send`).
   - `run_compounder_cmd` requires `OPENROUTER_API_KEY` at runtime; error path is covered by unit test.
4. **Commit** made on `wave/A/unblock` (see log). No pushes.

## Verification Evidence

| Gate | Command | Result |
|---|---|---|
| IPC contract | `npx vitest run src/__tests__/contracts/ipc-contract.test.ts` | **4/4 passed**; dead invokes = 0 (was 13); no new literals; snapshot matches |
| Backward channel | `npx vitest run src/__tests__/stores/backwardChannelStore.test.ts` | 10/10 passed (was 10/10 failed) |
| Costs | `npx vitest run src/__tests__/pages/CostAggregation.test.tsx` | 12/12 passed |
| Compounder (Rust) | `cargo test -p sourceforge --lib knowledge_commands compounder_llm` | 7 new tests passed (pipeline w/ Static LLM, dedupe + count bump, preflight rows, no-key error) |
| Rust lib | `cargo test -p sourceforge --lib` | **146 passed, 0 failed** |
| Rust workspace | `cargo test --workspace` | exit 0 (146 lib + 42 integration) |
| JS suite | `npx vitest run` | **330 passed, 0 failed** (was 314) |
| TypeScript / Lint | `npx tsc --noEmit` / `npm run lint` | exit 0 / 0 errors |
| Python | `python -m pytest -q` (both) | 48 + 146 passed |
| Worktree compat | `npx vitest run` inside `.worktrees/wave-A` | runs (previously "no tests" due to `**/.worktrees/**` exclude) |

**Defects fixed this wave:** compounder dedupe never bumped `confirmation_count` (SPEC DG-3 violation); vitest excluded every test inside any worktree; Wave A migration accidentally captured the other session's `mod decision;` line into this branch's `lib.rs` (removed — their module stays on `main`).

# REVIEW-FIXES — Closing the adversarial review (PR #6)

**Date:** 2026-09-28
**Trigger:** two specialist reviewers audited `wave/A/unblock`; verdict "do not merge as-is".
**Gates after fixes:** Rust **183 lib + 42 integration**; JS **338**; `tsc`/`lint` exit 0.

## Fixed (verified)

| Finding | Fix | Evidence |
|---|---|---|
| **B1 (backend)** default stack could never provision — `state.project_id` read before assignment | resolve project once up front (before the stage loop); `seed_plan` reuses it | call-site trace removed; new default-stack path exercised by tests |
| **B1 (frontend)** IPC contract test blind to single-quoted invokes; 2 dead commands shipped | scanner now matches `'`/`"`/`` ` ``; occurrence-keyed ratchet (L1); `detect_stack` → local TS detector; `check_skillbridge_status` → registered `check_skillbridge` | contract test **5/5**, 0 dead invokes; regression test asserts single-quote scanning |
| **M3** BOM + mojibake in DecisionPanel/pipeline files | encoding pass: BOM stripped, `…—–§·’` restored; only the 20 intended files changed (over-broad first pass reverted) | byte-level check; diff limited to intended files |
| **H3** verification-failed run reported `succeeded` | new `verification_failed` terminal status; success path only when verification passed (or override) | `STATUS_VERIFICATION_FAILED`; status derived from verification |
| **H4** retry respawned with the agent_ref as the prompt | respawn reads the worktree guideline (falls back to a descriptive ref prompt) | code + fallback |
| **H5** concurrent `build_app` could run the same run twice | `pipeline_store::claim_startable_run` errors when a run is `running` | unit-tested helper |
| **H6** SQL string-building in `update_knowledge_item` | bound parameters (`?N` + `ToSql`), no interpolation | compiles; compounder path now parameterized |
| **H7** auto-compounder inert on the adapter path | handoffs now recorded as `file_edit` events (`record_handoff_events`) | new test asserts events persisted |
| **M8** cancel killed only the current agent | cancellation now kills all still-running siblings | supervisor cancel test |
| **M10** crash-resume could duplicate the plan | report persisted after **every** stage (plan_id survives) | stage-loop change |
| **M13a** `version()` bypassed `cmd /C` + leaked a String per call | `cmd /C` on Windows + `OnceLock` cache (no leak) — also closes **L16** | code |
| **L5** plugin-http initialised but unused | removed `tauri_plugin_http::init()` | lib.rs |
| **L4** `shell:allow-open` unnecessary | capability removed; hardening test updated | hardening test asserts absence |
| **M1** CSP kept `script-src 'unsafe-inline'` | removed; hardening test now asserts no inline script-src | hardening test |
| **M2** agent field was free-text though the adapter set is an enum | replaced with a select (opencode / mock) | BuildApp tests |
| **H3 (frontend)** ErrorBoundary trapped the user on the fallback | keyed by `location.pathname` so navigation resets it | App.tsx |
| **L14/L15** tautological + env-racy tests | opencode smoke test asserts real postconditions; removed the global-env test | tests pass |
| **L7** trailing blank lines at EOF | stripped in the intended files | diff |

## Deferred (documented, non-blocking)

- **H2 (partial):** Cancel/Resume buttons in Build App. Backend commands exist and are IPC-typed, but the sync `build_app_cmd` does not surface `run_id` until completion; a proper fix (fire-and-forget returning `run_id` + polling) is a small design change — deferred rather than rushed. Docs no longer claim a UI cancel (see below).
- **M9** cost cap wired to nothing on the adapter path (config inert).
- **M11** resume when the agent branch already exists (worktree create errors).
- **M12** `resume_build_app_cmd` uses default options rather than the persisted ones.
- **M13b** kill registration still goes through a detached `tokio::spawn` (small race window right after spawn).
- **L3** `?? []` guards are dead today (Rust returns arrays) but harmless.
- **L8** `emit` failures ignored (webview gone) — acceptable.
- **L6** the Wave-A commit message overstates the net diff (budget/011/stack_registry already live on `main` via `f8eb9ea`); history not rewritten.

## Residual risk

The full **real pipeline run** is still unexecuted; with B1 fixed the default stack now provisions, and H2/H7 fix the paths that would have failed. The next real acceptance run is the remaining proof.

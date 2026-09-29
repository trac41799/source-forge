# Review round 8 — adversarial review of PR #9 (findings & fixes)

**Date:** 2026-09-29 · **Method:** independent adversarial reviewer over `origin/main...feat/decision-hardening-r7`,
then fix-until-clean.

## Reviewer verdict: `findings` — 1 High, 3 Med, 10 Low

### Fixed
| # | Sev | Finding | Fix |
|---|---|---|---|
| **H1** | High | I had converted 8 commands from `async fn` → sync `fn`. Tauri v2 runs sync commands **on the main thread**, so blocking `ureq` HTTP would freeze the webview (a regression vs `origin/main`). | Annotated the 8 commands `#[tauri::command(async)]` (Tauri dispatches them off the main thread). |
| **M1** | Med | R-1 was overstated: `compound_knowledge_cmd`, `create_budget_cmd`, and `knowledge_commands::run_compounder_cmd` (step 3 → `compounder_merge` → `judge_batch`) still held the shared lock across HTTP. | All three now use `crate::db::open_aux(&state.db_path)`. R-1 row updated to name them. |
| **M2** | Med | Migration 019's `CREATE UNIQUE INDEX` fails silently on a DB that already has duplicate reviews (non-fatal late loop) → dedup silently broken on upgrade. | 019 now `DELETE`s pre-existing duplicates before creating the index. |
| **M3** | Med | `assert_decision_tables` didn't check the new `input_hash` column; if migration 020 failed, usage writes no-op silently. | `assert_decision_tables` now also `prepare("SELECT input_hash FROM decision_usage LIMIT 1")`; test asserts the missing-column case errors. |
| **L1** | Low | `AppState::default()` used `:memory:`, whose aux connection opens a *different* empty DB (foot-gun). | Removed the unused `Default` impl. |
| **L2** | Low | Docs contradicted the code: stale round-1 text ("5 points deliver value"), stale daemon/R-1 diagram labels, and migration numbers `017/018` (now `019/020`). | Corrected `architecture.md` (§9, diagram, §8), `spec.md`, `review-round6.md`, `review-round7.md`, and the `decision.rs` comment. |
| **L3** | Low | R-9's "zero dead-code warnings" hid three now-uncalled helpers. | R-9 row now names `choose_agent`/`choose_entity_type`/`choose_relation_type` as kept public API. |
| **L4** | Low | `route_with_decision`'s band branching was untested; default (unset) thresholds changed routing for mid-confidence decisions. | Added 5 daemon tests (apply/review/skip/none/malformed) with a fake `httpx`. |
| **L5** | Low | Process-global `Breaker` could make `decision_request` tests order-dependent. | `global_breaker()` is inert under `cfg(test)` (threshold `u32::MAX`, cap disabled). |
| **L6** | Low | Spend-cap "cooldown" was meaningless (permanent stop). | Documented the cap as a per-process hard stop. |
| **L7** | Low | `health_probe` reports "degraded" when the breaker is open. | Accepted — an open breaker *is* a degraded state. |
| **L8** | Low | `intelligenceStore` still used inline `invoke("…")` literals. | All intel commands now go through IPC constants; baseline/snapshot regenerated. |
| **L9** | Low | `set_decision_config` hosted-URL guard was bypassable (`/api/v1`, case, spacing). | Guard now matches any `https://openrouter.ai…` origin; test covers the variants. |

## Verified gates (factual, after fixes)
| Gate | Result |
|---|---|
| `cargo test` (MSVC) | **210 lib + 42 integration = 252 passed / 0 failed** |
| `npm test` | **341 passed / 0 failed** (35 files, incl. IPC contract) |
| `npx tsc --noEmit` | clean |
| `npm run lint` | 0 errors (13 warnings) |
| `pytest local-daemon/tests/test_router.py` | **28 passed** |

## Reviewer checks that passed (no action)
Batching fallback semantics; `input_fingerprint` determinism; `enqueue_review` idempotence (with the index);
`policy_action` boundaries; verification R-16 inversion; `run_compounder_cmd` step 1→2 lock scoping;
`assert_decision_tables` no-injection; IPC command presence; `LAST_SUCCESS_MS` thread-safety; daemon audit
contents (no secrets/state); `collect_worktree_diff` in worktrees; `busy_timeout` on both connections.

**Net:** the reviewer's headline (R-1/R-2 "FIXED" not yet true) is now satisfied in code, and the doc claims
match the implementation.

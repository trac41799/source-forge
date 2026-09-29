# Review round 7 — full risk-register burn-down (R-1…R-17)

> **Superseded in part by `review-round8.md`.** An independent adversarial review of PR #9 found a
> High regression (H1: sync commands run on the Tauri main thread) and gaps in the R-1/R-15/R-14
> claims below; see round 8 for the fixes. The gate counts here are pre-round-8.

**Date:** 2026-09-26 · **Method:** SDD + TDD (RED→GREEN where a behaviour is visible)
**Outcome:** every High/Med risk open after round 6 is now fixed or formally resolved.

## Fixes
| Risk | Sev | Fix | Evidence |
|---|---|---|---|
| **R-1** | High | Network-holding commands (route, verify, health, compounder, KG, outcome, failure, handoff) open an **independent connection** via `db::open_aux`; `busy_timeout=5000` added. The shared `Mutex<Connection>` is no longer held across HTTP. | `db::tests::open_aux_opens_an_independent_connection` |
| **R-2** | High | `UreqTransport` reuses one `ureq::Agent` per timeout (keep-alive); the blocking-network commands were made **non-`async`** so Tauri runs them off the async runtime. | full suite + compile |
| **R-3** | High | Daemon applies an env-configurable accept/review band (`policy_action`) and emits a `decision_usage` audit line. | `test_router.py` +7 tests (23 pass) |
| **R-4** | Med | `choose_batch`/`judge_batch` put all questions in one request: KG types+gate = 2 requests, relations = 1, contradictions = 1 per item (was 2·N / N / N²). | `choose_batch_issues_one_request_for_many_questions`, `judge_batch_…`, `plan_entities_batches_into_two_requests`, `plan_relations_batches_into_one_request` |
| **R-8** | Med | `spec.md` R7 tightened: structured `state` is **rejected**, not truncated. | doc |
| **R-10** | Low | Circuit breaker (5 consecutive failures → 30 s open, reset on success) + $5 spend cap; short-circuits with a failure row. | `breaker_opens_after_threshold_failures`, `breaker_resets_on_success`, `cost_cap_opens_breaker` |
| **R-12** | Low | Hash-only replay: `input_hash` (FNV-1a of state+questions) stored in `decision_usage` (migration 020); raw `state` never stored. | `input_fingerprint_is_stable_and_sensitive`, `usage_row_records_input_hash_but_not_state` |
| **R-14** | Med | `apply_migrations` calls `assert_decision_tables` and errors loudly if a decision table is missing. | `missing_decision_table_fails_loudly` |
| **R-16** | Low | Verification now enqueues review-band README/secrets results (`verification.readme` / `verification.secrets`) via the testable `semantic_checks_with`. | `review_band_readme_enqueues_review` |
| **R-17** | Low | `health_probe`: backend/auth errors → "degraded" (offline only for transport/timeout); records `last_success_ms()`. | `health_probe_backend_error_is_degraded_not_offline`, `success_updates_last_success_timestamp` |

## Verified gates (factual)
| Gate | Result |
|---|---|
| `cargo +stable-x86_64-pc-windows-msvc test` | **183 lib + 42 integration = 225 passed / 0 failed** |
| `python -m pytest tests/test_router.py` | **23 passed** |
| `npm test` | 317 passed / 0 failed |
| `npx tsc --noEmit` | clean |
| `npm run lint` | 0 errors |

## Risk register after round 7
Open: **none**. R-1…R-19 are all FIXED / RESOLVED / MITIGATED / doc-corrected:
R-1 FIXED · R-2 FIXED · R-3 FIXED · R-4 FIXED · R-5 FIXED · R-6 FIXED · R-7 FIXED ·
R-8 FIXED(doc) · R-9 RESOLVED · R-10 FIXED · R-11 FIXED(doc) · R-12 FIXED · R-13 RESOLVED ·
R-14 FIXED · R-15 FIXED · R-16 FIXED · R-17 FIXED · R-18 FIXED · R-19 MITIGATED.

## Honest notes
- R-1: this is a targeted lock-scope fix (independent connection per network command), not a
  general connection pool; the remaining `Mutex<Connection>` still serialises the many
  non-network commands, which is acceptable (short, no I/O under lock).
- R-2: the blocking call itself still runs synchronously, but off the async runtime (sync
  command) and with a reused agent; a 5 s timeout bounds it.
- R-4: batching makes a single decision failure affect all items in that batch — the per-item
  LLM-type fallback is preserved, so no item is dropped.
- R-10: breaker/threshold/cap are process-global; env-overridable for tuning.

# Review round 6 — risk-register burn-down (R-5, R-7, R-9, R-15)

**Date:** 2026-09-26 · **Method:** SDD + TDD (RED→GREEN, one behavior at a time)

## Fixes
| Risk | Fix | File(s) | Test (RED → GREEN) |
|---|---|---|---|
| **R-15** | Review queue deduped: migration `019` adds `UNIQUE(consumer,question,decided_value)`; `enqueue_review` uses `INSERT OR IGNORE` and reuses the existing id | `migrations/019_decision_reviews_unique.sql`, `db.rs`, `decision.rs` | `enqueue_review_is_idempotent_for_same_decision` — RED: *UNIQUE constraint failed* on the 2nd insert |
| **R-7** | `verify_project_cmd` now feeds `collect_worktree_diff` (`git diff HEAD --no-color`, 20 000-char cap, empty for non-git) → the semantic **secrets** check actually runs | `verification.rs`, `commands.rs` | `collect_worktree_diff_empty_for_non_git_dir` — RED: `cannot find function collect_worktree_diff` |
| **R-5** | Contradiction detection now also **enqueues a review** when the model lands in the review band (transport + api-key injected for testability) | `knowledge.rs` (`detect_and_record_contradictions`) | `contradiction_in_review_band_enqueues_review` — RED (observed): `left: 0, right: 1` |
| **R-9** | Removed `#[allow(dead_code)] mod decision;` — no `decision.rs` dead-code warnings remain | `lib.rs` | build check: 0 warnings referencing `decision.rs` |

## Evidence (green)
- Rust: **169 lib + 42 integration = 211 passed / 0 failed** (+3 tests vs round 5)
- Frontend: **317 passed / 0 failed** · `tsc --noEmit` clean · lint 0 errors (14 pre-existing warnings)
- Reachability: **13 solid + 0 partial; unreachable = 0** (R-13 fully closed; #9 unblocked by R-7)

## Honest notes
- R-15: old databases created before migration `017` get the index on next start; the idempotent
  path only kicks in once the index exists (`INSERT OR IGNORE` degrades gracefully to a plain
  insert otherwise). The `db.rs` late-migration loop is still non-fatal (R-14, OPEN).
- R-7: the secrets check is reachable **for git repositories with uncommitted changes**; a non-git
  project path yields an empty diff and the check is skipped (verified by test).
- R-5: matching on the review band is conservative — a review-band contradiction is *both*
  recorded as a relation and queued for a human (unchanged recording semantics).

## Still open (next candidates)
- **R-1 / R-2** (High): DB lock held across network I/O; sync `ureq` from async commands → the
  architectural refactor (connection pool / lock-split / `spawn_blocking`).
- R-3 (High): daemon's second decision path (own config, no band, no audit).
- R-4 (Med): per-item decision calls → batch.
- R-10 (Low): circuit breaker / cost ceiling. R-14 (Med): fail loudly on migration failure.
- R-17 (Low): `health_probe` "last success" tracking.

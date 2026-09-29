# Review round 1 — adversarial review of the Jev architecture

**Date:** 2026-09-26 · **Reviewer:** adversarial subagent (read-only) · **Target:** `architecture.md` + spec + code
**Verdict returned:** `needs-revision`

The reviewer's findings and the author's disposition. `FIXED` = code/doc changed this round;
`OPEN` = accepted as real, tracked in the risk register (`architecture.md §8`); `REJECTED` = not a real issue, with reason.

| # | Sev | Finding (abbrev) | Disposition |
|---|---|---|---|
| 1 | blocker | KG `persist_extraction` has no caller → #3 dead | **OPEN** (R-13) — pre-existing orphan; wire KG pipeline |
| 2 | blocker | `run_compounder` unreachable; `run_compounder_cmd` missing → #2, #10 dead | **OPEN** (R-13) — add/register command |
| 3 | blocker | `suggest_outcome_decision`, `failure_confidence_decision` no callers → #5, #7 dead | **OPEN** (R-13) |
| 4 | blocker | `semantic_handoff_confidence` no caller → #8 dead | **OPEN** (R-6, R-13) |
| 5 | blocker | `enqueue_review` no production caller → review queue never populated | **OPEN** (R-5) |
| 6 | blocker | `backend` never selects the URL; UI has no `base_url` field | **FIXED (partial)** — `set_decision_config` now rejects `local`+hosted URL; UI field still OPEN (R-6/follow-up) |
| 7 | should-fix | DB lock held across network I/O | **OPEN** (R-1) |
| 8 | should-fix | sync `ureq` in async commands; new Agent per call | **OPEN** (R-2) |
| 9 | should-fix | early returns skip failure `record_usage` (R5) | **FIXED** — `record_failure_usage` on all failure paths |
| 10 | should-fix | `verify_project_cmd` passes empty diff → secrets noul inert | **OPEN** (R-7) |
| 11 | should-fix | daemon path: no band, no audit, env config | **OPEN** (R-3) |
| 12 | should-fix | `noul` compared to `review_threshold` as a positive gate | **OPEN** — needs `accept`-for-action + band→review semantics |
| 13 | should-fix | re-rank doesn't sort remaining suggestions | **FIXED** — full sort by confidence |
| 14 | should-fix | per-item O(N)/O(N²) calls | **OPEN** (R-4) |
| 15 | should-fix | R7 "truncate" vs structured-state reject | **ACCEPTED** (R-8) + spec wording to tighten |
| 16 | should-fix | migration 016 errors swallowed | **OPEN** (R-14) |
| 17 | should-fix | no circuit breaker / cost ceiling | **OPEN** (R-10) |
| 18 | should-fix | inconsistent verification thresholds, no enqueue | **OPEN** (R-16) |
| 19 | should-fix | `decision_reviews` no dedup key | **OPEN** (R-15) |
| 20 | nit | contract says `score` weighted-avg but code passes through | **FIXED (doc)** — pass-through is correct; Jev returns the weighted avg (verified live) |
| 21 | nit | R41 AC "equals" vs code 50/50 blend | **FIXED (spec)** — AC reworded to the blend |
| 22 | nit | `validate_choice_labels` only used by tests | **FIXED** — enforced inside `choose` |
| 23 | nit | diagram edge `POL→RV→V` fictional | **FIXED (doc v2)** — edge marked "wired: no" |
| 24 | nit | diagram node `run_compounder_cmd` doesn't exist | **FIXED (doc v2)** |
| 25 | nit | "13 integration points" overstates reachability | **FIXED (doc v2)** — status column added; 5 solid + 2 partial + 6 unreachable |
| 26 | nit | R-9 `#[allow(dead_code)]` misattributed | **FIXED (doc v2)** — it is `mod decision;` in `lib.rs` |
| 27 | nit | `health_probe` maps errors to "offline"; no last-success | **OPEN** (R-17) |
| 28 | nit | `policy_outcome` uses max confidence across answers | **OPEN** — per-answer outcome follow-up |
| 29 | nit | `update_failure_diagnosis` writes `diagnosis` twice | **FIXED** — duplicate assignment removed |
| 30 | nit | `estimate_tokens` counts JSON syntax for structured state | **OPEN** (part of R-8) |

## Author's response to the verdict
Accepted. The headline claim ("13 wired integration points") was wrong and is corrected: only
**5 points are reachable end-to-end**, 2 partial, 6 implemented-but-unreachable (pre-existing
orphan callers, exposed rather than caused by the decision layer). Seven code/doc defects were
fixed this round; the remainder are tracked in `architecture.md §8`. Re-review requested.

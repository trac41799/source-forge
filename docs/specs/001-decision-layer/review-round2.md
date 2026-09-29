# Review round 2 — re-review of the Jev architecture (v2)

**Date:** 2026-09-26 · **Reviewer:** adversarial subagent (same session) · **Target:** `architecture.md` v2 + code fixes
**Verdict returned:** `needs-revision`

Author confirmed: the round-1 fixes (#9, #13, #20, #21, #22, #23–27, #29) landed cleanly and
`record_failure_usage` does **not** double-count. New/remaining findings and disposition:

| # | Sev | Finding | Disposition |
|---|---|---|---|
| 1 | blocker | Spec/plan still list R11,R12,R21,R23,R30,R40 as deliverables though unreachable → not archivable | **FIXED** — `spec.md` gains a "Delivery status" table marking them Deferred |
| 2 | blocker | R-1 still open (DB lock across I/O) | **ACCEPTED (High, deferred)** — explicitly dispositioned; mitigation planned (pool / lock-split). Not a doc contradiction. |
| 3 | blocker | R-5 still open — enqueue not wired; R8/R50 AC false | **FIXED** — `enqueue_review` wired at routing via `policy_action`; R8/R50 ACs reworded to the wired site |
| 4 | should-fix | Sort mixed confidence vs success_rate; `choose_agent` discarded probabilities → R20 unmet | **FIXED** — new `rank_agents` ranks all agents by Jev probabilities; routing applies them |
| 5 | should-fix | Local backend unusable from UI (no `base_url` field) | **FIXED** — `base_url` input added to `DecisionPanel` (R-18) |
| 6 | should-fix | R-7 secrets noul inert (empty diff) | **OPEN** — tracked; needs wave diff threaded |
| 7 | should-fix | R-2 sync ureq in async | **ACCEPTED (deferred)** — tracked |
| 8 | should-fix | R-3 daemon second path | **ACCEPTED (deferred)** — tracked |
| 9 | should-fix | R50 semantics: noul vs review_threshold as positive gate | **FIXED** — `policy_action` (Apply/Review/Skip) used in routing + verification; the secrets question (p = has-secrets) now inverts correctly |
| 10 | should-fix | R-14 migration errors swallowed | **OPEN** — tracked |
| 11 | should-fix | Contract overstates local backends (Von is a lib) | **FIXED (doc)** — HTTP caveat added to `decision-contract.md` |
| 12 | nit | Diagram edge implied `backend` selects URL | **FIXED (doc)** — relabelled `base_url = <local>` |
| 13 | nit | `choose` ≥2-label rule could collapse duplicate agent ids | **FIXED** — candidates deduped before `rank_agents` (R-19) |
| 14 | nit | `local`+hosted guard only on write | **OPEN (R-20)** — add read-time sanitisation |
| 15 | nit | UI `base_url` gap untracked | **FIXED** — added as R-18 |
| 16 | nit | R-5 stale line ref | **FIXED** — evidence now names the symbol, not a line |

## Author's response
The v2 doc was judged factually honest; round 3 addressed the two doc-level blockers (spec
truthfulness; wiring enqueue) and the R20 ranking gap. R-1/R-2/R-3/R-6/R-7/R-10/R-14 are
**accepted, tracked risks with mitigation plans**, not contradictions — the spec no longer
claims them. Re-review requested to confirm `clear-with-open-risks`.

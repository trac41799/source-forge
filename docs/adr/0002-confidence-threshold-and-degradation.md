# ADR 0002 — Confidence thresholding and graceful degradation

**Status:** Proposed · **Date:** 2026-09-26 · **Spec:** 001-decision-layer

## Context
"Can't hallucinate" for decision models means the returned value is always inside the
offered label set — **not** that it is correct. Decision models are still wrong at some
rate, and their confidence is calibrated but imperfect on ACC's domain. Acting on a
wrong decision (routing to the wrong agent, marking a task done, merging two unrelated
knowledge items) is worse than falling back. Constitution §6 requires a defined failure
behavior for every external dependency.

## Decision
Every decision consumer applies a **configurable two-threshold policy**:
- `confidence ≥ accept` → act on the decision.
- `review ≤ confidence < accept` → route to review/human (surface in UI).
- `confidence < review` → treat as no answer; fall back (existing heuristic or LLM).
- Backend error/timeout/offline → retry bounded, then fall back and record the
  degradation. Never block an agent session.

The band defaults (accept 0.75, review 0.40) live in one config object and are tuned on
a labeled sample per workload before rollout. The policy engine and the `decision_reviews`
queue are foundation (spec R8/R50, task T13) so later tiers can enqueue review rows; the
review **UI** ships in M5 (T41/T42).

## Consequences
- **Positive:** wrong-but-confident outputs are contained; matches §6; gives the UI a
  natural "needs review" queue; degradation is observable, not silent.
- **Negative:** thresholds need per-workload calibration (an eval set is required);
  conservative defaults mean some correct decisions get reviewed first.
- **Neutral:** fallback path must be kept alive, so the old heuristics remain in code
  behind the fallback branch rather than being deleted.

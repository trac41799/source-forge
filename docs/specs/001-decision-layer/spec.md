# Spec 001 — Decision Layer

**Status:** Draft (review-hardened) · **Author:** opencode · **Date:** 2026-09-26
**Constitution:** `docs/sdlc/constitution.md` · **Review:** `sdd-review` pass applied 2026-09-26

## Goal
Introduce a first-class, swappable **decision layer** so that every place ACC currently
coerces a generative LLM into JSON-and-regex parsing, or guesses with keyword
heuristics, instead gets a **typed, calibrated decision** (choice / score / noul) from a
decision model — hosted **Jev** by default, local open-weight model (Von/Laya/Kev) as an
offline option. Raise accuracy, kill parse failures, cut token cost, and make the
product's confidence numbers real.

## Non-goals
- Replacing generative content (compounder prose, agent explanations, chat replies).
- Image/audio/video input (ACC inputs are text/PTY only).
- Training or fine-tuning a custom model (Nimble path) — future spec.
- Cloud sync of decisions or knowledge.

## Actors
- **Operator** — runs ACC, selects backend, reviews low-confidence decisions.
- **Orchestrator/wave executor** — consumes verified decisions to gate work.
- **Compounder / KG pipeline** — consumes typed categories and relations.
- **Daemon router** — routes inbound chat to an agent.
- **Decision backend** — hosted Jev (OpenRouter) or a local `/v1/systemone` server.

## External dependencies
- **OpenRouter System One API** — `POST https://openrouter.ai/api/v1/systemone`
  (base URL `https://openrouter.ai/api`); body `{model, state, questions}`; response
  `{model, answers, usage, id, provider}`. Auth: `Authorization: Bearer <OPENROUTER_API_KEY>`.
  (Alternative equivalent path: `POST https://openrouter.ai/api/alpha/decisions`.)
  Model IDs: `jev-1.13` (maps to `typesafe/jev-1.13`), `~typesafe/jev-latest`.
- **Local decision backend** — any server exposing the same `/v1/systemone` path
  (Von, Laya, Kev, Rizzo Flow), selected by `decision.base_url`.

## Requirements

### A. Foundation
- **R1** WHEN the decision layer initializes THEN it SHALL expose one interface with a
  configurable backend. *AC:* hosted POSTs `{model,state,questions}` to
  `{base_url}/v1/systemone` with base `https://openrouter.ai/api`; local POSTs the same
  shape to the configured `decision.base_url`. The existing `chat/completions` call path
  is NOT used for decisions.
- **R2** WHEN a decision is requested THEN it SHALL support `choice`, `score`, `noul`.
  *AC:* `choice`/`score` return a probability distribution summing to 1.0 (±0.001) and,
  for `score`, a `legend`; `noul` returns one value in [0,1].
- **R3** WHEN a decision response is received THEN the normalized result SHALL carry a
  `confidence` ∈ [0,1]. *AC:* `choice`/`score` use the wire `confidence`; `noul` has no
  wire confidence, so the adapter derives `confidence = noul`.
- **R4** IF the backend times out or errors THEN the system SHALL retry with bounded
  backoff and return a typed error the caller maps to its fallback. *AC:* timeout =
  `decision.timeout_ms` (default 5000); the HTTP layer is injectable/mockable so the
  timeout path is unit-testable.
- **R5** WHEN a decision call completes or fails THEN it SHALL record a `decision_usage`
  row with: `id, backend_id, model, primitives, answers, confidence, policy_outcome,
  latency_ms, input_tokens, cost, truncated, created_at`. *AC:* `state` is NEVER stored
  or logged (customer data); a usage row exists for both success and failure.
- **R6** WHEN a key is required THEN it SHALL come from env/vault and SHALL NOT appear in
  source, logs, or client bundles. *AC:* hosted key = `OPENROUTER_API_KEY` (Rust and
  daemon alike); no secret and no `state` appear in any log line or serialized payload.
- **R7** WHEN a **string** `state` exceeds the configured backend context limit THEN it SHALL be
  truncated client-side before sending. *AC:* limit = `decision.context_limit` tokens
  (default 32000, matching OpenRouter; TypeSafe allows 64k total = 32k state + longest
  question); token estimate = `ceil(chars/4)`; a truncated call sets `truncated=1`.
  **Structured (object/array) `state` is never silently truncated — an oversized structured
  `state` is rejected with a validation error** (mutilating JSON mid-key is unsafe; R-8).
- **R8** WHEN a decision in the review band belongs to a consumer with a review path
  THEN the system SHALL enqueue a `decision_reviews` row (decided value, confidence,
  consumer, created_at, resolved). *AC:* a review-band **routing** decision creates
  exactly one review row; handoff and contradiction review-band decisions also enqueue
  (R-5/R-6); duplicate `(consumer, question, decided_value)` reviews are deduplicated
  (migration 017, R-15).

### B. Tier 1 — replace brittle classification
- **R10** WHEN an inbound chat message is routed THEN the agent SHALL be a `choice` over
  configured agent IDs plus `none`. *AC:* the selected value is always one of the offered
  labels or `none`; no free-text/regex parsing of the model output occurs.
- **R11** WHEN the compounder extracts candidates THEN each item's category SHALL come
  from a `choice` over the 8 allowed categories (not LLM-emitted JSON).
- **R12** WHEN KG extraction types an entity/relation THEN `type` SHALL come from a
  `choice` over the allowed enum, gated by a `noul` "is this a real relationship".

### C. Tier 2 — calibrated confidence
- **R20** WHEN a task is routed on the Route page THEN agents SHALL be ranked by
  decision-derived confidence and it SHALL be displayed. *AC:* the returned list is sorted
  by confidence desc and each suggestion shows its confidence.
- **R21** WHEN a session outcome is inferred THEN it SHALL be a `choice` over
  {done, failed, revised, stalled} on the PTY tail. *AC:* tail ≤ 200 lines / ≤ 8k estimated
  tokens; hosted PTY egress is allowed only when the hosted backend is the selected
  backend (constitution §4); otherwise a local backend or fallback is used.
- **R22** WHEN a budget is auto-sized AND `task_complexity` is not supplied THEN
  complexity SHALL be derived by a decision (choice/score). *AC:* a missing complexity
  yields a decision-derived tier; when the backend is unavailable the prior behavior
  (caller-supplied value or the static table) is used.
- **R23** WHEN a failure diagnosis is generated THEN `confidence` SHALL be a `noul`
  ("does the suggested fix address the root cause"). *AC:* requires wiring the currently
  orphaned `update_failure_diagnosis` into the failure path; stored confidence is no
  longer hardcoded 0.0.

### D. Tier 3 — semantic verification
- **R30** WHEN a handoff is validated THEN semantic `noul` checks SHALL run (task
  completed? instruction actionable? files match?) and review-band handoffs SHALL be
  enqueued to `decision_reviews`. *AC:* a handoff whose checks fall in the review band
  produces a review row instead of `approved`.
- **R31** WHEN deployment verification runs THEN semantic `noul` checks SHALL be added
  (README explains setup? secrets present in the **wave diff**?) **alongside** the
  deterministic checks. *AC:* the semantic secret check inspects the wave's collected
  diff (`HandoffEnvelope.diff_preview` / changed files), not the project source tree.

### E. Tier 4 — flywheel quality
- **R40** WHEN two knowledge items are compared THEN same-insight/contradiction SHALL be
  a `noul` and the relation type a `choice`. *AC:* replaces the current
  `jaccard_similarity >= 0.5` heuristic (`knowledge.rs:1008,1036`); fallback retains it.
- **R41** WHEN items merge THEN confidence SHALL be decision-derived, not the fabricated
  recency-weighted value alone (`knowledge.rs:341`). *AC:* merged-item confidence is a
  50/50 blend of the decision same-insight probability and the recency heuristic; the
  recency-only formula is used when the backend is unavailable.

### F. Policy & cross-cutting
- **R50** IF confidence falls in the review band AND the consumer has a review path
  THEN the system SHALL enqueue review rather than act silently; consumers without a
  review path SHALL fall back (R51). *AC:* band = [review_threshold, accept_threshold);
  default [0.40, 0.75); a routing decision at 0.5 is enqueued to `decision_reviews`.
  (Wired site: routing — handoff/contradiction deferred.)
- **R51** WHEN no backend is available (offline, timeout, no model) THEN each consumer
  SHALL fall back to its documented prior behavior and record the degradation — never
  block an agent session. *AC:* the offline fallback matrix
  (`implementation-details/decision-contract.md`) passes as an integration test.
- **R52** WHEN configuring backends THEN Settings SHALL allow hosted/local selection and
  display health. *AC:* health check = a live probe (empty `state`, one `noul` question
  "is this endpoint reachable") carrying no real data, or last success < 10 min; shows
  `healthy | degraded | offline` plus the resolved backend/model id.
- **R53** WHEN a decision backend is selected THEN an eval harness SHALL be runnable to
  compare backends on a labeled sample. *AC:* `scripts/decision-eval` runs against hosted
  and a local backend and writes an accuracy/calibration/latency report.

## Edge cases
1. Confidence in review band → R50 (enqueue review).
2. Backend unreachable/timeout → R4, R51.
3. `state` exceeds context → R7.
4. Empty/duplicate label set → rejected at construction; a `choice` with <2 distinct
   labels fails fast (owner T9, `decision.rs`).
5. Local model not downloaded → R51 fallback + surfaced in R52 health (owner T37/T38).
6. Offline with a local backend and model present → must succeed (constitution §4).
7. Noul near 0.5 → treated as uncertain; review band catches it (R50).

## Delivery status (2026-09-26)
Reachability is authoritative in `architecture.md §5`.

| State | Requirements |
|---|---|
| **Delivered (reachable)** | R1–R8, R10–R12, R20–R23, R30, R40, R41, R50, R52, R53 |
| **Partial** | none |
| **Deferred** | none — all integration points wired (`architecture.md §5`); all High risks mitigated (`review-round7.md`) |

Deferred items are tracked as R-13 in `architecture.md §8`; no requirement above is claimed
as delivered unless its caller is reachable from a registered command or the daemon.

## Out of scope
- djev / OpenJev (image/multimodal inputs — n/a).
- Training a bespoke decision model (Nimble/NanoJev) — future spec.
- Decision calls for generative writing.

## Assumptions
- An `OPENROUTER_API_KEY` exists (ACC already calls OpenRouter) **or** a local
  `/v1/systemone` server is configured.
- The `/v1/systemone` request/response contract is stable across TypeSafe Jev, Von,
  Laya, Kev, Rizzo Flow (verified against OpenRouter's published docs, 2026-09-26).

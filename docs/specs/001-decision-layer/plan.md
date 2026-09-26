# Plan 001 — Decision Layer

**Spec:** `spec.md` (review-hardened) · **Constitution:** `docs/sdlc/constitution.md`
**ADRs:** [0001 adapter](../adr/0001-decision-model-adapter.md) · [0002 thresholds](../adr/0002-confidence-threshold-and-degradation.md)

## Approach
Add `src-tauri/src/decision.rs`: one `decision()` entry point that POSTs a
`/v1/systemone` body (`{model, state, questions}`) to a configurable base URL
(hosted default `https://openrouter.ai/api` → `…/api/v1/systemone`; local
`http://127.0.0.1:PORT`) and returns a normalized `{answers, confidence, usage}`.
The existing `intelligence.rs` `ureq` path is reused for **HTTP/auth/retry mechanics
only** — its `chat/completions` body/response is NOT reused. Expose a new `"decision"`
mode on `IntelligenceRequest`; swap one call site at a time; each consumer keeps its
prior behavior as the fallback branch (ADR 0002). The daemon calls the same endpoint
**directly over HTTP** (Python `requests`/`typesafe-sdk`) — no Tauri IPC bridge, so
offline/local works. Backend, thresholds, context limit and timeout are config; Settings
gains selection + health; a `decision_usage` audit table and a `decision_reviews` queue
are added in the foundation so later tiers can enqueue.

## Rejected alternative
- *Reuse the existing `chat/completions` call* — it returns `choices[].message.content`,
  not `{answers}`; Jev is served at `/v1/systemone` (verified, OpenRouter docs 2026-09-26).
- *Frontend calls Jev directly* — leaks keys to the client bundle (violates §4, R6).
- *Daemon calls a Tauri command* — Tauri commands are webview IPC; the daemon is a
  standalone queue poller with no host bridge (verified: `local-daemon/main.py`).
- *Keep forcing JSON out of the LLM* — the brittle status quo the spec removes.
- *Commit to hosted Jev only* — violates §4 local-first and creates lock-in (ADR 0001).

## Architecture deltas
- New module `src-tauri/src/decision.rs` + `"decision"` mode; new tables
  `decision_usage` (migration 016) and `decision_reviews`.
- Config (stored in `user_preferences`, no new config migration): `decision.backend`
  (hosted|local), `decision.base_url`, `decision.model`, `decision.accept_threshold`,
  `decision.review_threshold`, `decision.context_limit`, `decision.timeout_ms`.
- Call sites replaced (fallback retained): daemon router, compounder category
  (`knowledge.rs`), KG typing (`kg_extraction.rs`), task routing (`routing.rs`), outcome
  (`intelligence.rs`, wires the orphaned path), budget (`budget.rs`), failure diagnosis
  (`intelligence.rs`, wires `update_failure_diagnosis`), handoff (`handoff_parser.rs`),
  deployment (`verification.rs`, on the wave diff), contradiction/merge (`knowledge.rs`).
- Settings UI: backend selection + probe health + thresholds; review-queue UI.

## Risks & mitigations
- **Vendor/model quality drift** → adapter + local backends + eval harness (T44/R53).
- **Miscalibration on ACC's domain** → threshold band (ADR 0002) + per-workload labeled set.
- **Context overflow** → truncate to `decision.context_limit` (R7); PTY already capped (R21).
- **Latency in hot loops** → async, 70–500 ms budget.
- **Privacy egress** → never log/store `state` (R5/R6); hosted PTY only when hosted selected (R21).
- **Silent wrong decisions** → `decision_usage` audit + `decision_reviews` queue (R5, R8, R50).

## Milestones (each independently shippable, suite stays green)
- **M0** Foundation: adapter, config, usage+reviews tables, threshold policy, fallback core. *(R1–R8, R50, R51)*
- **M1** Tier 1: router (HTTP client), compounder category, KG typing. *(R10–R12)*
- **M2** Tier 2: routing, outcome (wire orphaned path), budget, failure diagnosis. *(R20–R23)*
- **M3** Tier 3: handoff + deployment semantic verification. *(R30–R31)*
- **M4** Tier 4: contradiction + merge confidence. *(R40–R41)*
- **M5** UX: health probe, Settings, review queue. *(R8, R52)*
- **M6** Polish: eval harness, operator docs. *(R53)*

Threshold policy lives in **M0** (not M5) so M3 review routing depends only on M0.
Detail (payloads, backends, file:line mapping) → `implementation-details/`.

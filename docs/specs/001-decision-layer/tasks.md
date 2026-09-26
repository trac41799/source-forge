# Tasks 001 — Decision Layer

**Spec:** `spec.md` · **Plan:** `plan.md`
**Baseline (2026-09-26, after baseline repair — see `docs/specs/001-decision-layer/baseline-repair.md`):**
- Frontend `npm test`: **314 passed / 0 failed** (29 files).
- Rust: `$env:Path = "C:\Users\mrtra\tools\mingw64\bin;$env:Path"; cargo +stable-x86_64-pc-windows-msvc test`
  → **lib 142 passed / 0 failed · integration_tests 42 passed / 0 failed · main 0**
  (total **184 passed / 0 failed**). MUST use MSVC explicitly (the directory override defaults to
  GNU, which cannot launch the test harness on this box) with `windres`/`dlltool` on PATH.
  **No task may increase either count.**

Format: `T<n> [P] Title — files — acceptance: spec.md §N — test`.

---

## M0 — Foundation (R1–R8, R50, R51)
- **T1** Failing test: migration 016 creates
  `decision_usage` + `decision_reviews` (+ `decision_config`); config defaults load
  (backend, thresholds, context_limit=32000, timeout_ms=5000), set/get round-trips —
  `src-tauri/migrations/016_decision_usage.sql`,
  `src-tauri/src/decision.rs`, `src-tauri/src/db.rs`, `src-tauri/src/lib.rs` — §A (R5, R8) — `decision.rs::tests::migrations_and_config` (**red**)
- **T2** Implement migration + config plumbing (single-row `decision_config`; no `ALTER`/prefs columns) — same files — §A (R1, R5, R8) — T1 green
- **T3** [P] Failing test: parse a recorded Jev fixture — `choice` (probabilities sum≈1),
  `score` (with `legend`), `noul` (derive `confidence = noul`) — `src-tauri/src/decision.rs`,
  `src-tauri/tests/fixtures/jev_*.json` — §A (R2, R3) — `decision.rs::tests::parses_jev_fixture` (**red**)
- **T4** Implement normalized result struct + validation (distinct labels ≥2, prob sum ±0.001) — `decision.rs` — §A (R2, R3) — T3 green
- **T5** [P] Failing test: backend selector builds `{hosted base}/v1/systemone` vs
  `{decision.base_url}/v1/systemone` (NOT `chat/completions`) — `decision.rs` — §A (R1) — `decision.rs::tests::selects_backend_url` (**red**)
- **T6** Implement request builder + transport (base URL injectable → mockable HTTP) — `decision.rs` — §A (R1) — T5 green
- **T7** [P] Failing test: hosted key read from `OPENROUTER_API_KEY`; no secret **and no
  `state`** in serialized payloads or logs — `decision.rs` — §A (R6) — `decision.rs::tests::no_secret_or_state_in_output` (**red**)
- **T8** [P] Failing test: oversized `state` truncated to `context_limit` (est. `ceil(chars/4)`) and
  flagged `truncated=1`; empty/duplicate label set rejected — `decision.rs` — §A (R7), edge #4 — `decision.rs::tests::truncates_oversized_and_rejects_empty_labels` (**red**)
- **T9** Implement truncation + label validation — `decision.rs` — §A (R7) — T8 green
- **T10** Failing test: timeout (`decision.timeout_ms`) + bounded retry returns typed error
  and caller invokes its fallback (mockable HTTP) — `decision.rs` — §A (R4), §F (R51) — `decision.rs::tests::timeout_falls_back` (**red**)
- **T11** Implement timeout/retry/typed-error — `decision.rs` — §A (R4) — T10 green
- **T12** Failing test: threshold policy classifies accept / review / fallback from bands — `decision.rs` — §F (R50) — `decision.rs::tests::threshold_policy` (**red**)
- **T13** Implement threshold policy + `decision_reviews` enqueue — `decision.rs`, `preferences.rs` — §A (R8), §F (R50) — T12 green
- **T14** Wire a `"decision"` mode dispatcher (`dispatch_mode`; `IntelligenceRequest` has no
  existing mode-match, so the dispatcher lives with the decision layer) — `src-tauri/src/decision.rs` — §A (R1) — `decision.rs::tests::decision_mode_dispatch`

## M1 — Tier 1: replace brittle classification (R10–R12)
- **T15** [P] Failing test: daemon router returns a `choice` label + confidence via the
  HTTP decision endpoint, no regex parse — `local-daemon/tests/test_router.py` — §B (R10) — `test_router.py::test_route_uses_decision_choice` (**red**)
- **T16** Implement daemon decision client (`requests` to `{base}/v1/systemone`; key from
  env, never bundled) + prompt fallback — `local-daemon/router.py`, `local-daemon/requirements.txt` — §B,§F (R10, R51, R6) — T15 green
- **T17** [P] Failing test: compounder category is a `choice` over the 8 categories — `src-tauri/src/knowledge.rs` — §B (R11) — `knowledge.rs::tests::category_from_decision` (**red**)
- **T18** Implement category decision + retain LLM content writing + fallback — `knowledge.rs`, `intelligence.rs` — §B,§F (R11, R51) — T17 green
- **T19** [P] Failing test: KG entity/relation `type` from `choice`, creation gated by `noul` — `src-tauri/src/kg_extraction.rs` — §B (R12) — `kg_extraction.rs::tests::types_from_decision` (**red**)
- **T20** Implement KG typing decision + fallback — `kg_extraction.rs` — §B,§F (R12, R51) — T19 green

## M2 — Tier 2: calibrated confidence (R20–R23)
- **T21** [P] Failing test: `route_task` ranks agents by decision confidence — `src-tauri/src/routing.rs` — §C (R20) — `routing.rs::tests::ranks_by_decision_confidence` (**red**)
- **T22** Implement decision routing + `Route.tsx` confidence display — `routing.rs`, `src/pages/Route.tsx`, `src/stores/*` — §C (R20) — T21 green
- **T23** [P] Failing test: outcome is a `choice` over {done,failed,revised,stalled}, with the
  orphaned path wired to real outcome recording — `src-tauri/src/intelligence.rs` — §C (R21) — `intelligence.rs::tests::outcome_decision` (**red**)
- **T24** Implement outcome decision + wire outcome recording — `intelligence.rs`, `commands.rs` — §C,§F (R21, R51) — T23 green
- **T25** [P] Failing test: missing `task_complexity` derives a tier via decision — `src-tauri/src/budget.rs` — §C (R22) — `budget.rs::tests::complexity_from_decision` (**red**)
- **T26** Implement complexity decision + fallback to supplied value/table — `budget.rs` — §C,§F (R22, R51) — T25 green
- **T27** [P] Failing test: failure `confidence` from `noul`, with `update_failure_diagnosis`
  wired into the failure path — `src-tauri/src/intelligence.rs` — §C (R23) — `intelligence.rs::tests::failure_confidence_from_noul` (**red**)
- **T28** Implement failure-confidence decision + wire diagnosis — `intelligence.rs` — §C (R23) — T27 green

## M3 — Tier 3: semantic verification (R30–R31)
- **T29** [P] Failing test: handoff semantic `noul` checks; review-band handoff → `decision_reviews` — `src-tauri/src/handoff_parser.rs`, `src-tauri/src/orchestrator.rs` — §D (R30) — `handoff_parser.rs::tests::semantic_checks` (**red**)
- **T30** Implement semantic handoff checks + review enqueue — same — §D,§F (R30, R50) — T29 green
- **T31** [P] Failing test: expert `noul` README/secret checks on the **wave diff**, alongside deterministic checks — `src-tauri/src/verification.rs` — §D (R31) — `verification.rs::tests::semantic_readme_check` (**red**)
- **T32** Implement semantic deployment checks (deterministic checks unchanged) — `verification.rs` — §D (R31) — T31 green

## M4 — Tier 4: flywheel quality (R40–R41)
- **T33** [P] Failing test: contradiction by `noul`, relation type by `choice` (replacing `jaccard_similarity`) — `src-tauri/src/knowledge.rs` — §E (R40) — `knowledge.rs::tests::contradiction_decision` (**red**)
- **T34** Implement contradiction/relation decisions + fallback to jaccard — `knowledge.rs` — §E,§F (R40, R51) — T33 green
- **T35** [P] Failing test: merge confidence is decision-derived — `knowledge.rs` — §E (R41) — `knowledge.rs::tests::merge_confidence_decision` (**red**)
- **T36** Implement decision-based merge confidence + fallback formula — `knowledge.rs` — §E (R41) — T35 green

## M5 — UX (R8, R52)
- **T37** [P] Failing test: health probe (empty `state`, one `noul`, no real data) → `healthy|degraded|offline` — `decision.rs` — §F (R52) — `decision.rs::tests::health_probe` (**red**)
- **T38** Implement health probe — `decision.rs` — §F (R52) — T37 green
- **T39** [P] Failing test: Settings shows backend selection + health + thresholds — `src/pages/Settings.tsx` — §F (R52) — `src/__tests__/pages/Settings.test.tsx::decision backend panel` (**red**)
- **T40** Implement Settings decision panel — `Settings.tsx`, `src/lib/commands/*` — §F (R52) — T39 green
- **T41** [P] Failing test: review queue lists `decision_reviews` and resolves — `src/pages/*`, `src/stores/*` — §A (R8) — `src/__tests__/pages/DecisionReviews.test.tsx` (**red**)
- **T42** Implement review-queue UI + resolve command — §A (R8) — T41 green
- **T43** Offline fallback matrix integration test, one case per consumer — `src-tauri/tests/decision_fallback.rs` — §F (R51) — `cargo test --test decision_fallback`

## M6 — Polish (R53)
- **T44** [P] Eval harness: labeled sample per workload, compare hosted vs local backend, record accuracy/calibration/latency — `scripts/decision-eval/`, `docs/specs/001-decision-layer/eval-results.md` — §F (R53) — harness runs, report committed
- **T45** [P] Operator docs: install a local backend (Von/Laya/Kev), tune thresholds, offline notes — `docs/specs/001-decision-layer/operator-guide.md` — §F (R52) — doc review
- **T46** Verify baselines unchanged: `npm test` ≤1 fail; `cargo test` green — all — checklist — `npm test`, `cargo test`

> Split-on-execution note: T2, T14, T22, T24, T42 touch multiple files; split into
> one-file test+impl pairs if any exceeds ~30 min of agent work.

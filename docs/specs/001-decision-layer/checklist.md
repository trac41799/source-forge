# Requirements checklist — 001 decision-layer

Verified 2026-09-26 against `spec.md`, `plan.md`, `tasks.md`, constitution.
Adversarial `sdd-review` pass applied (7 blockers + 21 should-fix + 7 nits — all resolved
or explicitly deferred). An unchecked item blocks implementation.

## Completeness
1. [x] Every requirement has an actor+trigger — EARS R1–R53 each name trigger + system behavior.
2. [x] One testable AC per requirement — R20–R23 now carry explicit observable ACs.
3. [x] Non-goals + out-of-scope explicit and dated — spec "Non-goals"/"Out of scope", 2026-09-26.
4. [x] All `[NEEDS CLARIFICATION]` resolved/deferred — duplicate-label policy resolved (T9, edge #4).
5. [x] External deps named — OpenRouter `/v1/systemone`, TypeSafe Jev, local Von/Laya/Kev/Rizzo/NanoJev/SemIf.
6. [x] Failure behavior per dep — R4 (timeout_ms=5000 + bounded retry), R51 + fallback matrix.

## Consistency
7. [x] No requirement contradiction — Noul confidence now uniformly derived (R3); R50 vs R51 scoped by review path.
8. [x] No constitution contradiction — reused infra (§3), local-first (§4), fallback (§6); §2 reworded from "already" to "planned".
9. [x] Terms consistent — `choice`/`score`/`noul`/`confidence`/`backend` used one way; endpoint spec pinned once (ADR 0001).
10. [x] Plan covers every requirement — M0→R1–8/R50/R51, M1→R10–12, M2→R20–23, M3→R30–31, M4→R40–41, M5→R8/R52, M6→R53.
11. [x] Every task acceptance pointer resolves — pointers reference real §A–§F sections.

## Testability
12. [x] AC observable externally — labels ∈ offered set, prob sum ±0.001, `decision_usage` row, review row.
13. [x] Deterministic criteria — context_limit=32000, timeout_ms=5000, bands [0.40,0.75), tail ≤200 lines.
14. [x] Each AC = one automated test — every task names one test (Rust `#[cfg(test)]`, `test_router.py`, integration test).
15. [x] Edge cases have criteria or "not tested" — all 7 map to R4/7/50/51/52 or T9; none untested.

## Plan quality
16. [x] Approach + rejected alternative — plan lists 5 rejected alternatives incl. `chat/completions` and Tauri-IPC.
17. [x] No impl detail in plan — payloads/backends/file:line live in `implementation-details/`.
18. [x] Risks have mitigations — each risk paired with a concrete mitigation.
19. [x] Milestones independently shippable — threshold policy moved to M0 so M3 no longer depends on M5.

## Tasks quality
20. [x] Dependency-ordered — Setup(M0) → tiers M1–M4 → UX M5 → polish M6.
21. [x] Independent tasks marked `[P]` — 20 `[P]` tasks.
22. [x] First task of each behavior is its failing test — `(red)` precedes each impl task.
23. [x] Every task names files + test — all rows include files and a test path/name.
24. [x] Baseline recorded — frontend 308/1/0; **Rust baseline recorded by T1 before T3**.
25. [x] No task > ~30 min — T2/T14/T22/T24/T42 flagged to split on execution.

## Review findings resolution (2026-09-26)
- **Blockers:** (1) endpoint corrected to `/v1/systemone` [R1/ADR0001/contract]; (2) daemon uses direct HTTP, no Tauri IPC [plan/T15-T16]; (3) Noul confidence derived [R3/contract]; (4) `context_limit`+`truncated`+counting unit added [R5/R7/T1/T8]; (5) contract pinned [ADR0001]; (6) `decision_reviews` queue in M0 [R8/T13]; (7) threshold policy moved to M0 [plan/tasks].
- **Should-fix:** orphaned-code claims corrected (`suggest_outcome`, `extract_pty_context`, `update_failure_diagnosis`, N/c) [tier-mapping]; jaccard vs shared-word corrected; `decision_usage` gains answers/confidence/policy_outcome [R5]; Rust baseline task added [T1]; multi-file tasks flagged to split; R22 premise corrected (supplied string, not keywords); T7 truncate-not-reject; R22/R23 ACs added; PTY tail size + hosted-egress opt-in [R21]; daemon key contract [R6/T16]; no-`state`-logging [R5/R6/contract]; probability-sum assertion [R2/T4]; fallback line refs corrected; matrix test moved to integration test [T43]; R23 diagnosis wiring [T27].
- **Nits:** score semantics + `legend` documented [contract]; ADR eval-harness reference reconciled with T44/R53; config key list completed; duplicate-label test assigned (T9); "cache repeated states" claim removed; constitution §2 reworded.

## Gate
- [x] High-risk spec → adversarial `sdd-review` pass complete; all findings applied above.

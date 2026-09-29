# Review round 4 — R-13 build (wire the orphan integration points)

**Date:** 2026-09-26 · **Method:** SDD + TDD (RED→GREEN) · **Scope:** close R-13 for the testable points

## What was built (TDD)
| Item | Code | Test (RED→GREEN) | New command |
|---|---|---|---|
| #5 outcome | `intelligence::decide_outcome` (pure) + `suggest_outcome_decision` refactor | `decide_outcome_uses_decision` | `infer_outcome_cmd` (+ frontend `inferOutcome`) |
| #7 failure | `intelligence::decide_failure_confidence` (pure) + `failure_confidence_decision` refactor | `decide_failure_confidence_uses_noul` | `diagnose_failure_cmd` (+ frontend `diagnoseFailure`) |
| #8 handoff | `parse_handoff_file_cmd` now State-aware; adds `semantic_handoff_confidence` + review-band enqueue | covered by `decision::policy_action_boundaries` | `parse_handoff_file_cmd` (updated) |

## Evidence (all green)
- Rust: `cargo +stable-x86_64-pc-windows-msvc test` → **164 lib + 42 integration = 206 passed / 0 failed**
- Frontend: `npm test` → **317 passed / 0 failed** (30 files) · `npx tsc --noEmit` clean · `npm run lint` 0 errors
- New tests: 2 Rust (`intelligence::tests::*`) + 2 frontend (`intelligenceStore` infer/diagnose)

## Effect on reachability
- Before: 6 solid + 1 partial reachable; 6 unreachable
- After: **9 solid + 1 partial reachable; 3 unreachable** (#2 compounder, #3 KG, #10 contradiction)
- R-6 (handoff unreachable) → **FIXED**

## Why #2/#3/#10 remain
They live inside `async` functions (`run_compounder`, KG extraction) that must hold the
`std::sync::Mutex<Connection>` guard across `.await` — a compile-blocking pattern. Adopting them
requires the R-1/R-2 fix (connection pool / lock-split / `spawn_blocking`). Deliberately **not**
hacked around; tracked as R-13 residual + R-1/R-2.

# Review round 5 — R-13 completion (wire #2, #3, #10)

**Date:** 2026-09-26 · **Method:** SDD + TDD (RED→GREEN) · **Result:** all six formerly-dead integration points reachable

## What was built
| Point | Change | Command | Test (RED→GREEN) |
|---|---|---|---|
| **#2 compounder** | `knowledge::run_compounder` made **synchronous** (new blocking `intelligence::invoke_openrouter_blocking`) so a sync command can hold the DB guard | `run_compounder_cmd` (fixes the broken Compounder Run button) | `run_compounder_no_events_returns_empty_without_llm` |
| **#10 contradiction** | reachable via the compounder path | (same) | (same) |
| **#3 KG typing** | sync `kg_extraction::run_llm_extraction_blocking` | `run_kg_extraction_cmd` | `persist_empty_extraction_is_noop` |

RED evidence: `no method named unwrap found for opaque type impl Future` (run_compounder was async) →
GREEN after the sync refactor.

## Evidence (green)
- Rust: **166 lib + 42 integration = 208 passed / 0 failed**
- Frontend: 317 passed / 0 failed · tsc clean · lint 0 errors (unchanged this round)
- New tests: +2 Rust

## Effect
- Reachable: **12 solid + 1 partial (#9)**; **unreachable: 0** → **R-13 RESOLVED**
- `run_compounder_cmd` also fixes a genuinely broken feature (the frontend already called it).

## Trade-off / honest note
Making the compounder/KG paths synchronous means they now hold the DB `MutexGuard` across a
**blocking** HTTP call — this widens the surface of accepted risk **R-1** (DB stall during the
call). Chosen deliberately over the large R-1/R-2 refactor (connection pool / lock-split), which
remains the recommended next architectural work; documented in `architecture.md §8`.

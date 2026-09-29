# Review round 9 — R31 (wave-diff semantic checks) + R51 (fallback matrix test)

**Date:** 2026-09-29 · **Method:** SDD + TDD; adversarial review over `origin/main...HEAD`

## Why
A post-merge audit found the "Jev complete" claim slightly overstated (`review-round8` left R31 and
R51's AC unmet):

- **R31** — the semantic secrets check ran only in `verify_project_cmd` on a `git diff HEAD` of the
  **project tree**, which the AC explicitly forbids, and the **wave-verification path ran no semantic
  checks at all**. `finalize_wave_with_verify` was dead code.
- **R51** — the offline-fallback matrix had no integration test.

## What changed
| Item | Change | Test (RED→GREEN) |
|---|---|---|
| **R31** | New `wave_executor::collect_wave_diff` (each agent worktree's `git diff HEAD` **plus untracked files** — a new `.env` is exactly where a secret hides); `finalize_wave_with_verify` made live and wired into `verify_and_finalize_wave_cmd` (deterministic + semantic checks on the wave diff); `verify_project_cmd` no longer feeds a project-tree diff; removed `verification::collect_worktree_diff` | `collect_wave_diff_gathers_agent_worktree_changes` (RED: untracked `secret.env` absent, fixed by including untracked files), `collect_wave_diff_empty_without_agents`, `wave_diff_review_band_checks_secrets` |
| **R51** | New `decision_fallback_tests` integration module driving the injectable transport seam with an always-failing backend | `core_returns_unavailable_when_backend_down`, `deployment_verify_falls_back_to_deterministic_only`, `kg_typing_falls_back_to_llm_types`, `contradiction_falls_back_to_jaccard` (RED first: jaccard fixture below 0.5) |
| — | `plan_entities` / `detect_and_record_contradictions` made `pub(crate)` for the test seam | — |

## Verified gates (factual)
| Gate | Result |
|---|---|
| `cargo test` (MSVC) | **219 lib + 42 integration = 261 passed / 0 failed** |
| `npm test` / `tsc` / `lint` | see PR |
| `pytest local-daemon/tests/test_router.py` | 28 passed |

## Honest notes
- R51's integration test covers the rows whose consumers expose the transport seam (core, deployment
  verify, KG typing, contradiction/merge). Rows whose consumers build their transport/config from the
  environment (outcome, budget, failure confidence, handoff, route_task, daemon) remain covered by
  those modules' own unit tests — stated in the test-file header, not claimed as covered here.
- R31's diff source is the agents' own worktrees (the wave's changes), which is what the AC wants;
  untracked files are included because secrets often land in new files.

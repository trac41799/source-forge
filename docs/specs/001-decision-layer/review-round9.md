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
| **R31** | New `wave_executor::collect_wave_diff` (each agent worktree's `git diff HEAD` plus untracked files **and gitignored secret-named files** — a committed `.gitignore` usually hides `.env`); bounded 20 000 chars / 4 096 per file / 200 files, symlinks skipped, no header for change-free worktrees; `finalize_wave_with_verify` made live and wired into `verify_and_finalize_wave_cmd` (deterministic + semantic checks on the wave diff); `verify_project_cmd` no longer feeds a project-tree diff; removed `verification::collect_worktree_diff` | `collect_wave_diff_gathers_agent_worktree_changes` (RED: gitignored `secret.env` absent), `collect_wave_diff_skips_worktrees_without_changes`, `collect_wave_diff_empty_without_agents`, `wave_diff_review_band_checks_secrets` |
| **R51** | New `decision_fallback_tests` `#[cfg(test)]` module driving the injectable transport seam with an always-failing backend | `core_returns_unavailable_when_backend_down`, `deployment_verify_falls_back_to_deterministic_only`, `kg_typing_falls_back_to_llm_types`, `contradiction_falls_back_to_jaccard` (RED first: jaccard fixture below 0.5), `fallback_records_usage_degradation` |
| — | `plan_entities` / `detect_and_record_contradictions` made `pub(crate)` for the test seam | — |

## Verified gates (factual)
| Gate | Result |
|---|---|
| `cargo test` (MSVC) | **219 lib + 42 integration = 261 passed / 0 failed** |
| `npm test` / `tsc` / `lint` | see PR |
| `pytest local-daemon/tests/test_router.py` | 28 passed |

## Honest notes
- R51's tests cover the rows whose consumers expose the transport seam (core, deployment
  verify, KG typing, contradiction). Rows whose consumers build their transport/config from
  the environment (outcome, budget, failure confidence, handoff, route_task, daemon) remain
  covered by those modules' own unit tests — stated in the test-file header, not claimed as
  covered here. `spec.md`'s AC says the same.
- R31's diff source is the agents' own worktrees (the wave's changes), which is what the AC wants;
  untracked **and gitignored secret-named** files are included because secrets often land in new
  files that `.gitignore` hides. Known limits: `git diff HEAD` misses changes an agent already
  **committed**, and the collected diff (including secret-named file bodies) is **egressed** to the
  configured backend to be judged — documented in `decision-contract.md §Privacy`.
- Adversarial review (round 9) found and this PR fixed: unbounded diff accumulation (now bounded
  incrementally), the `--exclude-standard` gap that dropped gitignored `.env` (now a targeted
  ignored-secret pass), spurious secrets calls for change-free worktrees, symlink traversal, and
  spec/coverage overstatement.

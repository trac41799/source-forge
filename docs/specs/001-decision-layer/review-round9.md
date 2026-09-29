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
| `cargo test` (MSVC) | **222 lib + 42 integration = 264 passed / 0 failed** |
| `npm test` / `tsc` / `lint` | 341 passed; tsc clean; 0 lint errors |
| `pytest local-daemon/tests/test_router.py` | 28 passed |

## Honest notes
- R51's tests cover the rows whose consumers expose the transport seam (core, deployment
  verify, KG typing, contradiction). Rows whose consumers build their transport/config from
  the environment (outcome, budget, failure confidence, handoff, route_task, daemon) rely on
  their **implementation** fallbacks and are verified by inspection — **not** test-covered;
  `spec.md`'s AC and the test-file header say the same.
- R31's diff source is the agents' own worktrees (the wave's changes), which is what the AC wants;
  untracked **and gitignored secret-named** files are included (pathspec-limited) because secrets
  often land in new files that `.gitignore` hides. Known limits: `git diff HEAD` misses changes an
  agent already **committed**; the collected diff (including secret-named file bodies) is
  **egressed** to the configured backend to be judged — documented in `decision-contract.md §Privacy`.
- Accepted (not fixed): the wave-verify command runs blocking `git`/`ureq` on the async runtime
  (same class as accepted risk R-2, and pre-existing for `verify_project`). A `spawn_blocking`
  refactor is tracked with R-2, not here.

## Adversarial review (round 9 — two passes)
Fixed after review: unbounded diff accumulation (now streamed/bounded incrementally); the
`--exclude-standard` gap that dropped gitignored `.env` (now a pathspec-limited ignored-secret
pass); the file cap starving the secret filter (filter runs before the count); spurious secrets
calls for change-free worktrees; symlink traversal; and spec/coverage overstatement.

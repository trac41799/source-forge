# Implementation detail — integration-point mapping (current → decision)

Line refs verified 2026-09-26. `N/c` = no callers found (orphaned code).

## Tier 1 — replace brittle LLM-prompt-then-parse
| # | Location | Current (brittle) | Decision call |
|---|---|---|---|
| 1 | `local-daemon/router.py:11` (`ROUTER_SYSTEM_PROMPT`), `:50` (`parse_router_response`), `:64` (`route`) | "output ONLY the id … no punctuation", then regex-scrape; returns `None` on any drift | `choice` over agent IDs + `none` (§R10) |
| 2 | `src-tauri/src/knowledge.rs:782` (`build_compounder_prompt`), `:808` (`parse_compounder_response`) | asks LLM for JSON array with `category`, strips fences, hunts brackets, silent-empty on failure | `choice` over the 8 categories; `noul` "is this a reusable knowledge item" (§R11) |
| 3 | `src-tauri/src/kg_extraction.rs:28` (`build_extraction_prompt`), `:46` (`parse_extraction_response`) | LLM emits entity/relation JSON; bracket-scrape | `choice` for `type` enums, `noul` "is this a real relationship" (§R12) |

## Tier 2 — calibrated confidence
| # | Location | Current | Decision call |
|---|---|---|---|
| 4 | `src-tauri/src/routing.rs:50` (`route_task`) | keyword type-detection; fallback = hardcoded defaults vec (`:93-111`) with `confidence: 0.5` but `success_rate: 0.0` | `choice` over agents + `score` complexity (§R20) |
| 5 | `src-tauri/src/intelligence.rs:597` (`suggest_outcome`) | keyword heuristic; **N/c — orphaned**, never wired to `record_outcome` | `choice` {done,failed,revised,stalled} + `noul`; **also wire it** (§R21) |
| 6 | `src-tauri/src/budget.rs:69` (`resolve_budget_total`) | maps a **caller-supplied** `task_complexity` string → fixed token table; no keyword inference | derive complexity via decision when not supplied (§R22) |
| 7 | `src-tauri/src/intelligence.rs:163` (`update_failure_diagnosis`) | **N/c — orphaned**; `create_failure_analysis:136` stores `confidence: 0.0` | wire diagnosis generation + `noul` "does the fix address the root cause" (§R23) |

## Tier 3 — semantic verification (new capability)
| # | Location | Current | Decision call |
|---|---|---|---|
| 8 | `src-tauri/src/handoff_parser.rs:26` + `orchestrator::validate_handoff_schema` | sections-exist only | `noul`: task completed? instruction actionable? files match? → review queue (§R30) |
| 9 | `src-tauri/src/verification.rs:93` (`verify_project`), `:200` README length/substring, `:638` secrets (scans **project source**, not a diff) | deterministic only | expert `noul` README/secret checks on the **wave diff** (§R31) |

## Tier 4 — flywheel quality
| # | Location | Current | Decision call |
|---|---|---|---|
| 10 | `src-tauri/src/knowledge.rs:1008` (`detect_and_record_contradictions`) → `jaccard_similarity` (`:970`, used `:1036`, threshold 0.5); merge uses `find_jaccard_match` (`:993`) | Jaccard similarity, not semantic | `noul` same-insight/contradiction + `choice` relation type (§R40) |
| 11 | `src-tauri/src/knowledge.rs:341` (`compound_knowledge` recency-weighted confidence); older path also merges by shared words (`:312`) | fabricated confidence | decision-derived confidence (§R41) |

## Context truncation
`extract_pty_context` (`intelligence.rs:586`) exists but is **orphaned (N/c)** and
truncates by **lines**, not tokens. The decision adapter must implement its own
token-estimate truncation (`ceil(chars/4)` vs `decision.context_limit`) per R7.

## Reused existing infrastructure
- HTTP/auth/retry mechanics (NOT the body/response shape): `intelligence.rs:730`
  (`invoke_with_backoff`), `:752` (`invoke_openrouter` → `chat/completions`).
- Mode abstraction: `IntelligenceRequest` (`intelligence.rs:558`) — add `"decision"` mode.
- Provider key already present: `OPENROUTER_API_KEY` (`knowledge.rs:695`).

# Requirements & SDD-BDD-TDD Checklist — Spec 002

Verified 2026-11-20 against `spec.md`, `bdd-scenarios.md`, `plan.md`, `tasks.md`, `docs/adr/0003-acp-universal-harness-transport.md`, and `docs/sdlc/constitution.md`.

## 1. GitHub Issue Traceability
1. [x] **Issue #19** (`/runner` Agent Spawn & Terminal I/O Broken) → `spec.md` §4.A (R101–R104) → `bdd-scenarios.md` Feature 1 → `tasks.md` M1 (T1–T8).
2. [x] **Issue #20** (Universal ACP Adapter & Multi-Provider Integration) → `spec.md` §4.B (R201–R204) → `bdd-scenarios.md` Feature 2 → `tasks.md` M2 (T9–T16).
3. [x] **Issue #21** (Wave Orchestrator Topological DAG & UI Execute Action) → `spec.md` §4.C (R301–R304) → `bdd-scenarios.md` Feature 3 → `tasks.md` M3 (T17–T24).
4. [x] **Issue #22** (Freeform Spec/Prompt Decomposition) → `spec.md` §4.D (R401) → `bdd-scenarios.md` Feature 4 → `tasks.md` M3 (T25–T26).
5. [x] **Issue #23** (Prune Speculative Facade & Consolidate 4 Core Surfaces) → `spec.md` §4.F (R601) → `bdd-scenarios.md` Feature 6 → `tasks.md` M4 (T33–T34).
6. [x] **Issue #24** (Memory Layer SQL Injection Fix, FTS5 BM25 & Context Injection) → `spec.md` §4.E (R501–R503) → `bdd-scenarios.md` Feature 5 → `tasks.md` M4 (T27–T32).

## 2. SDD Completeness & Constitution Alignment
7. [x] Every requirement in `spec.md` uses EARS (`WHEN ... THEN ... SHALL ...`) with an explicit actor, trigger, and deterministic acceptance criterion.
8. [x] **Constitution §1 (Simplicity) & §2 (Anti-Abstraction):** One `AcpAgentAdapter` serves 5 concrete harnesses (`pi`, `opencode`, `gemini`, `claude`, `codex`) over the standard ACP JSON-RPC 2.0 protocol rather than inventing 9 ad-hoc CLI wrappers. SQLite FTS5 (already bundled) provides BM25 search without external vector servers.
9. [x] **Constitution §3 (Integration-First):** Reuses existing AES-256 Connector Vault (`assets.rs`), `AdapterRegistry`, `wave_supervisor.rs`, `handoff_parser.rs`, and `agent_events.rs`.
10. [x] **Constitution §4 (Local-First / Privacy):** All execution and FTS5 indexing run locally; Vault API keys are injected into local child process environments and never logged or written to disk.
11. [x] **Constitution §5 (Test-First):** Every implementation task in `tasks.md` is preceded by a paired failing-test (`RED`) task.
12. [x] **Constitution §6 (Graceful Degradation):** `AcpAgentAdapter` falls back to `OpenCodeAdapter` (`opencode run --format json`) if an older `opencode` binary lacks `acp`; worktree junction creation degrades gracefully with a warning if the OS denies symlink/junction creation.

## 3. BDD & TDD Quality
13. [x] Every EARS requirement `R101–R601` has a corresponding Gherkin `Scenario` (`Given / When / Then`) in `bdd-scenarios.md`.
14. [x] Every Gherkin scenario maps to a named automated test function in `tasks.md`.
15. [x] Independent test+implementation pairs are marked `[P]` with non-overlapping file ownership for parallel worktree execution.

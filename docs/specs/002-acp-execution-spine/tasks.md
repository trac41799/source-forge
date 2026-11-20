# Tasks 002 — ACP Execution Spine, Real PTY Runner & Topological Wave DAG

**Spec:** `spec.md` · **BDD Scenarios:** `bdd-scenarios.md` · **Plan:** `plan.md`  
**GitHub Issues:** #19, #20, #21, #22, #23, #24

Every behavior begins with a failing test task (**RED**) followed by the implementation task (**GREEN**). Tasks marked `[P]` can run in parallel worktrees without file ownership conflicts.

Format: `T<n> [P] Title — owned files — acceptance: spec.md §N — test`.

---

## M1 — Interactive PTY Runner Spine & IPC Repair (Issue #19 · R101–R104)

- **T1** [P] Failing test: IPC payload contract test asserts `agentStore.spawnAgent` passes `{ agentId, projectPath, command, args, envVars }` matching `commands.rs::spawn_agent` and uses `currentProject.path` — `src/__tests__/stores/agentStore.test.ts`, `src/__tests__/pages/Runner.test.tsx` — §4.A (R101) — `agentStore.test.ts::spawns_with_valid_ipc_payload` (**red**)
- **T2** Fix `agentStore.spawnAgent` IPC payload, pass `currentProject.path` in `AgentPanel.tsx`, and use `IPC.spawnAgent` / `IPC.killAgent` / `IPC.writeToAgent` — `src/stores/agentStore.ts`, `src/components/runner/AgentPanel.tsx`, `src/lib/ipc/commands.ts` — §4.A (R101) — T1 (**green**)
- **T3** [P] Failing test: `PtyManager` allocates a real pseudo-terminal (`portable-pty`), injects Vault env vars, emits `agent-output` lines, accepts `write_to_process` on PTY stdin, and supports `resize_pty` — `src-tauri/src/pty.rs` — §4.A (R102, R103) — `pty.rs::tests::test_real_pty_roundtrip_and_resize` (**red**)
- **T4** Implement `portable-pty` backend in `pty.rs`, add `resize_pty_cmd`, wire `agent_events::emit_agent_output` & `emit_agent_status`, and inject Vault env vars in `commands.rs::spawn_agent` — `src-tauri/Cargo.toml`, `src-tauri/src/pty.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` — §4.A (R102, R103) — T3 (**green**)
- **T5** [P] Failing test: `PtyTerminal.tsx` subscribes to `agent-output` events for its `agentId`, writes data to `xterm.js`, and forwards `terminal.onData` to `writeToAgent` — `src/__tests__/components/PtyTerminal.test.tsx` — §4.A (R103) — `PtyTerminal.test.tsx::streams_output_and_forwards_input` (**red**)
- **T6** Wire `listen('agent-output')`, `terminal.onData`, and `ResizeObserver` → `resize_pty_cmd` in `PtyTerminal.tsx` — `src/components/terminal/PtyTerminal.tsx` — §4.A (R103) — T5 (**green**)
- **T7** [P] Failing test: `Runner.tsx` lists all 9 `AGENT_CONFIGS` in `+ Add Agent` and wires `Load Profile`, `+ New` preset, `Analyze`, and `Docs` buttons to real actions — `src/__tests__/pages/Runner.test.tsx` — §4.A (R104) — `Runner.test.tsx::lists_all_configured_agents_and_wires_actions` (**red**)
- **T8** Update `Runner.tsx` to use `AGENT_CONFIGS` and wire header/footer actions — `src/pages/Runner.tsx` — §4.A (R104) — T7 (**green**)

---

## M2 — Universal ACP Adapter & Multi-Provider Integration (Issue #20 · R201–R204)

- **T9** [P] Failing test: `AcpAgentAdapter` completes JSON-RPC 2.0 `initialize` → `session/new` (with `cwd` and `mcpServers`) → `session/prompt` against a mock ACP stdio server and marks session finished on prompt result — `src-tauri/src/agent_adapters/acp.rs` — §4.B (R201) — `agent_adapters::acp::tests::test_acp_handshake_and_prompt_lifecycle` (**red**)
- **T10** Implement `AcpAgentAdapter` core JSON-RPC 2.0 transport and lifecycle (`initialize`, `session/new`, `session/prompt`, `session/cancel`) — `src-tauri/src/agent_adapters/acp.rs`, `src-tauri/src/agent_adapters/mod.rs` — §4.B (R201) — T9 (**green**)
- **T11** [P] Failing test: `AcpAgentAdapter` accumulates cost from `session/update` notifications, serves `fs/read_text_file` & `fs/write_text_file` inside `worktree`, approves worktree-scoped `session/request_permission`, and denies path traversal outside `worktree` — `src-tauri/src/agent_adapters/acp.rs` — §4.B (R202) — `agent_adapters::acp::tests::test_acp_telemetry_and_worktree_permission_jail` (**red**)
- **T12** Implement `session/update`, `session/request_permission`, and `fs/*` handlers with canonical path containment in `AcpAgentAdapter` — `src-tauri/src/agent_adapters/acp.rs` — §4.B (R202) — T11 (**green**)
- **T13** [P] Failing test: `AdapterRegistry` registers ACP profiles for `pi`, `opencode`, `gemini`, `claude`, and `codex`, and injects decrypted Vault secrets into the child environment — `src-tauri/src/agent_adapters/mod.rs` — §4.B (R203) — `agent_adapters::tests::test_registry_has_all_acp_harnesses_and_vault_env` (**red**)
- **T14** Register `pi`, `opencode`, `gemini`, `claude`, and `codex` profiles in `AdapterRegistry` with Vault environment injection and expose all 5 harnesses in `BuildApp.tsx` and `Orchestrate.tsx` — `src-tauri/src/agent_adapters/mod.rs`, `src/pages/BuildApp.tsx`, `src/pages/Orchestrate.tsx` — §4.B (R203) — T13 (**green**)
- **T15** [P] Failing test: when an ACP turn completes with modified files in the worktree but no `HANDOFF_<id>.md`, `AcpAgentAdapter` synthesizes a valid 6-section `HANDOFF_<id>.md` that passes `handoff_parser::parse_handoff_file` — `src-tauri/src/agent_adapters/acp.rs` — §4.B (R204) — `agent_adapters::acp::tests::test_auto_synthesizes_handoff_on_clean_turn` (**red**)
- **T16** Implement fallback `HANDOFF_<id>.md` synthesis on ACP turn completion — `src-tauri/src/agent_adapters/acp.rs` — §4.B (R204) — T15 (**green**)

---

## M3 — Topological Wave DAG Execution, Worktree Junctions & Freeform Spec Parser (Issues #21, #22 · R301–R304, R401)

- **T17** [P] Failing test: `orchestrator::validate_wave_plan` rejects dependency cycles (`A -> B -> A`), self-dependencies, unknown `depends_on` refs, and dependencies on higher wave numbers — `src-tauri/src/orchestrator.rs` — §4.C (R301) — `orchestrator::tests::test_rejects_cycles_and_invalid_wave_deps` (**red**)
- **T18** Implement topological sort and DAG validation in `orchestrator.rs` and call it in `add_plan_agent` / `execute_wave` — `src-tauri/src/orchestrator.rs` — §4.C (R301) — T17 (**green**)
- **T19** [P] Failing test: in a 2-wave plan where Wave 2 agent `2-1` depends on Wave 1 agent `1-1`, `execute_wave_with_adapters` finishes and merges Wave 1 before creating `2-1`'s worktree, so `2-1`'s worktree contains `1-1`'s committed file at spawn time — `src-tauri/src/wave_executor.rs` — §4.C (R302) — `wave_executor::tests::test_topological_wave_execution_merges_wave1_before_spawning_wave2` (**red**)
- **T20** Refactor `wave_executor::execute_wave_with_adapters` and `pipeline.rs` to execute, supervise, verify, and merge wave-by-wave in topological order — `src-tauri/src/wave_executor.rs`, `src-tauri/src/pipeline.rs` — §4.C (R302) — T19 (**green**)
- **T21** [P] Failing test: `worktree::create_worktree` creates a junction/symlink to `base_repo/node_modules` inside the new worktree when `base_repo/node_modules` exists — `src-tauri/src/worktree.rs` — §4.C (R303) — `worktree::tests::test_worktree_links_node_modules` (**red**)
- **T22** Implement `link_shared_dependencies` (`node_modules` junction on Windows / symlink on Unix) in `worktree.rs` — `src-tauri/src/worktree.rs` — §4.C (R303) — T21 (**green**)
- **T23** [P] Failing test: `Orchestrate.tsx` loads persisted `wavePlans` on mount via `get_wave_plans_cmd` and clicking "Execute Wave Plan" calls `store.executeWave` and renders verification results — `src/__tests__/pages/OrchestrateTabs.test.tsx` — §4.C (R304) — `OrchestrateTabs.test.tsx::loads_plans_and_executes_wave_from_ui` (**red**)
- **T24** Add `get_wave_plans_cmd` to `orchestrator.rs` / `commands.rs` / `lib.rs` / `IPC`, load plans on mount in `Orchestrate.tsx`, and wire **Execute Wave Plan** and **Verify & Finalize** buttons — `src-tauri/src/orchestrator.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/lib/ipc/commands.ts`, `src/stores/orchestrationStore.ts`, `src/pages/Orchestrate.tsx` — §4.C (R304) — T23 (**green**)
- **T25** [P] Failing test: `spec_parser::parse_spec_or_prompt` parses both structured `## Phase N` specs and freeform markdown PRDs (headings/bullet lists) into valid `Vec<SpecTask>` with wave numbers and dependency edges — `src-tauri/src/spec_parser.rs` — §4.D (R401) — `spec_parser::tests::test_parses_freeform_prd_and_bullets` (**red**)
- **T26** Implement `parse_spec_or_prompt` in `spec_parser.rs` and wire into `pipeline.rs::stage_parse_spec` — `src-tauri/src/spec_parser.rs`, `src-tauri/src/pipeline.rs` — §4.D (R401) — T25 (**green**)

---

## M4 — Memory Layer SQL Safety, FTS5 BM25, Context Injection & UI Consolidation (Issues #23, #24 · R501–R503, R601)

- **T27** [P] Failing test: `memory::create_memory_fact`, `update_memory_fact`, and `get_memory_facts` safely handle single quotes (`"I'll use Rust's borrow checker"`) and SQL injection payloads (`"'; DROP TABLE memory_facts; --"`) without error or data loss — `src-tauri/src/memory.rs` — §4.E (R501) — `memory::tests::test_sql_injection_and_single_quotes_safe` (**red**)
- **T28** Replace all `format!` SQL string interpolations in `memory.rs` with parameterized `rusqlite::params!` bindings — `src-tauri/src/memory.rs` — §4.E (R501) — T27 (**green**)
- **T29** [P] Failing test: migration `023_memory_fts5.sql` creates `memory_facts_fts` with sync triggers, and `memory::hybrid_search` ranks multi-word matches using FTS5 BM25 — `src-tauri/src/memory.rs`, `src-tauri/migrations/023_memory_fts5.sql` — §4.E (R502) — `memory::tests::test_fts5_bm25_hybrid_search` (**red**)
- **T30** Add migration `023_memory_fts5.sql` and update `memory::hybrid_search` to use FTS5 `bm25(memory_facts_fts)` with sanitized token matching — `src-tauri/migrations/023_memory_fts5.sql`, `src-tauri/src/db.rs`, `src-tauri/src/memory.rs` — §4.E (R502) — T29 (**green**)
- **T31** [P] Failing test: `wave_executor` injects project memory facts (`memory::get_context`) and stack anti-pattern warnings (`knowledge::get_preflight_warnings`) into `.acc/GUIDELINE.md` under `## Project Memory & Anti-Pattern Guardrails` — `src-tauri/src/wave_executor.rs` — §4.E (R503) — `wave_executor::tests::test_injects_memory_and_antipatterns_into_guideline` (**red**)
- **T32** Wire `memory::get_context` and `knowledge::get_preflight_warnings` into `guideline_spawn.rs` / `wave_executor.rs` — `src-tauri/src/guideline_spawn.rs`, `src-tauri/src/wave_executor.rs` — §4.E (R503) — T31 (**green**)
- **T33** [P] Failing test: `Sidebar.tsx` groups routes into the 4 core surfaces and hides experimental panels when `settingsStore.experimentalFeatures` is `false` — `src/__tests__/components/Sidebar.test.tsx` — §4.F (R601) — `Sidebar.test.tsx::renders_four_core_groups_and_gates_experimental` (**red**)
- **T34** Consolidate `Sidebar.tsx` into the 4 core workflow groups and gate experimental panels (`Knowledge.tsx` Bagua hexagram view) behind `settingsStore.experimentalFeatures` — `src/components/layout/Sidebar.tsx`, `src/stores/settingsStore.ts`, `src/pages/Knowledge.tsx`, `src/pages/Settings.tsx` — §4.F (R601) — T33 (**green**)
- **T35** Adversarial review & full gate verification across all changed files (`cargo test`, `npm test`, `npx tsc --noEmit`, `npm run lint`, `pytest`) — all — AGENTS.md §3 — all suites green

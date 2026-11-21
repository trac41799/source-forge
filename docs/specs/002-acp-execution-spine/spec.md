# Spec 002 — ACP Execution Spine, Real PTY Runner & Topological Wave DAG

**Status:** Draft (SDD-BDD-TDD) · **Date:** 2026-11-20  
**Constitution:** `docs/sdlc/constitution.md` · **ADR:** `docs/adr/0003-acp-universal-harness-transport.md`  
**GitHub Issues:** [#19](https://github.com/trac41799/source-forge/issues/19), [#20](https://github.com/trac41799/source-forge/issues/20), [#21](https://github.com/trac41799/source-forge/issues/21), [#22](https://github.com/trac41799/source-forge/issues/22), [#23](https://github.com/trac41799/source-forge/issues/23), [#24](https://github.com/trac41799/source-forge/issues/24)

---

## 1. Goal
Transform SourceForge from a partially wired prototype into a working, multi-provider, multi-harness AI engineering cockpit by:
1. Fixing the broken `/runner` interactive terminal spine with real `portable-pty` (`ConPTY`/`openpty`) and bidirectional Tauri event streaming (Issue #19).
2. Introducing a universal **Agent Client Protocol (ACP)** JSON-RPC 2.0 adapter (`AcpAgentAdapter`) with AES-256 Vault credential injection so SourceForge natively drives **Pi Agent (`pi-acp`, 15+ providers)**, **OpenCode (`opencode acp`)**, **Gemini CLI (`gemini --acp`)**, **Claude Code (`claude-agent-acp`)**, and **Codex CLI (`codex-acp`)** (Issue #20).
3. Enforcing true topological Wave & Dependency DAG execution in `wave_executor.rs` and wiring `/orchestrate` to execute, monitor, verify, and merge waves from the UI (Issue #21).
4. Upgrading `spec_parser.rs` to decompose freeform markdown PRDs or natural-language goals into conflict-free waves (Issue #22).
5. Eliminating SQL injection and single-quote crashes in `memory.rs`, indexing facts with SQLite FTS5 BM25, and injecting project memory/anti-patterns into every agent session (Issue #24).
6. Pruning dead/unwired UI controls and consolidating navigation into 4 functional core surfaces (Issue #23).

## 2. Non-goals
- Cloud-hosted multi-tenant SaaS execution (all execution remains local-first per Constitution §4).
- Modifying or training decision models (covered by Spec 001).
- Expanding Clifford-Bagua hexagram mathematics or Leiden GraphRAG clustering (frozen behind `experimentalFeatures` flag).

## 3. Actors
- **Developer / Operator** — spawns interactive agents in `/runner`, plans and executes multi-agent waves in `/orchestrate` and `/build`, manages provider keys in Vault.
- **ACP Harness Process** — external child process speaking ACP JSON-RPC 2.0 over `stdio` (`pi-acp`, `opencode acp`, `gemini --acp`, `claude-agent-acp`, `codex-acp`).
- **Interactive PTY Process** — child TUI process attached to a `portable-pty` pseudo-terminal in `/runner`.
- **Wave Executor & Supervisor** — schedules tasks in topological wave order across isolated git worktrees, enforces budgets/deadlines, runs verification gates, and merges deliverables.

---

## 4. Requirements (EARS Format)

### A. Interactive PTY Runner Spine (Issue #19)
- **R101 (IPC Contract & Payload Integrity)** WHEN the frontend requests to spawn an interactive agent in `/runner` THEN `agentStore.spawnAgent` SHALL invoke ` IPC.spawnAgent` (`spawn_agent`) with `{ agentId, projectPath, command, args, envVars }` matching `commands.rs::spawn_agent`, using the active project path from `projectStore` (never a hardcoded `'/projects/current'` placeholder).
  - *AC:* `spawn_agent` succeeds in an integration test and an IPC payload contract test verifies parameter keys between `agentStore.ts` and `commands.rs`.
- **R102 (Real Pseudo-Terminal Execution)** WHEN `PtyManager::spawn_process_with_guards` spawns an interactive agent THEN it SHALL allocate a real pseudo-terminal (`portable-pty`, default 120×36) with `TERM=xterm-256color` and merge decrypted provider secrets from the AES-256 Connector Vault into `env_vars`.
  - *AC:* Processes checking `isatty(0)` see a TTY, ANSI output is preserved, and `resize_pty_cmd(agent_id, cols, rows)` resizes the active PTY master.
- **R103 (Bidirectional Terminal Streaming)** WHEN a spawned PTY process emits bytes or changes lifecycle state THEN `PtyManager` SHALL emit `agent-output` (`AgentOutputEvent`) and `agent-status` (`AgentStatusEvent`) via Tauri events, and `PtyTerminal.tsx` SHALL write incoming chunks to `xterm.js` and forward `terminal.onData` keystrokes to `write_to_agent`.
  - *AC:* Typing a line in `PtyTerminal.tsx` sends bytes to the child PTY's stdin, and child stdout renders in `PtyTerminal.tsx` within 50 ms without polling `get_agent_output`.
- **R104 (Truthful Runner Controls)** WHEN `/runner` renders THEN the `+ Add Agent` selector SHALL list all configured agents from `AGENT_CONFIGS` (with install/availability badges), and every footer/header button (`Load Profile`, `+ New` preset, `Analyze`, `Docs`) SHALL invoke its wired store action.
  - *AC:* Zero dead buttons exist in `Runner.tsx`; clicking each button triggers its verified store/IPC action in `Runner.test.tsx`.

### B. Universal ACP Adapter & Multi-Provider Integration (Issue #20)
- **R201 (ACP JSON-RPC 2.0 Handshake & Session Lifecycle)** WHEN `AcpAgentAdapter::spawn(task, worktree)` is called THEN it SHALL spawn the configured ACP binary/command with piped `stdin`/`stdout`, complete the JSON-RPC 2.0 `initialize` and `session/new` (including `cwd: worktree` and active project `mcpServers`) handshake, and send `session/prompt` with the task content.
  - *AC:* Against a mock ACP server and real `opencode acp` / `pi-acp`, the adapter completes `initialize` → `session/new` → `session/prompt` and transitions session state to `done` upon receiving the prompt result.
- **R202 (ACP Notifications, Permissions & Telemetry)** WHILE an ACP session is active THEN `AcpAgentAdapter` SHALL process `session/update` notifications (recording streamed output, tool calls, diffs, and token/cost usage), respond to `fs/read_text_file` and `fs/write_text_file` requests scoped strictly within `worktree`, and evaluate `session/request_permission` against the worktree security policy (auto-approving edits/commands inside `worktree`, denying path traversal outside `worktree`).
  - *AC:* `session_cost()` returns the cumulative cost from ACP usage updates; permission requests targeting paths outside `worktree` (e.g., `../../etc/passwd` or `~/.ssh`) are denied and logged.
- **R203 (Multi-Harness Registry & Vault Environment Injection)** WHEN `AdapterRegistry::new_with_db` initializes THEN it SHALL register ACP profiles for `pi` (`pi-acp`), `opencode` (`opencode acp` with CLI fallback), `gemini` (`gemini --acp`), `claude` (`claude-agent-acp`), and `codex` (`codex-acp`), injecting decrypted API keys (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`) from `vault_entries` into the child process environment.
  - *AC:* Selecting any of the 5 harnesses in `/build` or `/orchestrate` resolves a registered `AgentAdapter` and passes vault credentials without writing plaintext secrets to disk or logs (Constitution §4).
- **R204 (Auto-Synthesized Handoff Envelope on ACP Turn Completion)** IF an ACP agent finishes its `session/prompt` turn with a non-empty worktree diff but did not manually write `HANDOFF_<agent_ref>.md` THEN `AcpAgentAdapter` SHALL synthesize a valid 6-section `HANDOFF_<agent_ref>.md` from the ACP assistant summary, modified files list (`git status --porcelain`), and test output.
  - *AC:* An ACP agent that modifies assigned files and returns a final message passes `handoff_parser::parse_handoff_file` even if the model omitted manual markdown boilerplate.

### C. Topological Wave DAG Execution & Orchestrator UI Wiring (Issue #21)
- **R301 (DAG Validation & Cycle Rejection)** WHEN `add_plan_agent` or `validate_wave_plan` is called THEN `orchestrator.rs` SHALL validate that `depends_on` references exist in an earlier or same wave and reject any dependency cycle (e.g., `A -> B -> A`) with a descriptive error.
  - *AC:* Adding a cyclic dependency or a dependency on a higher wave number returns `Err("Dependency cycle detected...")` or `Err("Agent in wave 1 cannot depend on wave 2...")`.
- **R302 (Topological Multi-Wave Execution)** WHEN `execute_wave_with_adapters` runs a `WavePlan` containing multiple waves or `depends_on` edges THEN it SHALL group agents by topological level (Wave 1, Wave 2, …), execute and supervise Wave `N` to completion, merge Wave `N`'s verified worktree commits into the base branch, and only then create worktrees and spawn agents for Wave `N+1` (so Wave `N+1` worktrees start with Wave `N`'s merged code!).
  - *AC:* In a 2-wave plan where Agent `2-1` (Wave 2) depends on Agent `1-1` (Wave 1), Agent `2-1`'s worktree contains the file created and merged by Agent `1-1` before Agent `2-1` is spawned.
- **R303 (Worktree Dependency Junctions)** WHEN `worktree::create_worktree` creates a linked worktree THEN it SHALL create a directory junction (Windows) or symlink (Unix) for `node_modules` from `base_repo` into `worktree_path` if `base_repo/node_modules` exists and `worktree_path/node_modules` does not.
  - *AC:* A newly created worktree in a Node/TypeScript repo can run `npx tsc --noEmit` immediately without running `npm install`.
- **R304 (Live `/orchestrate` UI Execution & Persistence)** WHEN `/orchestrate` mounts THEN it SHALL fetch all persisted `feature_plans` for the active project via `get_wave_plans_cmd`, and provide **Execute Plan**, **Cancel**, and **Verify & Finalize** buttons that invoke `execute_wave_cmd` / `verify_and_finalize_wave_cmd` and display live per-agent progress.
  - *AC:* Creating a plan, refreshing `/orchestrate`, and clicking **Execute Plan** invokes `IPC.executeWave`, streams status transitions (`queued` → `running` → `done`), and displays verification + merge results.

### D. Flexible Spec/Prompt Decomposition (Issue #22)
- **R401 (Dual-Mode Spec Parser: Structured + Freeform PRD/Prompt)** WHEN `spec_parser::parse_spec_or_prompt` is given a markdown document or prompt THEN it SHALL parse explicit `## Phase N` / `### Step X.Y` headers if present, and otherwise decompose markdown headings/bullet lists or natural-language goals into a valid `Vec<SpecTask>` with wave numbers, dependencies, and non-overlapping owned file hints.
  - *AC:* Both `GAP_CLOSURE_PLAN.md` and a plain 3-bullet feature PRD produce a non-empty `Vec<SpecTask>` that seeds a runnable `WavePlan` in `pipeline.rs`.

### E. Memory Layer Security, FTS5 Search & Context Injection (Issue #24)
- **R501 (Parameterized SQL in `memory.rs`)** WHEN `get_memory_facts`, `update_memory_fact`, or any query in `memory.rs` executes THEN it SHALL use parameterized `rusqlite::params!` bindings and NEVER interpolate user or agent strings with `format!`.
  - *AC:* Storing, updating, and querying facts containing `"I'll use Rust's borrow checker"` and `"'; DROP TABLE memory_facts; --"` succeeds with zero SQL errors and leaves `memory_facts` intact.
- **R502 (SQLite FTS5 BM25 Hybrid Search)** WHEN migration `023_memory_fts5.sql` runs THEN it SHALL create an FTS5 virtual table `memory_facts_fts` synchronized via triggers with `memory_facts`, and `memory::hybrid_search` SHALL rank candidates using FTS5 `bm25(memory_facts_fts)` combined with recency decay and confidence.
  - *AC:* `hybrid_search` returns multi-term ranked matches via FTS5 `MATCH` and falls back gracefully on special-character queries.
- **R503 (Automatic Worktree & ACP Context Injection)** WHEN `wave_executor` prepares an agent worktree THEN it SHALL query top project memory facts (`memory::get_context`) and stack anti-pattern warnings (`knowledge::get_preflight_warnings`) and append a `## Project Memory & Anti-Pattern Guardrails` section to `.acc/GUIDELINE.md` (and `.acc/TASK.md`).
  - *AC:* Confirmed anti-patterns and memory facts for the project appear in the spawned agent's `.acc/GUIDELINE.md`.

### F. Surface Consolidation & Experimental Gating (Issue #23)
- **R601 (4-Group Core Navigation & Experimental Flag)** WHEN the application renders with default settings (`experimentalFeatures = false`) THEN the sidebar SHALL organize navigation into 4 clear workflow groups (**Workspace**, **Knowledge & Assets**, **Review & Outcomes**, **Settings**) and hide experimental/speculative panels (Clifford-Bagua hexagram explorer, manual ACB signal injector) unless `experimentalFeatures` is enabled in Settings.
  - *AC:* Sidebar renders the 4 core groups by default; toggling `experimentalFeatures` in Settings reveals/hides experimental panels.

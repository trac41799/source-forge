# BDD Scenarios — Spec 002: ACP Execution Spine, Real PTY Runner & Topological Wave DAG

**Spec:** `spec.md` · **ADR:** `docs/adr/0003-acp-universal-harness-transport.md`  
**Issues:** #19, #20, #21, #22, #23, #24

Each Gherkin scenario below maps 1:1 to an EARS requirement in `spec.md` and an automated test in `tasks.md`.

---

## Feature 1: Interactive PTY Runner Spine (Issue #19 · R101–R104)

```gherkin
Feature: Interactive Multi-Agent Terminal Runner (/runner)
  As a developer using SourceForge
  I want to spawn, view live ANSI output from, and type commands into interactive AI coding agents in /runner
  So that I can supervise multiple interactive CLI sessions in one workspace

  Scenario: Spawning an interactive agent sends a valid IPC payload with the active project path (R101)
    Given the active project in projectStore is "D:/projects/demo-app"
    And the user selects "Claude Code" from the "+ Add Agent" menu in /runner
    When AgentPanel or Runner triggers spawnAgent
    Then agentStore SHALL invoke "spawn_agent" with:
      | field       | value                            |
      | agentId     | claude                           |
      | projectPath | D:/projects/demo-app             |
      | command     | claude                           |
      | args        | ["--dangerously-skip-permissions"] |
      | envVars     | {}                               |
    And the agent card status SHALL transition from "idle" to "Running"

  Scenario: Interactive process runs inside a real pseudo-terminal with Vault secrets injected (R102)
    Given a Vault secret "ANTHROPIC_API_KEY" exists in vault_entries
    When PtyManager spawns an interactive session for agent "claude"
    Then a native PTY pair (120 cols x 36 rows, TERM=xterm-256color) SHALL be allocated
    And the child process environment SHALL contain the decrypted "ANTHROPIC_API_KEY"
    And calling "resize_pty_cmd" with 160 cols x 48 rows SHALL resize the live PTY master

  Scenario: Bidirectional terminal streaming between PtyManager and xterm.js (R103)
    Given an interactive PTY session "claude" is running
    When the child process writes "Welcome to Claude Code\r\n> " to its PTY slave
    Then PtyManager SHALL emit an "agent-output" Tauri event for agent_id "claude"
    And PtyTerminal.tsx SHALL write the chunk into xterm.js
    When the user types "help\r" in PtyTerminal.tsx
    Then terminal.onData SHALL invoke "write_to_agent" with agentId "claude" and text "help\r"
    And the child process SHALL receive "help\r" on its PTY stdin
```

---

## Feature 2: Universal ACP (Agent Client Protocol) Adapter & Multi-Provider Support (Issue #20 · R201–R204)

```gherkin
Feature: Universal ACP (Agent Client Protocol) Execution
  As a developer or wave orchestrator
  I want SourceForge to drive any ACP-compliant coding harness (Pi Agent, OpenCode, Gemini CLI, Claude Code, Codex CLI) over JSON-RPC 2.0
  So that I can run multi-provider and multi-harness waves with structured telemetry and permission guardrails

  Scenario: Full ACP JSON-RPC 2.0 session lifecycle (R201)
    Given AcpAgentAdapter is configured with an ACP harness command
    And a task "Create src/hello.ts exporting greet()" in worktree ".worktrees/plan-1-1"
    When AcpAgentAdapter::spawn is called
    Then the adapter SHALL send JSON-RPC request "initialize" with protocolVersion 1
    And upon receiving the initialize response, SHALL send "session/new" with cwd ".worktrees/plan-1-1" and configured mcpServers
    And upon receiving sessionId, SHALL send "session/prompt" with the task instructions
    When the ACP agent responds to "session/prompt" with stopReason "end_turn"
    Then AcpAgentAdapter::is_running SHALL return false

  Scenario: ACP streaming telemetry and worktree-scoped permission mediation (R202)
    Given an active ACP session in worktree "/repo/.worktrees/plan-1-1"
    When the agent emits "session/update" notifications containing usage cost 0.0142 USD
    Then AcpAgentAdapter::session_cost SHALL return Some(0.0142)
    When the agent sends "session/request_permission" to edit "/repo/.worktrees/plan-1-1/src/index.ts"
    Then AcpAgentAdapter SHALL respond with outcome "selected" (allow)
    When the agent sends "session/request_permission" to edit "/repo/outside.ts" or "../../etc/passwd"
    Then AcpAgentAdapter SHALL respond with outcome "cancelled" (deny)

  Scenario: Out-of-the-box multi-harness registry with Vault credential injection (R203)
    Given AdapterRegistry is initialized
    Then adapters for "pi", "opencode", "gemini", "claude", and "codex" SHALL be registered
    When "pi" is spawned with "OPENROUTER_API_KEY" stored in the Connector Vault
    Then the spawned "pi-acp" child process SHALL receive "OPENROUTER_API_KEY" in its environment without logging the key

  Scenario: Auto-synthesizing HANDOFF_<id>.md when an ACP agent completes work without writing boilerplate (R204)
    Given an ACP agent completes its "session/prompt" turn in worktree ".worktrees/plan-1-1"
    And the agent modified "src/hello.ts" in the worktree
    But the agent did not manually create "HANDOFF_1-1.md"
    When the ACP turn finishes
    Then AcpAgentAdapter SHALL synthesize "HANDOFF_1-1.md" containing all 6 required sections from the ACP transcript and git status
    And handoff_parser::parse_handoff_file("HANDOFF_1-1.md") SHALL succeed
```

---

## Feature 3: Topological Wave DAG Execution & `/orchestrate` UI Wiring (Issue #21 · R301–R304)

```gherkin
Feature: Dependency-Aware Topological Wave Execution
  As a team lead orchestrating a multi-agent feature
  I want Wave 2 agents to wait until Wave 1 dependencies complete, verify, and merge into the base branch
  So that dependent agents build on top of Wave 1's delivered code without conflicts

  Scenario: Rejecting cyclic or backwards wave dependencies (R301)
    Given a wave plan "feat-auth"
    And agent "A1" is in wave 1 depending on "A2"
    When the user or planner adds agent "A2" in wave 1 depending on "A1"
    Then orchestrator::validate_wave_plan SHALL fail with a dependency cycle error

  Scenario: Wave 2 worktree receives Wave 1's merged files before Wave 2 spawns (R302)
    Given a wave plan with:
      | agent_ref | wave | depends_on | task                                  |
      | 1-1       | 1    |            | Create schema.ts exporting UserSchema |
      | 2-1       | 2    | 1-1        | Import UserSchema from schema.ts      |
    When execute_wave_with_adapters runs the plan
    Then only agent "1-1" SHALL spawn in Wave 1
    And after agent "1-1" completes and passes verification, its branch SHALL be merged into the base branch
    And only after that merge completes SHALL worktree ".worktrees/<plan>-2-1" be created and agent "2-1" spawned
    And ".worktrees/<plan>-2-1/schema.ts" SHALL exist when agent "2-1" starts

  Scenario: Worktree creation automatically links node_modules (R303)
    Given a base repository containing a "node_modules" directory
    When worktree::create_worktree creates ".worktrees/test-wt"
    Then ".worktrees/test-wt/node_modules" SHALL exist as a junction/symlink to the base repository's "node_modules"

  Scenario: Executing and monitoring a Wave Plan from /orchestrate UI (R304)
    Given a persisted wave plan "add-billing" exists in SQLite
    When the user navigates to /orchestrate
    Then "add-billing" SHALL be loaded and selectable on mount
    When the user selects "add-billing" and clicks "Execute Wave Plan"
    Then orchestrationStore.executeWave SHALL be invoked
    And the UI SHALL display live status transitions and the verification + merge summary
```

---

## Feature 4: Flexible Spec & Natural-Language Prompt Decomposition (Issue #22 · R401)

```gherkin
Feature: Dual-Mode Spec and PRD Parser
  As a user of Build App (/build) or Wave Orchestrator (/orchestrate)
  I want to supply either a structured GAP_CLOSURE_PLAN.md file or a standard markdown PRD / feature prompt
  So that I don't have to hand-format my specs with rigid "## Phase N" / "### Step X.Y" headers

  Scenario: Decomposing a freeform markdown PRD into runnable wave tasks (R401)
    Given a markdown file containing:
      """
      # Todo App Spec
      - Build SQLite todo repository in src/db.ts
      - Build REST endpoints in src/api.ts (depends on db)
      - Build React TodoList component in src/TodoList.tsx
      """
    When spec_parser::parse_spec_or_prompt parses the content
    Then it SHALL return 3 SpecTasks with non-empty titles, objectives, wave assignments, and dependency links
```

---

## Feature 5: Memory Layer SQL Safety, FTS5 BM25 & Context Injection (Issue #24 · R501–R503)

```gherkin
Feature: Safe, Searchable, Auto-Injected Project Memory
  As a developer whose agents accumulate project conventions and lessons
  I want memory facts with single quotes stored safely, ranked via real BM25, and injected into agent guidelines
  So that agents never crash SQLite and automatically follow discovered project conventions

  Scenario: Storing and querying memory facts containing single quotes and SQL metacharacters (R501)
    Given an empty memory_facts table
    When create_memory_fact is called with content "I'll use Rust's rusqlite::params! so '; DROP TABLE memory_facts; -- is safe"
    And update_memory_fact updates the content to "We don't use raw format!('...') in SQL"
    And get_memory_facts is queried with q "don't"
    Then all three operations SHALL succeed without error and return the updated fact

  Scenario: Ranking memory facts with SQLite FTS5 BM25 (R502)
    Given memory_facts contains 3 facts about "authentication token expiry", "database pool", and "token refresh rotation"
    When memory::hybrid_search is called with query "token rotation"
    Then it SHALL query memory_facts_fts using FTS5 BM25 and return "token refresh rotation" as the top match

  Scenario: Injecting project memory and anti-pattern warnings into worktree guidelines (R503)
    Given a high-confidence anti-pattern "Never use format! in SQL queries" exists for stack "rust"
    And a memory fact "Always run cargo test with MSVC toolchain" exists for the project
    When wave_executor prepares ".acc/GUIDELINE.md" for an agent in that project
    Then ".acc/GUIDELINE.md" SHALL contain a "## Project Memory & Anti-Pattern Guardrails" section listing both items
```

---

## Feature 6: Surface Consolidation & Experimental Gating (Issue #23 · R601)

```gherkin
Feature: Focused 4-Surface Navigation & Experimental Feature Gate
  As a developer opening SourceForge
  I want a clean sidebar organized into 4 functional workflow groups with speculative subsystems hidden by default
  So that every visible screen and button works reliably

  Scenario: Default sidebar shows 4 core groups and hides experimental panels (R601)
    Given settingsStore has experimentalFeatures = false (default)
    When the app renders Sidebar and Knowledge page
    Then Sidebar SHALL display the 4 core groups ("Workspace", "Knowledge & Assets", "Review & Outcomes", "Settings")
    And experimental Clifford-Bagua hexagram controls SHALL NOT be rendered unless experimentalFeatures is toggled on
```

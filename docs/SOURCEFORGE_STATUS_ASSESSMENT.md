# SourceForge Status Assessment

**Date**: 2026-07-10  
**Version**: v0.9.0 (unreleased)  
**Architecture**: Tauri v2 (Rust) + React 19 (TypeScript) + SQLite

---

## 1. Original Design Intent

SourceForge (originally "Agent Control Center") was designed as a desktop AI agent orchestration
platform. Its core promise: **parse a spec → provision infrastructure → spawn parallel AI agents →
verify output → deploy to production — all automated.**

Key design pillars:
1. **Wave Orchestration**: Parse a markdown spec into tasks, execute them in parallel worktrees
2. **Agent Harness**: Spawn and manage multiple AI coding agents (OpenCode, Claude, Aider, etc.)
3. **Infrastructure Aware**: Know about stacks, provision databases, generate deploy configs
4. **E2E Verification**: Build output check, SPA routing, server smoke tests
5. **Playbooks**: Export/import reusable agent configurations

---

## 2. What Exists (Feature Inventory)

### Backend (Rust/Tauri v2)

| Layer | Module | Purpose | Status |
|-------|--------|---------|--------|
| **Orchestration** | `wave_executor.rs` | Execute waves, create worktrees, spawn agents | Working |
| | `orchestrator.rs` | Plan/agent DB CRUD, guideline generation | Working |
| | `spec_parser.rs` | Parse GAP_CLOSURE_PLAN.md into tasks | Working |
| | `handoff_parser.rs` | Validate HANDOFF_*.md files | Working |
| | `guideline_spawn.rs` | Write GUIDELINE.md per agent | Working |
| | `worktree.rs` | Git worktree create/remove/list | Working |
| | `pty.rs` | PTY-based agent process management | Working |
| | `pty_guards.rs` | Deadline/cost enforcement for agents | Working |
| **Stack Aware** | `stack_registry.rs` | 4 stack definitions, CLI detection | Working (11 tests) |
| | `preferences.rs` | User stack preferences, DB CRUD | Working (5 tests) |
| | `provisioner.rs` | CLI check/install, stack requirements | Working (7 tests) |
| | `arch_engine.rs` | Scaffold Next.js/Express/FastAPI projects | Working (4 tests) |
| | `delegation.rs` | User handoff for unautomatable steps | Working (3 tests) |
| **Verification** | `verification.rs` | 18 checks: build, deploy, E2E server test | Working (10 tests) |
| **Agent Adapters** | `agent_adapters/opencode.rs` | OpenCode CLI adapter | Working |
| | `agent_adapters/mod.rs` | Adapter registry + MockAdapter | Working |
| **Infrastructure** | `integrations.rs` | Supabase config CRUD, GitHub repo/PR | Working |
| | `skillbridge.rs` | macOS SkillBridge detection | Working |
| | `scheduler.rs` | Cron engine for recurring waves | Working |
| | `backward_channel.rs` | Lark/Slack/Discord/Telegram messaging | Working |
| **Knowledge** | `knowledge.rs` + `kg_*.rs` | Knowledge graph, BFS, community detection | Working |
| | `memory.rs` | Memory facts, checkpoints, hybrid search | Working |
| | `codebase.rs` | Repo map, codebase search | Working |
| **Business** | `budget.rs` | Token budgets, WIP, cost breakdowns | Working |
| | `control.rs` | File locking, conflict detection | Working |
| | `routing.rs` | Task routing with AI suggestions | Working |
| | `acb.rs` | Agent Communication Bus (inter-agent signals) | Working |
| | `playbook.rs` | Export/import .acc bundles | Working |
| | `events.rs` | Event recording and replay | Working |
| | `intelligence.rs` | Outcome tracking, failure analysis | Working |
| | `agent_events.rs` | Agent output streaming events | Working |
| | `wave_persistence.rs` | Crash recovery state save/load | Working |
| **Data** | `db.rs` | SQLite init + 15 migrations | Working |
| | `assets.rs` | Skills library, MCP registry, secrets vault | Working |
| **Integration Tests** | `integration_tests.rs` | 17 integration tests (written, not verified) | Untested |

### Frontend (React 19 / TypeScript)

| Feature | Route | Status |
|---------|-------|--------|
| Runner (agent spawn) | `/runner` | PTY terminals, orchestrator toggle, presets |
| Orchestrate (wave plans) | `/orchestrate` | Wave plan CRUD, agent assignment, corrections |
| Handoffs | `/orchestrate/handoffs` | Build/validate handoff envelopes |
| Messages (ACB) | `/orchestrate/messages` | Parse/record inter-agent signals |
| Knowledge | `/knowledge` | Browse, relations graph, KG explorer, memory |
| Outcomes | `/outcomes` | Agent success/failure tracking table |
| Replay | `/replay` | Session browser with event timeline |
| Route (task router) | `/route` | Task → AI-recommended agent routing |
| Assets | `/assets` | Skills, memory, MCP, secrets, plugins (5 tabs) |
| Integrations | `/integrations` | Supabase, GitHub, chat daemons (5 subtabs) |
| Scheduler | `/scheduler` | Cron job CRUD with wave presets |
| Playbooks | `/playbooks` | Export/import .acc bundles |
| Costs | `/costs` | Token usage, budgets, models breakdown |
| Settings | `/settings` | Appearance, defaults, integrations status, **stack selector** |

### Key Metrics

| Metric | Count |
|--------|-------|
| Tauri commands | 184 |
| Rust modules | 35+ |
| Rust tests | 40 (16 new, 24 pre-existing) |
| JS/TS tests | 308/309 passing |
| Frontend pages | 17 |
| Zustand stores | ~10 (agentStore, projectStore, settingsStore, etc.) |
| DB migrations | 15 |
| Supported stacks | 4 |
| Verification checks | 18 |
| MCP servers connected | 2 (supabase-sino-vn, supabase-nexusboard) |

---

## 3. Does SourceForge Achieve Its Original Design?

**Partially — about 70%.** Individual modules work well in isolation, but the integrated pipeline
is not yet a single-click experience.

### What Works End-to-End (Verified by NexusBoard v2)

| Capability | Status |
|-----------|--------|
| Specify a task in markdown | ✅ |
| Parse spec into structured tasks | ✅ |
| Create isolated git worktrees | ✅ |
| Spawn OpenCode agents | ✅ (foreground only; background unreliable) |
| Generate GUIDELINE.md per agent | ✅ |
| Validate HANDOFF_*.md | ✅ |
| Verify build output (18 checks) | ✅ |
| Scaffold project per stack | ✅ |
| Detect available CLIs | ✅ |
| Deploy to Vercel | ✅ (manual; not auto from pipeline) |
| Provision Supabase DB | ✅ (via MCP; not auto from pipeline) |
| Run E2E server smoke test | ✅ |

### What Doesn't Work / Is Missing

| Gap | Severity | Description |
|-----|----------|-------------|
| **Integrated pipeline** | CRITICAL | Individual modules work but don't chain together. No "Build App" button. |
| **Agent background execution** | HIGH | PowerShell jobs fail; only sequential foreground execution works |
| **Auto-provisioning** | HIGH | Supabase MCP is connected but pipeline doesn't auto-call `apply_migration` |
| **Auto-deploy** | HIGH | Vercel deploy is manual; not triggered by pipeline completion |
| **Architecture decision at spec time** | MEDIUM | Spec parser doesn't consider chosen stack; agents build Express+Vite for Vercel |
| **Error recovery** | MEDIUM | No automatic retry or correction loop for failed agents |
| **E2E pipeline tests** | MEDIUM | No integration tests for the full pipeline flow |
| **Multi-project support** | LOW | Single-project focused; no project switching during orchestration |
| **Documentation** | MEDIUM | Design docs exist but no user guide |
| **Rust test runner** | LOW | Pre-existing `STATUS_ENTRYPOINT_NOT_FOUND` blocks cargo test |
| **Delegation UI** | LOW | Delegation protocol exists but no frontend UI for delegation tasks |

---

## 4. Gap Closure Plan

### Priority 1: Integrated Pipeline (the "Build App" button)

**What**: A single Tauri command that chains the full pipeline:
```
build_app(spec_path, project_path) → 
  1. Parse spec
  2. Read preferences → choose stack
  3. Provision infrastructure (install CLIs, create Supabase tables via MCP)
  4. Scaffold project (arch_engine)
  5. Execute waves (wave_executor)
  6. Verify (verification.rs)
  7. Deploy (vercel --prod)
  8. Return report with URL
```

**Files to create/modify**:
- NEW: `src-tauri/src/pipeline.rs` — `build_app()` orchestrator
- MODIFY: `commands.rs` — add `build_app_cmd` Tauri command
- NEW: Frontend button in Runner or Orchestrate page

### Priority 2: Auto-Provisioning

**What**: When executing a wave that needs a database, automatically:
1. Check if Supabase MCP is connected
2. Generate Prisma/Supabase schema from spec
3. Call `apply_migration` via MCP
4. Write DATABASE_URL to project's .env

**Files to modify**:
- `provisioner.rs` — add `provision_database()` function
- `wave_executor.rs` — call provisioning before wave execution

### Priority 3: Auto-Deploy

**What**: After verification passes, trigger Vercel deploy:
1. Check if `vercel` CLI is installed
2. Run `vercel --prod --yes` in project directory
3. Return deployment URL

**Files to modify**:
- `provisioner.rs` — add `deploy_to_vercel()` function
- `verification.rs` — call deploy after verification passes

### Priority 4: Agent Reliability

**What**: Fix background agent spawning to work reliably:
- Use tokio::spawn for async agent execution
- Add proper stdout/stderr capture
- Add timeout/retry logic

### Priority 5: Documentation + Tests

- Write `docs/USER_GUIDE.md`
- Add pipeline integration tests
- Fix Rust test runner issue

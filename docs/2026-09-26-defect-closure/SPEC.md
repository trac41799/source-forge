# SPEC-001: Defect Closure & Pipeline Design

**Date:** 2026-09-26
**Status:** Proposed — needs sign-off on §6 Open Questions before Wave C starts
**Methodology:** SDD (this spec) → TDD (`TDD_PLAN.md`) → parallel subagent waves
**Audit basis:** HEAD `afd234d` (2026-07-15), verified by build/test/contract runs on 2026-09-26

---

## 1. Verified defect inventory (evidence, not opinion)

| ID | Defect | Evidence | Severity |
|----|--------|----------|----------|
| D-01 | Rust lib does not compile: 72 × E0015 | `stack_registry.rs:34-87` — `static STACK_REGISTRY: &[StackPreset]` calls `.into()`/`vec!` in const context. `cargo +stable-x86_64-pc-windows-msvc check --workspace` → 72 errors | **Fatal** |
| D-02 | Frontend build fails: TS2339 | `src/pages/Settings.tsx:316` calls `store.updateDefaults?.()` which does not exist on `SettingsState` | **Fatal** |
| D-03 | 13 dead IPC invokes (silent "command not found") | Contract diff of `src/**` `invoke()` vs `lib.rs` handler. Full list in §3.2 | **High** |
| D-04 | Knowledge Compounder has no callable path | `knowledgeStore.ts:317` invokes `run_compounder_cmd` (never defined, never registered); `knowledge.rs:685 run_compounder` has zero callers | **High** |
| D-05 | Deployment verification built but not wired | `verify_project_cmd`, `verify_and_finalize_wave_cmd`, `generate_deploy_config_cmd` registered at `lib.rs:117-119`; zero frontend callers. UI still finalizes on handoff-file existence only | **High** |
| D-06 | Core "Build App" pipeline does not exist | No `pipeline.rs`; `git log -S build_app` finds only the status doc. Confirms `SOURCEFORGE_STATUS_ASSESSMENT.md` gaps 1–3 | **Critical (design gap)** |
| D-07 | Security blockers | `tauri.conf.json` `"csp": null` (explicit CSP added `f4ae90b`, reverted `4605cd5`); broad shell/fs/sql capabilities | **High** |
| D-08 | Test suites broken/red | JS 308/309 + 7 uncaught (`Messages.tsx:57` `acbSignals` undefined); local-daemon 35 pass/13 errors; webhook-server collection fails (`webhook_server` import); `npm run lint` red (does not ignore `target/`) | **Medium** |
| D-09 | Repo hygiene | `.worktrees/dogfood--*` committed as gitlinks (mode 160000); `.gitignore` tail UTF-16-corrupted → patterns inert | **Low** |
| D-10 | No CI gates | Only `release.yml` (on tags). No push/PR build+test workflow; README "build passing" badge unbacked | **High** |
| D-11 | Agent spawn never verified end-to-end | `SELFDOGFOODING_REPORT.md` — no agent CLI ever completed a task; PTY spawn, caps, watcher unverified | **Critical** |

D-01…D-05, D-07…D-10 are **defects** (fix is known). D-06 and D-11 are **design gaps** (require decisions + new architecture before coding).

---

## 2. Scope

**In scope:** everything needed to make the product (a) buildable, (b) wired so no advertised feature silently fails, (c) able to execute the promised outcome (spec → provision → agents → verify → deploy) as one supervised run, (d) provable by tests and one real end-to-end run.

**Out of scope (v1):** multi-project concurrent builds, cloud providers beyond Supabase/Vercel, team/cloud sync, VS Code extension, auto-update (tracked as a release task, not a gate).

---

## 3. IPC Contract (design)

### 3.1 Naming and registration rule (D-03)

- **R-1:** Every Tauri command is named `<verb>_<noun>_cmd` (existing convention; 179/179 registered commands with suffixes follow it).
- **R-2:** The frontend never writes a command name inline. Names live in `src/lib/ipc/commands.ts` as `const IPC = { ... } as const`; all stores/pages import from it.
- **R-3:** A contract test enforces bidirectional truth:
  - Parse `lib.rs` handler block `[103..~310]` → set of registered names.
  - Read `IPC` values → set of invoked names.
  - Fail if `invoked − registered ≠ ∅` (dead invokes) with the exact list.
  - Report `registered − invoked` as a snapshot section (unused commands are allowed but must not grow silently).
- **R-4:** The contract test also greps `src/**` for `invoke(` with a string literal and fails if found (forces R-2).
- **R-5:** Command wrappers for existing library functions go in **new module files** (e.g., `compounder_commands.rs`) so feature agents never edit `commands.rs` (conflict-free parallel merge, §4).

### 3.2 Dead invokes to eliminate (D-03, D-04)

Frontend call (wrong) → declared backend symbol → resolution:

| Frontend invoke | Backend exists as | Fix |
|---|---|---|
| `get_chat_platform_configs` | `get_chat_platform_configs_cmd` | frontend → `_cmd` |
| `save_chat_platform_config` | `save_chat_platform_config_cmd` | frontend → `_cmd` |
| `delete_chat_platform_config` | `delete_chat_platform_config_cmd` | frontend → `_cmd` |
| `toggle_chat_platform_config` | `toggle_chat_platform_config_cmd` | frontend → `_cmd` |
| `start_backward_channel_daemon` | `start_backward_channel_daemon_cmd` | frontend → `_cmd` |
| `stop_backward_channel_daemon` | `stop_backward_channel_daemon_cmd` | frontend → `_cmd` |
| `get_backward_channel_daemon_status` | `get_backward_channel_daemon_status_cmd` | frontend → `_cmd` |
| `get_backward_channel_daemon_logs` | `get_backward_channel_daemon_logs_cmd` | frontend → `_cmd` |
| `check_backward_channel_queue_health` | `check_backward_channel_queue_health_cmd` | frontend → `_cmd` |
| `test_chat_platform_connection` | `test_chat_platform_connection_cmd` | frontend → `_cmd` |
| `get_cost_summary` | `get_cost_summary_cmd` | frontend → `_cmd` |
| `get_preflight_warnings_cmd` | `knowledge::get_preflight_warnings` (no command) | add wrapper in new `knowledge_commands.rs` |
| `run_compounder_cmd` | `knowledge::run_compounder` (no command) | add wrapper + call path (DG-3) |

### 3.3 Migration numbering (conflict-free)

| Wave | Reserved numbers |
|---|---|
| B (wiring) | none |
| C (pipeline) | `016`–`024` |
| D (reliability) | `025`–`029` |
| E (hardening) | `030`+ |

Integrator owns `db.rs` migration registration and the `migrations/` directory; feature agents ship SQL as a file + a registration line in their handoff.

---

## 4. Parallel subagent protocol (no conflicts)

### 4.1 Topology per wave

```
Wave Lead (1, human or orchestrator)
├── Integrator (1)        — exclusive owner of shared files; merges in dependency order
├── Feature Agent ×N       — exclusive owner of their module + test files
└── Verifier (1)           — runs wave gate commands on the integration branch; vetoes
```

### 4.2 Ownership rules

1. **One writer per file, per wave.** Ownership matrix is in `TDD_PLAN.md` §Ownership.
2. **Shared-file set (Integrator only):** `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/db.rs`, `src-tauri/src/main.rs`, `src/App.tsx`, `src/components/layout/Sidebar.tsx`, `package.json`, `src-tauri/Cargo.toml`, `Cargo.toml`, `.github/workflows/*`.
3. **Registration patches:** a feature agent needing shared-file edits writes `docs/2026-09-26-defect-closure/patches/<wave>-<step>.registration.txt` with exact insert lines; Integrator applies. Agents never touch shared files.
4. **Migrations:** only via reserved number ranges (§3.3).
5. **Test files:** owned by the step that creates them; nobody centralizes test utilities unless assigned (`src/__tests__/helpers/ipc.ts` is owned by Step 2.1).
6. **Isolation:** every step runs in its own worktree branch `wave/<wave>/<step>` created from `wave/<wave>/base`.
7. **Merge order:** topological by `Depends on`. After every merge, Integrator runs the wave gate; a red gate blocks the next merge.
8. **Conflict protocol:** if a feature branch conflicts with integration (should not happen under rule 1), the Integrator rebases — the feature agent never force-pushes.

### 4.3 Artifact contract per step (subagent DoD)

Each step MUST produce:

1. A failing test first (**RED evidence** — paste of the failing run).
2. Implementation (**GREEN evidence** — paste of the passing run).
3. `HANDOFF_<step>.md` with the 6 required sections (`Original Task`, `Completed By`, `Model Used`, `Output Summary`, `Files Changed`, `Handoff Instructions`) — parsable by `handoff_parser.rs`.
4. An extra `## Verification Evidence` section (allowed: parser ignores unknown sections) with exact commands + observed results.
5. No edits outside owned files.

---

## 5. Design gaps (the part that needs decisions)

### DG-1: Supervised Build Pipeline (`build_app`) — closes D-06

**Decision:** introduce a persisted, resumable, cancellable stage machine — not a fire-and-forget function chain.

```
build_app_cmd(project_id, spec_path, project_path, options) -> BuildRunId
  stages: parse_spec → resolve_stack → provision → scaffold → seed_plan
        → execute_waves → finalize_and_verify → deploy → report
```

- **Persistence:** new table `build_runs` (id, project_id, spec_path, project_path, stack_id, status, current_stage, stage_log JSON, report JSON, error, created_at, updated_at). Each stage is idempotent and writes `stage_log`.
- **Events:** `build-app-progress` (stage, status, message) emitted per transition; frontend progress UI subscribes once.
- **Failure policy:**
  - `parse_spec` / `resolve_stack` failures → run `failed` (hard stop).
  - `provision` missing CLI / no Supabase MCP → creates a `DelegationTask` (existing `delegation.rs`) and pauses the run in `awaiting_user`; resumes via `resume_build_app_cmd(run_id)`.
  - `execute_waves` → per-agent deadline/retry from `wave_executor`; wave failure does not auto-abort the run, it marks the wave failed and continues non-dependent waves.
  - `finalize_and_verify` → verification report is **blocking** for `deploy` unless `options.allow_deploy_on_failed_verification = true`.
  - `deploy` failures → run `failed` with deploy log; retry via `resume_build_app_cmd`.
- **Seams (testability):** `Deployer` trait (Vercel impl + `MockDeployer`), `LlmClient` trait for the compounder (OpenRouter impl + `MockLlm`), `AgentAdapter` (already exists: opencode + mock). No network in unit tests.
- **Idempotency:** re-running `build_app` on the same `project_path` resumes the existing non-terminal run instead of duplicating it.
- **Cancellation:** `cancel_build_app_cmd(run_id)` flips a token checked between stages and kills spawned PTYs via `PtyManager`.
- **Non-goals:** parallel builds of different projects in one run; provisioning providers other than Supabase; deploy targets other than Vercel in v1.

**BuildReport schema (stable contract):**
```json
{
  "run_id": "...", "status": "succeeded|failed|awaiting_user|cancelled",
  "stack_id": "nextjs-supabase-vercel",
  "stages": [{"name":"execute_waves","status":"done","started_at":"...","ended_at":"..."}],
  "waves": [{"plan_id":"...","agents":[{"agent_ref":"...","status":"done"}]}],
  "verification": {"passed": true, "checks": [{"name":"build","passed":true,"detail":"..."}]},
  "deploy": {"url": "https://...","provider":"vercel"},
  "artifacts": {"project_path": "...", "report_path": "..."}
}
```

### DG-2: IPC contract enforcement — closes D-03 (see §3)

**Decision:** contract test is the invariant; `IPC` constants module is the only allowed name source. Adding a command = (a) Rust registration, (b) `IPC` constant, (c) contract test green.

### DG-3: Knowledge Compounder call path — closes D-04

**Decision:** the compounder runs **automatically after each finalized+verified wave** and remains manually runnable.

- **Auto:** `finalize_and_verify` spawns `tokio::spawn(run_compounder(session_ids_of_wave))`; result is recorded; wave status is unaffected by compounder failure (recorded in `compounder_runs`).
- **Manual:** `run_compounder_cmd(session_id, project_id) -> Vec<KnowledgeItem>` exposed via new `compounder_commands.rs` wrapper.
- **Idempotency/dedupe:** key = `(session_id, project_id)`; re-runs upsert confidence/confirmation counts rather than duplicating items.
- **Failure semantics:** `get_compounder_status_cmd.health = 'error'` + `last_error`; never blocks wave completion.

### DG-4: Verification as the finalize gate — closes D-05

**Decision:** `finalize_wave_with_verify` (existing `wave_executor.rs:203`) becomes the **only** finalize path used by UI and pipeline. `finalize_wave_cmd` stays for compatibility but the UI calls `verify_and_finalize_wave_cmd`.

- Verification report persisted in `verification_reports` (wave/plan id, JSON, passed, created_at) and surfaced per wave in Orchestrate/Outcomes.
- `generate_deploy_config` runs automatically when verification finds a framework needing config (Vite/React + Vercel → `vercel.json` SPA rewrites) — the documented NexusBoard failure mode.
- A wave with `verify_deploy = true` (spec_parser) blocks plan completion on `verification.passed = false`.

### DG-5: Agent execution reliability — closes D-11

**Decision:** the wave executor owns **non-blocking** spawns and a **handoff watcher** decides completion; do not rely on foreground process exit.

- **Watcher:** poll `HANDOFF_<agent_ref>.md` every 2s per running agent; completion = valid handoff (existing parser) **or** deadline timeout (existing `deadline_secs`) **or** cost cap (`cost_cap_usd`).
- Timeout → kill PTY → agent `failed` → create correction doc (existing correction mechanism) → optional 1 auto-retry (`retry_count`), then stop.
- **E2E probe:** `tools/dogfood.rs` gains `--agent <cmd>` to spawn a real CLI against a fixture repo and assert a valid handoff is produced; this is the gate for D-11 (env-gated in CI, required manually before release).
- Adapters beyond opencode/mock stay out of scope; PTY generic spawn already covers all 9 CLIs by command string.

### DG-6: Build/toolchain & CI strategy — closes D-10

- **Decision:** MSVC is the canonical Windows toolchain. Add `rust-toolchain.toml` at repo root (`channel = "stable"`, target `x86_64-pc-windows-msvc` on Windows) and document VS Build Tools as prerequisite. The GNU/minGW path is unsupported (no `dlltool`).
- **CI (`ci.yml`, on push/PR):** 4 jobs, all required:
  1. `web`: `npm ci && npm run lint:src && npx tsc --noEmit && npx vitest run`
  2. `rust`: matrix `windows-latest` + `ubuntu-latest`: `cargo test --workspace`
  3. `python`: `pytest -q` in `local-daemon/` and `webhook-server/`
  4. `contract`: `npx vitest run src/__tests__/contracts/ipc-contract.test.ts`
- `release.yml` gets a `needs: ci`-equivalent gate (workflow_run) so tags cannot ship a red build.

### DG-7: Security & release hardening — closes D-07

- Restore CSP from `f4ae90b` (with `ipc:`/`http://ipc.localhost` already included) and add `connect-src` entries for OpenRouter if used from the webview; prove dev + packaged app still work before merge.
- Add React Error Boundary wrapping every route (root + per-page).
- Capability minimization review: drop unused `shell:*`, keep `shell:allow-execute` only if PTY needs it (PTY uses Rust-side `portable-pty`, so frontend shell may be removable).
- No auto-update in v1 (decision deferred to Open Questions).

---

## 6. Open questions (RESOLVED 2026-09-26 — decisions below are binding for Wave C)

1. **Deploy target v1:** **Vercel + Dockerfile artifact** — run `vercel --prod --yes` for Vercel-target stacks AND generate a generic `Dockerfile` artifact for non-Vercel hosts (not executed in v1).
2. **Supabase provisioning:** **Connected Supabase MCP first** — the pipeline detects an MCP session and delegates `apply_migration`; otherwise it pauses the run as `awaiting_user` (no new credentials stored).
3. **Compounder trigger:** **Auto after each verified wave + manual** — `tokio::spawn` after finalize+verify using the split API; failures never block the wave; manual `run_compounder_cmd` stays available.
4. **Real-CLI E2E in CI:** **Optional CI job with provider key** — a non-blocking job runs one real agent using a repository secret; the deterministic stub-CLI E2E stays required.

---

## 7. Success criteria for this spec

- [ ] `cargo +stable-x86_64-pc-windows-msvc test --workspace` green (D-01)
- [ ] `npx tsc --noEmit` green and `npm run build` produces `dist/` (D-02)
- [ ] IPC contract test green; 0 dead invokes (D-03, D-04)
- [ ] `build_app_cmd` executes the full stage machine against a fixture project with MockAdapter and returns a `BuildReport` with all stages `done` (D-06)
- [ ] Compounder items created automatically after a verified wave (D-04, DG-3)
- [ ] Verification report attached to every wave in the UI; SPA-config auto-generated when needed (D-05)
- [ ] One real agent CLI completes one task through the orchestrator with a parsed handoff (D-11)
- [ ] CI green on push; `csp` no longer null (D-07, D-10)
- [ ] All Python + JS suites green, `npm run lint` green (D-08)

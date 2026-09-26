# TDD-PLAN-001: Defect Closure & Pipeline (Parallel Subagent Waves)

**Date:** 2026-09-26
**Spec:** `docs/2026-09-26-defect-closure/SPEC.md` (SPEC-001)
**Methodology:** RED → GREEN → REFACTOR per step. Each step is a subagent work item with exclusive file ownership (SPEC-001 §4).
**Seeding:** headers are `## Phase N` + `### Step X.Y` so `spec_parser::parse_gap_closure_plan` can ingest this file (Step 1.5 upgrades the parser to honor the explicit `**Wave:**` / `**Depends on:**` markers).

**Phase ↔ wave map:** Phase 1 = Wave A (serial) · Phase 2 = Wave B · Phase 3 = Wave C · Phase 4 = Wave D · Phase 5 = Wave E.

**Global gates (run by Verifier on `wave/<X>/base` before merge):**
```powershell
npx tsc --noEmit
npx vitest run
cargo +stable-x86_64-pc-windows-msvc test --workspace      # from repo root
python -m pytest -q                                        # run in local-daemon/ and webhook-server/
node scripts/check-ipc-contract.mjs                        # generated in Step 2.1, aliased by contract test
```

---

## Phase 1: Unblock — Build Green (Wave A, serial)

### Step 1.1: Rust library compiles (fix E0015 in Stack Registry)

**Wave:** A · **Depends on:** — · **Owner:** Rust agent
**RED:** `cargo +stable-x86_64-pc-windows-msvc check --workspace` currently fails with 72 × E0015.
Before fixing, add tests to `stack_registry.rs` that cannot compile today:
`test_registry_has_four_stacks`, `test_stack_ids_unique`, `test_default_stack_is_nextjs_supabase_vercel`, `test_get_by_id_roundtrip`.
**EVIDENCE (RED):** paste of `cargo check` output (72 errors) + test compile failure.
**GREEN:** replace `pub static STACK_REGISTRY: &[StackPreset]` (`stack_registry.rs:34-87`) with `static STACK_REGISTRY: LazyLock<Vec<StackPreset>> = LazyLock::new(|| vec![...])`; keep `StackPreset::all()`, `default_stack()`, `get_by_id()` signatures unchanged (no caller edits).
**AC:**
- [ ] `cargo +stable-x86_64-pc-windows-msvc check --workspace` → exit 0, zero `error[E0015]`
- [ ] `cargo +stable-x86_64-pc-windows-msvc test -p sourceforge --lib` → 4 new tests pass, 0 failures
- [ ] `list_available_stacks_cmd` returns 4 stacks (unit test, in-memory DB not required)
**Files (owner):** modify `src-tauri/src/stack_registry.rs` only.

### Step 1.2: Frontend typecheck + build green (fix Settings store)

**Wave:** A · **Depends on:** — · **Owner:** FE agent
**RED:** add `src/__tests__/stores/settingsStore.test.ts` case `updateDefaults persists defaultStack via set_preferences_cmd` and `src/__tests__/pages/Settings.test.tsx` case `clicking a stack card selects it` — both fail (`updateDefaults` missing; selection static).
Run: `npx vitest run src/__tests__/stores/settingsStore.test.ts src/__tests__/pages/Settings.test.tsx`
**GREEN:** add `updateDefaults(partial)` + `loadDefaults()` to `src/stores/settingsStore.ts` calling `set_preferences_cmd` / `get_preferences_cmd` (registered at `lib.rs:124-125`); `Settings.tsx:303-327` renders selected stack from store (remove hardcoded `def: true`).
**AC:**
- [ ] `npx tsc --noEmit` → exit 0 (D-02 gone)
- [ ] `npx vitest run` → ≥311 passing, 0 failing (the 2 new cases included)
- [ ] `npm run build` → exit 0, `dist/` written
- [ ] Manual: select stack #2 → reload app → stack #2 still selected
**Files (owner):** modify `src/stores/settingsStore.ts`, `src/pages/Settings.tsx`; create `src/__tests__/stores/settingsStore.test.ts`, `src/__tests__/pages/Settings.test.tsx` cases.

### Step 1.3: Toolchain pin + CI gates

**Wave:** A · **Depends on:** 1.1, 1.2 · **Owner:** Integrator
**RED:** `cargo test` on a clean clone fails (no pin) and no workflow runs on PR — verify `.github/workflows/ci.yml` absence.
**GREEN:** create `rust-toolchain.toml` (`[toolchain] channel = "stable"`), `.github/workflows/ci.yml` with 4 required jobs (web / rust matrix win+linux / python / ipc-contract), add `lint:src` script (`eslint src`), and set `release.yml` to `on: workflow_run` of CI success.
**AC:**
- [ ] CI workflow appears on PR and all 4 jobs green
- [ ] `cargo +stable-x86_64-pc-windows-msvc test --workspace` exit 0 on clean clone
- [ ] README badge updated to the CI workflow (or removed until green)
**Files (owner):** create `rust-toolchain.toml`, `.github/workflows/ci.yml`; modify `package.json`, `.github/workflows/release.yml`, `README.md`.

### Step 1.4: Repo hygiene + lint gate

**Wave:** A · **Depends on:** — · **Owner:** Integrator
**RED:** `npm run lint` → 3,513 errors (all from `target/`); `git ls-files -s .worktrees` → 4 × mode 160000; `.gitignore` tail is UTF-16.
**GREEN:** eslint ignores `target/`, `dist/`, `.worktrees/`; rewrite `.gitignore` as UTF-8; `git rm --cached .worktrees/dogfood--*`; `git worktree prune`; delete stale `dogfood/*` branches.
**AC:**
- [ ] `npm run lint` → exit 0 (`eslint src` 0 errors; repo-wide ≤ 0 errors with ignores)
- [ ] `git status --porcelain` → no `.worktrees/*` entries
- [ ] `.gitignore` parses as UTF-8 (`git check-ignore .worktrees/x` succeeds)
**Files (owner):** modify `eslint.config.js`, `.gitignore`; index-only changes via `git rm --cached`.

### Step 1.5: Spec parser honors explicit wave/dependency markers

**Wave:** A · **Depends on:** — · **Owner:** Rust agent
**RED:** tests in `spec_parser.rs`: `test_parse_reads_wave_marker` (`**Wave:** C` → `task.wave == 3`), `test_parse_reads_depends_marker` (`**Depends on:** 2.1` → `Some("2.1")`), `test_wave_falls_back_to_phase_when_absent`. Current parser fails all three (hardcoded phase mapping `spec_parser.rs:97-102`, naive depends heuristic `:124-131`).
**GREEN:** extend `build_task` to parse `**Wave:**` letter (A=1…E=5) and `**Depends on:** X.Y`; keep phase fallback for old docs.
**AC:**
- [ ] `cargo test spec_parser` → 8 existing + 3 new tests pass
- [ ] `cargo run --bin dogfood -- . docs/2026-09-26-defect-closure/TDD_PLAN.md` prints all Phase 1–5 tasks with correct waves
**Files (owner):** modify `src-tauri/src/spec_parser.rs`, `tools/dogfood.rs`.

---

## Phase 2: Wiring & Contract (Wave B — 5 parallel agents after 2.1)

### Step 2.1: IPC contract test + canonical command constants (foundation)

**Wave:** B · **Depends on:** 1.1 · **Owner:** FE agent
**RED:** create `src/__tests__/contracts/ipc-contract.test.ts` that (a) parses `lib.rs` handler block, (b) reads `src/lib/ipc/commands.ts`, (c) fails on `invoked − registered ≠ ∅`, (d) fails on literal `invoke("...")` in `src/**`. First run fails listing exactly the 13 names in SPEC-001 §3.2.
**GREEN:** create `src/lib/ipc/commands.ts` exporting every command name used by the app (139 names) as `IPC` constants; no behavior change yet.
**AC:**
- [ ] `npx vitest run src/__tests__/contracts/ipc-contract.test.ts` → fails with the 13 items (RED evidence)
- [ ] after Steps 2.2–2.4 merge: same test passes with empty diff; unused-command snapshot written to `docs/2026-09-26-defect-closure/ipc-unused.snapshot.json`
**Files (owner):** create `src/__tests__/contracts/ipc-contract.test.ts`, `src/lib/ipc/commands.ts`, `scripts/check-ipc-contract.mjs`.
**Handoff:** publish `IPC` constant names for 2.2–2.5 in the handoff (they import, never edit).

### Step 2.2: Backward-channel (Chat tab) restored

**Wave:** B · **Depends on:** 2.1 · **Owner:** FE agent
**RED:** `src/__tests__/stores/backwardChannelStore.test.ts` — `saveChatPlatformConfig invokes save_chat_platform_config_cmd` (+ same for the other 9 names); fails against current store.
**GREEN:** `src/stores/backwardChannelStore.ts` uses `IPC.*` constants for all 10 calls (SPEC-001 §3.2).
**AC:**
- [ ] `npx vitest run src/__tests__/stores/backwardChannelStore.test.ts` → 10 new cases pass
- [ ] `npx vitest run src/__tests__/pages/Integrations.test.tsx` → pass
- [ ] contract test dead-list no longer contains any `chat_platform|backward_channel` name
**Files (owner):** modify `src/stores/backwardChannelStore.ts`, `src/__tests__/stores/backwardChannelStore.test.ts`; may modify `src/__tests__/pages/Integrations.test.tsx`.

### Step 2.3: Costs page restored

**Wave:** B · **Depends on:** 2.1 · **Owner:** FE agent
**RED:** `CostAggregation.test.tsx` case `loads summary via get_cost_summary_cmd` fails today.
**GREEN:** `CostAggregation.tsx:61` → `IPC.getCostSummary`.
**AC:**
- [ ] `npx vitest run src/__tests__/pages/CostAggregation.test.tsx` → pass
- [ ] contract test dead-list no longer contains `get_cost_summary`
**Files (owner):** modify `src/pages/CostAggregation.tsx`, `src/__tests__/pages/CostAggregation.test.tsx`.

### Step 2.4: Compounder + preflight commands exposed (Rust wrappers, no `commands.rs` edit)

**Wave:** B · **Depends on:** 2.1 (constants only for FE side), 1.5 · **Owner:** Rust agent
**RED:** new `src-tauri/src/knowledge_commands.rs` with `#[cfg(test)]` tests: `test_run_compounder_cmd_returns_items_with_mock_llm`, `test_get_preflight_warnings_cmd_returns_rows`. Extract an `LlmClient` seam (`compounder_llm.rs`: trait + `OpenRouterLlm` + `MockLlm`) so `knowledge::run_compounder` is testable without network.
**GREEN:** wrappers `run_compounder_cmd`, `get_preflight_warnings_cmd` in `knowledge_commands.rs`; registration patch file for Integrator (`mod knowledge_commands;` + 2 handler lines).
**AC:**
- [ ] `cargo test knowledge_commands` → 2 new tests pass, no network
- [ ] contract test dead-list is empty (both names now registered)
- [ ] `knowledgeStore.runCompounder` works against `run_compounder_cmd` (FE store unchanged, name now real)
**Files (owner):** create `src-tauri/src/knowledge_commands.rs`, `src-tauri/src/compounder_llm.rs`; **Integrator** applies `src-tauri/src/lib.rs` patch.

### Step 2.5: Verification gate wired into the UI finalize path

**Wave:** B · **Depends on:** 2.1 · **Owner:** FE agent
**RED:** `orchestrationStore.test.ts` case `finalizeWave calls verify_and_finalize_wave_cmd with project_path` fails (currently calls `finalize_wave_cmd`).
**GREEN:** `src/stores/orchestrationStore.ts:254` → `IPC.verifyAndFinalizeWave`; render returned `verification.passed` + failing checks in the Orchestrate wave result panel (read-only, no new page).
**AC:**
- [ ] `npx vitest run src/__tests__/stores/orchestrationStore.test.ts` → new case passes
- [ ] contract test lists `verify_and_finalize_wave_cmd` as invoked
- [ ] manual: finalize a mock wave without `vercel.json` → UI shows `verification.passed=false` with the missing-config check
**Files (owner):** modify `src/stores/orchestrationStore.ts`, wave-result panel inside `src/pages/Orchestrate.tsx`, its test.
**Note:** report persistence is Step 3.10; this step only surfaces the returned report.

### Step 2.6: Flaky/failing JS tests repaired

**Wave:** B · **Depends on:** — · **Owner:** QA agent
**RED:** `npx vitest run src/__tests__/App.test.tsx` → `redirects /connectors → /integrations` fails (`getByText(/Integrations/i)` matches sidebar + heading); full suite logs 7 uncaught `Cannot read properties of undefined (reading 'filter')` from `Messages.tsx:57`.
**GREEN:** assert on the page heading element specifically (`getByRole("heading", ...)`); guard `orchestrationStore.getOpenSignals` with `set({ acbSignals: signals ?? [] })` and `MessagePanel` with `const signals = store.acbSignals ?? []`.
**AC:**
- [ ] `npx vitest run` → 0 failing, 0 uncaught exceptions in output
**Files (owner):** modify `src/__tests__/App.test.tsx`, `src/stores/orchestrationStore.ts`, `src/pages/Messages.tsx`.

### Step 2.7: Python suites green

**Wave:** B · **Depends on:** — · **Owner:** Python agent
**RED:** `python -m pytest -q` in `local-daemon/` → 13 errors; in `webhook-server/` → collection error `No module named 'webhook_server'`.
**GREEN:** add `pyproject.toml` (or `conftest.py` sys.path) so `webhook_server` resolves (package dir is `webhook-server/`); fix local-daemon `conftest.py:44` import.
**AC:**
- [ ] `python -m pytest -q` in both dirs → exit 0, 0 errors
**Files (owner):** create `webhook-server/pyproject.toml` (+ rename packaging if chosen); modify `local-daemon/tests/conftest.py`.

---

## Phase 3: Supervised Build Pipeline (Wave C — 5 parallel agents; SPEC-001 DG-1)

### Step 3.1: `build_runs` persistence

**Wave:** C · **Depends on:** 2.1 · **Owner:** Integrator
**RED:** `pipeline_store.rs` unit tests: `test_create_and_get_build_run`, `test_stage_log_appends`, `test_resume_returns_non_terminal_run`, `test_cancel_sets_cancelled` — fail before table exists.
**GREEN:** migration `016_build_runs.sql`; `pipeline_store.rs` CRUD; register migration in `db.rs` (Integrator).
**AC:**
- [ ] `cargo test pipeline_store` → 4 tests pass (fresh in-memory DB)
- [ ] re-running migration on existing DB is idempotent (`IF NOT EXISTS`), verified by test
**Files (owner):** create `src-tauri/migrations/016_build_runs.sql`, `src-tauri/src/pipeline_store.rs`; Integrator edits `db.rs`.

### Step 3.2: Stage machine skeleton + seams

**Wave:** C · **Depends on:** 3.1 · **Owner:** Rust agent
**RED:** `pipeline.rs` tests: `test_stage_order_is_parse_provision_scaffold_seed_execute_finalize_deploy_report`, `test_failed_hard_stage_marks_run_failed`, `test_awaiting_user_stage_pauses_run`, `test_progress_events_emitted_per_transition` — fail before implementation. Use `MockDeployer`, `MockLlm`, `MockAgentAdapter` (no network, temp dirs).
**GREEN:** `pipeline.rs` — `BuildStage` enum, `run_stage()`, state transitions, cancellation token, event emission (`build-app-progress`).
**AC:**
- [ ] `cargo test pipeline` → 4 tests pass
- [ ] no `reqwest`/`Command` calls in `pipeline.rs` except behind seams (grep-based test)
**Files (owner):** create `src-tauri/src/pipeline.rs`, `src-tauri/src/deployer.rs` (trait + `VercelDeployer` + `MockDeployer`).

### Step 3.3: Provision stage (with delegation pause)

**Wave:** C · **Depends on:** 3.2 · **Owner:** Rust agent
**RED:** `test_provision_missing_cli_creates_delegation_and_awaits_user`, `test_provision_all_present_continues`, `test_resume_build_app_continues_from_provision`.
**GREEN:** provision stage calls `provisioner::check_cli_status`; missing CLI → `delegation::create_delegation` + status `awaiting_user`; `resume` re-enters the same stage idempotently.
**AC:**
- [ ] `cargo test pipeline::provision` → 3 tests pass
- [ ] state after missing CLI = `awaiting_user`, stage_log last entry = `provision`
**Files (owner):** modify `src-tauri/src/pipeline.rs`, `src-tauri/src/provisioner.rs` (+ its tests).

### Step 3.4: Scaffold stage

**Wave:** C · **Depends on:** 3.2 · **Owner:** Rust agent
**RED:** `test_scaffold_stage_invokes_arch_engine_for_stack`, `test_scaffold_skipped_when_project_nonempty`.
**GREEN:** scaffold stage → `arch_engine::scaffold_project(stack_id, path, name)`; guard: non-empty dir + no `package.json` marker → skip with warning.
**AC:**
- [ ] `cargo test pipeline::scaffold` → 2 tests pass
- [ ] fixture temp dir contains expected files after stage
**Files (owner):** modify `src-tauri/src/pipeline.rs`, `src-tauri/src/arch_engine.rs`.

### Step 3.5: Execute → finalize → verify stage + compounder hook

**Wave:** C · **Depends on:** 3.2, 2.4, 2.5 · **Owner:** Rust agent
**RED:** `test_execute_closes_wave_via_verify_and_finalize`, `test_compounder_spawned_after_verified_wave`, `test_compounder_failure_does_not_fail_run`.
**GREEN:** stage calls `wave_executor::execute_wave_with_adapters` (mock adapter), waits for handoffs (stub polling), then `finalize_wave_with_verify`; on success `tokio::spawn(knowledge::run_compounder(...))` recording into `compounder_runs` (migration `017_compounder_runs.sql`).
**AC:**
- [ ] `cargo test pipeline::execute` → 3 tests pass
- [ ] `knowledge_items` table gains rows after the stage in the integration test
- [ ] compounder error path leaves run `status=succeeded` with a warning entry
**Files (owner):** modify `src-tauri/src/pipeline.rs`; create `017_compounder_runs.sql`; Integrator edits `db.rs`.

### Step 3.6: Deploy stage

**Wave:** C · **Depends on:** 3.2, 3.5 · **Owner:** Rust agent
**RED:** `test_deploy_skipped_when_verification_failed_by_default`, `test_deploy_proceeds_with_override_flag`, `test_deploy_uses_mock_deployer_url`.
**GREEN:** deploy stage behind `Deployer` trait; gating on `verification.passed`; URL recorded in `BuildReport.deploy`.
**AC:**
- [ ] `cargo test pipeline::deploy` → 3 tests pass
- [ ] `BuildReport` JSON matches SPEC-001 §5 schema (serde snapshot test)
**Files (owner):** modify `src-tauri/src/pipeline.rs`, `src-tauri/src/deployer.rs`.

### Step 3.7: Pipeline commands

**Wave:** C · **Depends on:** 3.3, 3.4, 3.5, 3.6 · **Owner:** Rust agent (Integrator registration)
**RED:** `pipeline_commands.rs` tests: `test_build_app_cmd_returns_run_id`, `test_resume_build_app_cmd`, `test_cancel_build_app_cmd`, `test_build_app_status_cmd`.
**GREEN:** `build_app_cmd`, `resume_build_app_cmd`, `cancel_build_app_cmd`, `get_build_app_status_cmd` in new `pipeline_commands.rs`; registration patch for Integrator; `IPC` constants + contract test update (one writer: this step).
**AC:**
- [ ] `cargo test pipeline_commands` → 4 tests pass
- [ ] IPC contract test green with 4 new names
**Files (owner):** create `src-tauri/src/pipeline_commands.rs`; modify `src/lib/ipc/commands.ts`, `src/__tests__/contracts/ipc-contract.test.ts`; Integrator edits `lib.rs`.

### Step 3.8: Build App frontend (button + progress + report)

**Wave:** C · **Depends on:** 3.7 · **Owner:** FE agent
**RED:** `src/__tests__/pages/BuildApp.test.tsx` — `renders stage timeline`, `button disabled while running`, `shows deploy url on success`, `shows failing checks on verification failure` (invoke mocked).
**GREEN:** new `src/pages/BuildApp.tsx` (route `/build`, Sidebar entry via Integrator), subscribes to `build-app-progress`, renders `BuildReport`.
**AC:**
- [ ] `npx vitest run src/__tests__/pages/BuildApp.test.tsx` → 4 cases pass
- [ ] `npx tsc --noEmit` exit 0; contract test green
**Files (owner):** create `src/pages/BuildApp.tsx`, `src/__tests__/pages/BuildApp.test.tsx`, `src/lib/types/pipeline.ts`; Integrator edits `src/App.tsx`, `Sidebar.tsx`.

### Step 3.9: Pipeline E2E integration test (the "promised outcome" gate)

**Wave:** C · **Depends on:** 3.7 · **Owner:** Verifier
**RED:** `tests/pipeline_e2e.rs` (Rust integration): `test_full_pipeline_with_mock_adapter_produces_complete_report` — create temp project + tiny spec, run all stages with mocks, assert every stage `done`, verification attached, `BuildReport.deploy.url` present, knowledge items > 0.
**GREEN:** wire missing pieces until it passes (small fixes only).
**AC:**
- [ ] `cargo test --test pipeline_e2e` → pass, prints full stage timeline
- [ ] re-running the same command resumes (no duplicate run row)
**Files (owner):** create `src-tauri/tests/pipeline_e2e.rs`.

### Step 3.10: Verification report persistence + surfacing

**Wave:** C · **Depends on:** 2.5 · **Owner:** Rust agent
**RED:** `test_verification_report_persisted_per_wave`, `test_wave_list_exposes_passed_flag`.
**GREEN:** migration `018_verification_reports.sql`; persist in `verify_and_finalize_wave_cmd`; Orchestrate wave list shows pass/fail badge (FE part owned by same agent for this step).
**AC:**
- [ ] `cargo test verification_reports` → 2 pass; `npx vitest run src/__tests__/pages/OrchestrateTabs.test.tsx` → pass
**Files (owner):** create `018_verification_reports.sql`; modify `src-tauri/src/commands.rs` (Integrator), `src/pages/Orchestrate.tsx`; Integrator edits `db.rs`.

---

## Phase 4: Reliability & Real-Agent E2E (Wave D — 3 parallel agents; SPEC-001 DG-5)

### Step 4.1: Handoff watcher

**Wave:** D · **Depends on:** 3.9 · **Owner:** Rust agent
**RED:** `test_watcher_marks_done_when_handoff_valid`, `test_watcher_marks_failed_after_deadline`, `test_watcher_ignores_incomplete_handoff` — using a stub CLI script (`tools/fixtures/stub-agent.ps1`) that writes a valid handoff after N seconds.
**GREEN:** poll loop (2s) in `wave_executor.rs`; completion via `handoff_parser::parse_handoff_file`; deadline kill via `PtyManager`.
**AC:**
- [ ] `cargo test wave_executor::watcher` → 3 pass
- [ ] no foreground blocking spawn remains in `wave_executor.rs` (grep test)
**Files (owner):** modify `src-tauri/src/wave_executor.rs`, `src-tauri/src/pty.rs` (kill path); create `tools/fixtures/stub-agent.ps1`.

### Step 4.2: Retry + correction loop

**Wave:** D · **Depends on:** 4.1 · **Owner:** Rust agent
**RED:** `test_failed_agent_gets_one_retry`, `test_second_failure_creates_correction_doc`, `test_retry_respects_cost_cap`.
**GREEN:** use existing correction docs + `retry_count`; cap at 1 retry; cost/deadline checks before retry.
**AC:**
- [ ] `cargo test wave_executor::retry` → 3 pass
**Files (owner):** modify `src-tauri/src/wave_executor.rs`, `src-tauri/src/orchestrator.rs` (correction write only).

### Step 4.3: Cancellation kills PTYs

**Wave:** D · **Depends on:** 4.1 · **Owner:** Rust agent
**RED:** `test_cancel_build_app_kills_running_ptys` (stub CLI sleeps; after cancel, process gone).
**GREEN:** cancellation token checked by watcher loop + `PtyManager` kill all sessions of the run.
**AC:**
- [ ] `cargo test pipeline::cancel` → pass; no orphan `stub-agent` processes after test (assert via registry empty)
**Files (owner):** modify `src-tauri/src/pipeline.rs`, `src-tauri/src/pty.rs`.

### Step 4.4: Dogfood E2E probe with a real CLI

**Wave:** D · **Depends on:** 4.2 · **Owner:** Verifier
**RED:** `tools/dogfood.rs --agent opencode` on a fixture repo currently fails/does nothing.
**GREEN:** implement `--agent <cmd>` path: create worktree → spawn → wait handoff → parse → print report; exit 0 only on valid handoff.
**AC:**
- [ ] `cargo run --bin dogfood -- --agent "pwsh tools/fixtures/stub-agent.ps1" <fixture>` → exit 0 (deterministic, CI-runnable)
- [ ] `cargo run --bin dogfood -- --agent opencode <fixture>` → exit 0 with real OpenCode CLI (manual, release gate; evidence pasted)
**Files (owner):** modify `tools/dogfood.rs`; create `tools/fixtures/fixture-repo/` (10-line repo + task).

---

## Phase 5: Hardening & Release (Wave E — 4 parallel agents)

### Step 5.1: CSP restored and proven

**Wave:** E · **Depends on:** 3.8 · **Owner:** FE+Rust agent
**RED:** `test_tauri_conf_csp_is_not_null` (node script) fails.
**GREEN:** restore the `f4ae90b` CSP string in `tauri.conf.json`; add `connect-src` entries needed by OpenRouter **only if** the webview calls it directly (Rust-side HTTP does not need it).
**AC:**
- [ ] node check exits 0; `npx tauri dev` loads Runner + Settings with 0 console CSP violations (manual, screenshot in handoff)
**Files (owner):** modify `src-tauri/tauri.conf.json`; create `scripts/check-csp.mjs`.

### Step 5.2: Error boundaries

**Wave:** E · **Depends on:** — · **Owner:** FE agent
**RED:** `ErrorBoundary.test.tsx` — throwing child renders fallback, not white screen.
**GREEN:** `src/components/ErrorBoundary.tsx`; wrap root + each route in `src/App.tsx` (Integrator applies route wrap).
**AC:**
- [ ] `npx vitest run src/__tests__/components/ErrorBoundary.test.tsx` → pass
- [ ] manual: `throw` in a page shows fallback + reload button
**Files (owner):** create `src/components/ErrorBoundary.tsx`, its test; Integrator edits `src/App.tsx`.

### Step 5.3: Capability minimization

**Wave:** E · **Depends on:** 5.1 · **Owner:** Rust agent
**RED:** node script `check-capabilities.mjs` reports shell perms not referenced by frontend code.
**GREEN:** remove unused `shell:*` permissions; keep only what PTY-through-Rust needs (none) and `shell:allow-open` if used.
**AC:**
- [ ] `npx tauri dev` launches; spawn/kill agent works (smoke); script exits 0
**Files (owner):** modify `src-tauri/capabilities/default.json`; create `scripts/check-capabilities.mjs`.

### Step 5.4: Docs complete

**Wave:** E · **Depends on:** 3.8 · **Owner:** Docs agent
**RED:** files absent (`docs/USER_GUIDE.md`, `CHANGELOG.md`, `CONTRIBUTING.md`).
**GREEN:** write user guide (install → build app → verify → deploy), CHANGELOG from tags, CONTRIBUTING.
**AC:**
- [ ] files exist; USER_GUIDE covers the Build App flow with the actual stage names
**Files (owner):** create the 3 files.

### Step 5.5: Release gate

**Wave:** E · **Depends on:** all above · **Owner:** Integrator
**RED:** tag a dry-run `v0.10.0-rc1` — release fails if CI red (expected until gates wired).
**GREEN:** `release.yml` requires CI success; checksums attached; draft release created (not published).
**AC:**
- [ ] draft release `v0.10.0-rc1` has 4 platform artifacts + checksums
- [ ] installing the Windows artifact launches the app (manual, evidence in handoff)
**Files (owner):** modify `.github/workflows/release.yml`.

### Step 5.6: Final acceptance — dogfood a real app end-to-end

**Wave:** E · **Depends on:** 4.4, 5.5 · **Owner:** Verifier
**RED:** NexusBoard (or a fresh fixture app) built through `build_app` reports verification pass but deploy 404s — the historical failure (SPEC-001 D-05).
**GREEN:** run Build App against a new spec; fix any stage that fails; deploy; run the auth flow test.
**AC:**
- [ ] `BuildReport.status == "succeeded"` with deploy URL
- [ ] deployed `/register` returns HTTP 200 (no SPA 404) and register→cookie→me E2E passes (existing auth test adapted to the URL)
- [ ] knowledge items created ≥ 1 from the run
**Files (owner):** no source changes unless a defect is found; evidence only.

---

## Ownership matrix (per wave, exclusive writers)

| Wave | Integrator (shared files) | Feature agents (exclusive files) | Verifier |
|---|---|---|---|
| A | `lib.rs`+`commands.rs` untouched; CI/`package.json`/`.gitignore`/eslint | `stack_registry.rs`, `settingsStore.ts`, `Settings.tsx`, `spec_parser.rs`, `tools/dogfood.rs` | gates |
| B | `lib.rs` (apply 2 Rust patches only) | 2.2 store · 2.3 cost · 2.4 Rust modules + `IPC`/contract (2.1) · 2.5 store+Orchestrate · 2.6 tests+Messages · 2.7 Python | gates + handoff check |
| C | `lib.rs`, `db.rs`, `App.tsx`, `Sidebar.tsx`, `orchestrationStore.ts` merge order | 3.1 store · 3.2–3.6 `pipeline.rs`+`deployer.rs` · 3.7 commands+IPC · 3.8 `BuildApp.tsx` · 3.10 reports | E2E 3.9 |
| D | — | 4.1–4.3 `wave_executor.rs`+`pty.rs` · 4.4 `dogfood.rs` | real-CLI evidence |
| E | `App.tsx`, `release.yml` | 5.1 conf · 5.2 boundary+test · 5.3 capabilities · 5.4 docs | acceptance 5.6 |

**Conflict rules recap (SPEC-001 §4):** one writer per file per wave · feature agents never edit shared files (registration-patch files instead) · migrations only from reserved ranges · merge in dependency order · red gate blocks next merge · Integrator rebases, agents never force-push.

---

## Definition of Done (every step)

- [ ] RED observed and pasted (failing test or failing gate command)
- [ ] GREEN observed and pasted (same command, now passing)
- [ ] Global gate commands run by Verifier on `wave/<X>/base` after merge
- [ ] `HANDOFF_<step>.md` written with 6 required sections + `## Verification Evidence`
- [ ] No file touched outside the ownership line
- [ ] IPC contract test green (any step adding/removing commands)

## Exit criteria (whole plan)

- [ ] All 5 phases merged; CI green on `main`
- [ ] `cargo test --workspace`, `npx vitest run`, `npm run lint`, both pytest suites → all green
- [ ] `build_app_cmd` E2E (mock) green AND one real agent run produces a valid handoff
- [ ] One app deployed by SourceForge is reachable and passes its auth smoke test
- [ ] SPEC-001 §7 checklist fully ticked

# GAP: Deployment Verification Phase Missing in Pipeline

**Root cause of NexusBoard "page not found" on signup.**

## Case Study

NexusBoard was built through SourceForge orchestration (4 waves, 8 agents). All agents completed,
handoffs were validated, TypeScript compiled with 0 errors, production build passed. Deployed to
Vercel — *"The page could not be found"* on `/register`.

### Two Gaps Found

| Gap | Symptom | Root Cause |
|-----|---------|------------|
| SPA Routing | `/register`, `/login`, `/dashboard` all 404 on Vercel | No `vercel.json` with SPA rewrites generated |
| API Backend | Form submits fail — `/api/auth/register` returns 404 | Backend not deployed; frontend assumes proxy |

### Why SourceForge Didn't Catch This

1. **`finalize_wave` only checks handoff file existence** (`wave_executor.rs:162-198`) — structural
   validation of markdown sections. Does NOT verify build output, run smoke tests, or check
   deployed behavior.

2. **No "verify deploy" phase** — Pipeline has 3 phases: `parse spec → execute wave → finalize`.
   Missing: `verify deploy → report`.

3. **`SpecTask` has no verification fields** (`spec_parser.rs:13-22`) — No way to express "after
   this wave, run `npm run build` and check the output."

4. **`handoff_parser` validates structure, not truth** — "Test Results: PASS" is accepted without
   actually running the tests.

## What Needs to Be Added to SourceForge

### Step 1: SpecTask verification field
Add `verification: Option<Vec<VerifyStep>>` to SpecTask where VerifyStep has `command`, `expected_exit_code`, `check_urls`.

### Step 2: `verify_wave` command (Tauri command)
New Tauri command that takes a merged worktree path and runs:
1. `npm install` → check exit code
2. `npm run build` → check exit code + output files exist
3. (Optional) Start server on random port → HTTP smoke tests → kill server
4. Returns `VerificationReport { passed: bool, checks: Vec<CheckResult> }`

### Step 3: `generate_deploy_config` command
Scans project for framework (Vite/React → vercel.json, Node/Express → Dockerfile checks)
and generates missing deployment configs.

### Step 4: Wave 5 in orchestration spec
The orchestration pipeline should include a Wave 5 (Deploy & Verify) by default after all
build waves complete.

## TDD Plan

1. **RED**: Write failing tests for `verify_wave` — expects it to detect missing vercel.json
2. **GREEN**: Implement `verification.rs` with build check + SPA routing check
3. **REFACTOR**: Wire into `finalize_wave` as optional post-verification step
4. **RED**: Write test for `generate_deploy_config` detecting Vite project
5. **GREEN**: Implement deploy config generation
6. **INTEGRATION**: Wire new commands into lib.rs invoke_handler, verify 0 regressions

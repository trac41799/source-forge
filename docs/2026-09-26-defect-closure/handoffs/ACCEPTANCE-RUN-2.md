# Acceptance Run 2 — Full real pipeline (spec → agents → verify → deploy)

**Date:** 2026-09-28
**Branch:** `wave/A/unblock` · worktree `.worktrees/wave-A`
**Command:**

```powershell
$env:ACC_AGENT_MODEL="opencode-go/gpt-5.6-luna"; $env:ACC_AGENT_TIMEOUT="240"
cargo test -p sourceforge --lib -- --ignored --nocapture real_pipeline
```

The run drives `pipeline::run_pipeline` with the **production** `AdapterWaveRunner`
(worktrees → `opencode` adapter → supervisor) against a git fixture repo, a temp
SQLite DB, real migrations, and a mock deployer. The test lives at
`src-tauri/src/real_pipeline_test.rs` and is `#[ignore]`d so CI stays hermetic.

## Result — the pipeline itself passes

```
status : succeeded
  parse_spec   done  1 tasks parsed
  resolve_stack done stack=nextjs-prisma-vercel
  provision    done  all CLIs + MCPs available
  scaffold     skipped (package.json present)
  seed_plan    done  1 agents
  execute_waves done 1 agents spawned
  finalize_and_verify done verification passed=true
  deploy       done  deployed via mock → https://mock-vercel.example.app (+ Dockerfile artifact)
  report       done
artifacts: dist/index.html=true Dockerfile=true
```

Worktrees are created **inside the repo** with the full checkout
(`.acc/GUIDELINE.md`, `package.json`, `src/`, `dist/`, `vercel.json`, …), the
agent is spawned, the supervisor polls for the handoff with retry, verification
runs 18 checks, and the deploy stage produces the Dockerfile artifact.

## Defects this run surfaced and fixed

| # | Defect | Fix |
|---|---|---|
| 1 | **Worktree path resolved against two different bases.** `git -C <repo> worktree add .worktrees/…` created the worktree **inside the repo**, while the guideline writer and the agent CWD used the *process* CWD (`src-tauri/`). The agent therefore ran in a stray directory containing only `.acc/GUIDELINE.md` — no code, no repo. (This was the "H2 deferred" item; it is a hard blocker, not cosmetic.) | `wave_executor::execute_wave_with_adapters` now builds an absolute path `<base_repo>/.worktrees/<plan>-<ref>` |
| 2 | **`kill()` / process registration panicked** — `tokio::spawn` with "there is no reactor running": the supervisor kills/respawns agents from the pipeline's *synchronous* stage loop, where no Tokio runtime is in scope. Retry and cancel were unusable (the whole run aborted). | `opencode.rs` uses `tauri::async_runtime::spawn` (global runtime, lazily initialised) at all four sites |
| 3 | **The agent was never asked for a handoff.** Spawn passed only `agent.task` (a one-line step description); the guideline — which carries the task, conventions and the `HANDOFF_<ref>.md` filename — was written to disk but never reached the prompt. Attempt 1 could never satisfy the supervisor. | The guideline content is now the spawn prompt (matching what the H4 retry already did) |
| 4 | **The guideline omitted a required handoff section.** `validate_handoff_schema` demands 6 sections, but `generate_agent_guideline` listed only 5 — `Interface Contracts Exposed` was missing, so a *compliant* agent could never pass validation. | `orchestrator::REQUIRED_HANDOFF_SECTIONS` is now the single source of truth, used by both the guideline text and the validator |

## Open items (honest)

- **Agent completion is nondeterministic within the budget.** With a 240 s
  timeout the agent *did* the work — `src/greeting.ts` + `src/greeting.test.ts`
  were present in the worktree — but ran out of time before writing the
  handoff (it spent the budget on `npm install`). With 600 s it produced
  nothing at all on that attempt. So `wave.agents[0].status == "failed"` while
  the run still reports `succeeded`.
- **Run status is driven by verification, not by agent completion.** Here
  verification passes because the fixture already satisfies every check, so the
  pipeline reports success even though the agent wrote no handoff. That is
  worth a product decision (should a wave with zero completed agents be able to
  produce a `succeeded` run?), and it is why the acceptance test asserts
  handoff completion separately.
- **Deploy used a mock deployer**; `vercel deploy --prod` was not executed
  (`ACC_REAL_DEPLOY=1` enables it). Supabase provisioning still uses the
  DB-row MCP proxy.
- The guideline tells the agent "all existing tests must pass / write tests",
  which on a fixture with no test tooling invites a long install-and-test loop —
  a plausible cause of the timeout above.

## Gates

Rust **183 lib + 42 integration**; the real run is `ignored` so the hermetic
suite stays green. Artifacts from the last run are preserved at
`src-tauri/target/real-run-artifacts/`.

# WAVE-F HANDOFF — Real Acceptance Run (Step 5.6, first attempt)

**Date:** 2026-09-26
**Trigger:** "attempt acceptance run first before we merge/rebase"
**Branch/worktree:** `wave/A/unblock` @ `.worktrees/wave-A`

## Goal

Prove a **real agent CLI** can complete a task and produce a schema-valid
handoff through SourceForge's probe — the prerequisite for the promised
spec → agents → verify → deploy outcome.

## Environment

- `opencode` v2.0.18 (real binary at
  `%APPDATA%\npm\node_modules\@opencode\cli\bin\opencode.exe`)
- `vercel` CLI 54.21.1
- No `OPENROUTER_API_KEY` / `ANTHROPIC_API_KEY`; opencode has its own stored
  credentials (OpenCode Go, DeepSeek, OpenRouter)

## Findings (each one is a real defect this run surfaced)

1. **`opencode` is an npm `.cmd` shim** — `Command::new("opencode")` fails with
   "program not found" on Windows. Must spawn via `cmd /C` or the real `.exe`.
2. **Headless runs block without `--auto`** — opencode waits on a permission
   prompt forever (first probe hung until killed).
3. **The default model does not execute tools** — with
   `perceptron/perceptron-mk1.5` opencode replied *"Both files have been
   created"* while writing **nothing** (hallucinated success). With
   `--model opencode-go/gpt-5.6-luna` it really patched 2 files.
4. **Stray guideline discovery** — with no `.acc/GUIDELINE.md`, the agent
   followed an unrelated `.acc-test/GUIDELINE.md` found above the fixture and
   produced an incomplete handoff. The probe correctly **rejected** it
   (5 of 6 required sections missing) — validation works.

## Result

With `opencode run --auto --model opencode-go/gpt-5.6-luna` and a fixture
guideline, the probe passed:

```
[dogfood] agent exited with exit code: 0
[dogfood] valid handoff: tools/fixtures/fixture-repo\HANDOFF_probe.md
PROBE_RC= 0
```

## Fix applied

`src-tauri/src/agent_adapters/opencode.rs` — `spawn` now:
- launches through `cmd /C` on Windows (npm shim resolution),
- passes `--auto` unconditionally,
- passes `--model` from `ACC_AGENT_MODEL` when set (operator-pinned
  tool-capable model).

Fixture: `tools/fixtures/fixture-repo/.acc/GUIDELINE.md` (explicit task + the
exact 6 required handoff headers); the nested fixture repo's `.git/` is
gitignored.

## Not yet done

- **Full real pipeline run** (`build_app` with the real `AdapterWaveRunner`:
  worktrees → adapter spawn → supervision → verify → deploy). The adapter is
  now unblocked, but this end-to-end run has **not** been executed yet.
- **Deploy** (`vercel --prod`) was never executed; `VercelDeployer` remains
  unproven.
- **Supabase provisioning** still pauses as `awaiting_user` (MCP detection is a
  DB-row proxy).

## Verification Evidence

| Check | Command | Result |
|---|---|---|
| Stub probe (deterministic) | `dogfood --agent "powershell -File stub-agent.ps1"` | exit 0 |
| **Real probe** | `dogfood --agent "<opencode.exe> run --auto --model opencode-go/gpt-5.6-luna"` | **exit 0, valid handoff** |
| Adapter unit tests | `cargo test -p sourceforge --lib agent_adapters` | 12 passed |

## Security note (unrelated to this branch)

`~/.config/opencode/opencode.json` stores plaintext bearer tokens (Supabase
`sbp_…`, Tavily) and MCP URLs. Consider moving them to env vars / a secret
store and rotating any that were shared in logs.

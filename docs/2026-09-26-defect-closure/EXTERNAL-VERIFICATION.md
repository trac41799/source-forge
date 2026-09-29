# External verification requirements (what cannot be cleared locally)

**Date:** 2026-09-29
**Status:** everything verifiable on this machine is closed and merged. This file
is the handover for the two remaining proofs that need external access
(production infrastructure), plus the security policy for supplying credentials.

---

## 1. Real Vercel deploy — ready to run, needs approval

**Why it is open:** the acceptance runs use `MockDeployer`. `VercelDeployer`
(`src-tauri/src/deployer.rs`) has never executed against the real CLI, so the
"deploy to production" half of the product promise is unproven end to end.

**Local readiness (verified):**
- `vercel` CLI **54.21.1** installed and **authenticated locally as `trac41799`**.
- The repo is linked to a Vercel project (CI publishes a preview per PR).
- The deployer runs `vercel deploy --prod --yes` in the project directory and
  returns the URL; non-zero exit fails the stage.

**What is needed from the operator:**
1. Approval to deploy the throwaway fixture to the Vercel account — ideally a
   dedicated project/scope so nothing user-facing is touched (CI uses
   `radiance-s-projects1`).
2. Nothing else: no keys need to be pasted anywhere.

**How it will be verified (exact steps):**
```powershell
# in .worktrees/wave-A/src-tauri
$env:ACC_AGENT_MODEL="opencode-go/gpt-5.6-luna"
$env:ACC_AGENT_TIMEOUT="420"
$env:ACC_REAL_DEPLOY="1"                     # switches MockDeployer -> VercelDeployer
cargo test -p sourceforge --lib -- --ignored --nocapture real_pipeline
```
**Acceptance criteria:**
- `deploy: Some(Some("https://<project>.vercel.app"))` — a real URL, not the mock.
- An HTTP fetch of that URL returns 200 and serves the fixture's `index.html`.
- A deliberately failing deploy (e.g. bad token) must surface as a failed stage
  and must not report `succeeded`.
**Cost/impact:** a static-site deploy of a throwaway fixture; no traffic.

---

## 2. Supabase provisioning — needs a decision and one implementation round

**Why it is open:** `stage_provision` (`src-tauri/src/pipeline.rs`) only checks
that the CLIs exist and that a `supabase_configs` row exists for the project
(`has_supabase_config`). **Nothing creates a database, applies migrations, or
checks connectivity** — `provisioner.rs` only installs CLIs. The gate is a proxy.

**What "real provisioning" requires (code, ~1 round):**
1. Resolve the target project ref (existing project, or create one via the
   Supabase Management API / MCP).
2. Apply the project's migrations (e.g. `src-tauri/migrations/*.sql` for
   SourceForge's own schema, or the scaffolded project's schema) through the
   Supabase API/MCP, idempotently.
3. Verify: connectivity + expected tables present (Supabase MCP
   `list_tables` is a natural check), then record the real project ref in
   `supabase_configs`.
4. Failure → `awaiting_user` with an actionable message (never `succeeded`).

**What is needed from the operator (one of):**
- **Existing project path:** connect a Supabase project in the app's
  Integrations UI (that is what writes `supabase_configs`), and tell me the
  project ref to target. No secret leaves your machine.
- **Dedicated test projects path:** a Supabase **access token** supplied via
  environment variable (`SUPABASE_ACCESS_TOKEN`) so I can create/tear down
  throwaway projects. Also note the `supabase` CLI is **not installed** on this
  machine (`npm i -g supabase` installs it; the app can do this via
  `provisioner::install_cli`).

**How it will be verified:**
- A spec-driven run targeting the Supabase stack reaches `verification passed`
  with the provision stage reporting the real project ref (not a proxy).
- `list_tables` on that project shows the migrated schema.
- Re-running the same spec is idempotent (no duplicate objects, no failure).
- With the project unavailable, the run pauses as `awaiting_user` with a clear
  reason.

---

## 3. Security policy for supplying credentials

- Keys go **only** into environment variables or the app's Integrations UI.
- **Never** paste tokens into chat/PRs. If a token is ever exposed, rotate it.
- Known pre-existing exposure to rotate when convenient:
  `~/.config/opencode/opencode.json` contains plaintext bearer tokens
  (Supabase `sbp_…`, Tavily) on this machine (outside the repository).
- Verified non-exposure in the repo: `git diff` scans for `sk-`, `ghp_`,
  `sbp_`, `tvly-`, and private-key headers found nothing across the merged PRs.

---

## 4. Locally cleared in this round (for reference)

| Item | Evidence |
|---|---|
| Cost cap (M9) was inert | Adapter now captures `--format json` usage and reports it; the supervisor refreshes `agent.cost_usd` each poll. Real run captured **$0.0055 / $0.0056 per agent**. Unit tests: `test_parse_cost_from_json_lines`, `test_cost_cap_triggers_from_adapter_telemetry`. |
| Injection residue (H1) | Task text is written to `.acc/TASK.md`; only a constant pointer is passed as an argument (Windows `cmd /C` expands `%VAR%` even inside quotes). Test: `test_task_text_never_reaches_the_command_line`. |
| Ignored `emit` errors (L8) | `TauriEventSink` logs failures instead of dropping them. |
| Multi-agent + real-agent E2E | 2 concurrent agents, valid handoffs, verification passed, deploy stage — 45–55 s per run. |

**Remaining non-external item:** L2 (unused IPC constants) — cosmetic and already
tracked by the IPC contract snapshot (`ipc-unused.snapshot.json`); deliberately
not churned.

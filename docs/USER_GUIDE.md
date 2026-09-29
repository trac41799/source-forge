# SourceForge User Guide

SourceForge is a local-first desktop cockpit for AI coding agents. It runs
multiple agents in parallel PTY sessions, orchestrates dependency-aware waves,
verifies output, and compounds what it learns.

## Install

- Prerequisites: Node.js 18+, Rust (stable), and at least one agent CLI
  (`opencode`, `claude`, `aider`, …).
- Build from source: `npm install` then `npx tauri build`. The binary lands in
  `src-tauri/target/release/`.

## First run

1. Open the app — you land on **Runner**.
2. Open a project (any local directory).
3. Click an agent button (e.g. **OpenCode**) to start a PTY session and type a task.

## Build App (the end-to-end pipeline)

`/build` runs the whole flow as one supervised, resumable run:

```
parse spec → resolve stack → provision → scaffold → seed plan
→ execute waves → finalize + verify → deploy → report
```

1. Enter a **spec file** — markdown with `## Phase N` and `### Step X.Y` headers
   (the same format as `docs/GAP_CLOSURE_TEMPLATE.md`-style plans).
2. Enter the **project path** (new or existing directory).
3. Pick a **stack** (Next.js/Express variants) and the **agent command**.
4. Click **Build App**. Watch the stage timeline; progress streams as
   `build-app-progress` events.
5. If a required CLI (or Supabase MCP) is missing, the run pauses as
   `awaiting_user` with the exact delegation task — install it and press
   **Build App** again to resume (completed stages are skipped).

**Deploy is gated on verification.** If any check fails (build output, SPA
rewrites, env vars, runtime smoke test), deploy is skipped unless you
explicitly allow deploying a failed verification. A `Dockerfile` artifact is
always generated for non-Vercel hosts.

## Knowledge Compounder

After each verified wave the compounder extracts decisions, patterns, and
anti-patterns into the Knowledge page. Run it manually from Knowledge → Run
Compounder. It needs `OPENROUTER_API_KEY` in the environment; without a key the
wave still succeeds and the compounder is skipped.

## Verification checks

`verify_project` runs ~18 checks: package.json scripts, build output,
`dist/index.html`, TypeScript, README/.env.example, git remote, SPA rewrites
excluding `/api`, API client production config, CORS, secrets, and an optional
runtime smoke test. Checks that cannot run without `node_modules` report
**Skip** (not Fail).

## Troubleshooting

| Symptom | Fix |
|---|---|
| "command not found" for a feature | Run `npm run test` — the IPC contract test lists any frontend/backend mismatch |
| Build paused as `awaiting_user` | Install the missing CLI, or connect Supabase MCP under Integrations |
| Deploy skipped | Read the failing verification checks in the build report |
| Agent never finishes | It is killed at the deadline, retried once, then a correction doc is written (Outcomes) |
| Rust build fails on Windows | Ensure MSVC Build Tools; `rust-toolchain.toml` pins the MSVC toolchain |

## Testing & gates

```bash
npx tsc --noEmit          # typecheck
npx vitest run            # 330+ frontend tests (incl. IPC contract)
cargo test --workspace    # Rust lib + integration tests
python -m pytest -q       # in local-daemon/ and webhook-server/
npm run lint              # eslint
```

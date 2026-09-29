# Acceptance Run 5 — from an empty repo to a type-checking, building app

**Date:** 2026-09-29 · branch `hardening/production`
**Command:**

```powershell
$env:ACC_AGENT_MODEL="opencode-go/gpt-5.6-luna"; $env:ACC_POC_TIMEOUT="300"
cargo test -p sourceforge --lib -- --ignored --nocapture test_real_poc_from_empty
```

New test `real_pipeline_test::test_real_poc_from_empty`: the project starts as an
**empty git repo** (a README and a remote, nothing else) and runs the production
path with `install_deps: true` (the hard gate) and `deliver: true`.

## Result — the delivery chain works, the last check fails closed

```
scaffold  done  scaffolded 15 files          npm install + prisma generate  ok
tsc       Pass  (real typecheck)
build     Pass  (real `next build`)
output    Pass  (.next/BUILD_ID)
delivery  merged: [1-1.1] conflicts: []   app/about/page.tsx in base project: true
verification passed=false  →  deploy skipped (fail-closed)
only failing check: E2E runtime test — "Server failed to boot within 60 seconds"
```

So: **empty repo → scaffolded app → real install → real typecheck → real build →
agent's feature merged into the project**. The run correctly refuses to deploy
while one check fails.

## Six defects this run found and fixed

| # | Defect | Fix |
|---|---|---|
| 1 | **Scaffold emitted invalid TypeScript**: `{{`/`}}` were written literally in three non-`format!` strings (`app/api/health/route.ts`, `lib/prisma.ts`, `lib/supabase.ts`) — every scaffolded app failed `tsc` and `next build` | single braces; regression test scans **every** file in `files_created` for doubled braces |
| 2 | **Prisma schema had no models** → `prisma generate` exits 1 → `npm install` failed | scaffold ships a `User` model; `postinstall: prisma generate`; test asserts a model exists |
| 3 | **Scaffold lacked `typecheck`/`test` scripts and the env vars the checks require** | added both scripts, a real `node:test` file, and JWT_SECRET/PORT/CLIENT_URL to `.env.example` |
| 4 | **`npm`/`npx` invoked directly** — same `.cmd`-shim bug as the agent adapter, so the build/typecheck checks always reported "program not found" on Windows | `npm_command()` routes through `cmd /C` |
| 5 | **Build artifact check ran before the build** → could never pass on a fresh project | build runs first, then artifact checks; `.next/BUILD_ID` accepted for Next.js |
| 6 | **Undiagnosable failures**: `tsc` counted errors on the *wrong stream* (reported "0 errors" while failing), build failures discarded their output, server output went to an undrained pipe | diagnostics capture the real errors (this is what exposed defects 1 and 2); server output goes to a log file with its tail in the failure message |

Plus: `install_project_dependencies` bootstraps `.env` from `.env.example` (Prisma's
postinstall resolves the datasource URL during install).

## Still open

- **`E2E runtime test` fails**: the dev server did not answer within 60 s. It is no
  longer a mystery box — the boot failure now carries the server log tail, so the
  next run will say why (Next dev cold start, port, missing env…). Until then the
  run stays `verification_failed` and deploy stays skipped — correct behaviour.
- The runtime check's six auth-flow probes only apply to apps with auth routes;
  apps without them are now judged on serving traffic (a template contract was
  being imposed on every app).
- GitHub delivery (private repo, issues, PRs) — unchanged, see `EXTERNAL-VERIFICATION.md`.

## Gates

Rust **218 lib + 42 integration** (2 real-agent tests `ignored`), Web 343, `tsc`/`lint` clean.

# Local-first database: containerized Postgres on this machine

**Date:** 2026-09-30 · branch `hardening/production`
**Status:** implemented + unit-tested; live container boot pending Docker Desktop
(the engine was still starting at ship time).

## Design

Postgres-backed stacks no longer hard-require a Supabase cloud project up front.
When a stack needs `supabase` MCP and no cloud config exists, the pipeline
provisions a **local Docker Postgres** instead of pausing:

```
spec → scaffold (app + prisma schema + docker-compose.yml)
  → provision (docker compose up -d db, wait healthy, record target)
  → agents → merge → install (npm ci, .env bootstrapped)
  → prisma schema applied (migrate deploy | db push)
  → verify (reachability + tables, through the project's own Prisma toolchain)
  → deploy
```

**Portability is structural, not promised:** Prisma is the migration layer. The
same `prisma/schema.prisma` applies anywhere by changing only `DATABASE_URL`
(local container → Supabase cloud → Neon → RDS); data moves with
`pg_dump`/`pg_restore` because Supabase *is* Postgres. The compose file and the
schema are the portable artifacts.

## What changed

| Area | Change |
|---|---|
| `migrations/018_database_targets.sql` (+ `db.rs` registration) | `database_targets` table: provider, URL, container, compose file per project |
| `database.rs` (new) | compose rendering (pinned `postgres:16-alpine`, port 54322, healthcheck, volume), slug, daemon detection, `ensure_local_postgres` (idempotent, resume-safe), `apply_prisma_schema` (`migrate deploy` vs `db push`), `check_database` (reachability + per-model `to_regclass` via the project's Prisma) |
| `pipeline.rs` | provision falls back to local-docker (cloud config still wins); daemon-down pauses as `awaiting_user` with "start Docker Desktop"; schema apply runs after install when `install_deps` |
| `verification.rs` | new `database reachable + schema applied` check (Skips cleanly without a Prisma schema) |
| `arch_engine.rs` | scaffold emits `docker-compose.yml`; `.env.example` defaults `DATABASE_URL` to the local container, cloud URL kept as a commented template |
| `lib.rs` | `mod database` |

## Verified

- 7 new unit tests (compose content, slug, daemon detection, model parsing, SQL
  builder, target roundtrip/upsert) + existing suites green (lib 235+, integration 42).
- A self-skipping live test (`test_live_local_postgres_roundtrip`, `#[ignore]`d)
  boots a real container, runs a `psql` roundtrip, and tears it down — it runs
  automatically once Docker is up.

## Verified (SQLite ladder, 2026-09-30)

- New `nextjs-sqlite-vercel` preset: file database, no daemon, no cloud account.
- From-empty POC with that stack is **green**: provision records the sqlite
  target, scaffold (now committed, so agents see the app), agent delivers,
  `prisma db push` creates `dev.db`, and verification passes including
  `database reachable + schema applied` (per-model `COUNT` probes, since
  `db execute` prints no rows on SQLite) and the E2E runtime check.
- Delivery hardening from the same runs: `HANDOFF_*.md` excluded from delivery
  commits; scaffold output committed so worktrees contain the app; a post-merge
  guard turns silently-lost files into a loud conflict.

## Not yet verified (needs Docker Desktop running)

```powershell
# 1. wait until this prints a version:
docker info --format "{{.ServerVersion}}"
# 2. live container roundtrip:
cargo test -p sourceforge --lib -- --ignored database::tests::test_live_local_postgres_roundtrip --nocapture
# 3. full E2E with a DB-backed stack (uses the local container for provision):
$env:ACC_AGENT_MODEL="opencode-go/gpt-5.6-luna"
#    (set stack to nextjs-supabase-vercel in a scratch run)
```

## Follow-ups (documented, not started)

- A `get_database_target` Tauri command + UI surface (Integrations page) showing
  provider/URL/container state per project.
- Express stack has no Prisma schema yet, so the DB check skips for it.
- An app→DB health ping (e.g. `/api/db-health` doing `SELECT 1`) would prove the
  app tier, not just the CLI toolchain, reaches the database.
- `POSTGRES_PASSWORD` is read at scaffold and provision time; changing it between
  the two needs a compose-file refresh (compose file is authoritative).

-- 018: local-first database targets (Docker Postgres on this machine).
--
-- SourceForge can provision a containerized Postgres for postgres-backed
-- stacks instead of requiring a Supabase cloud project up front. The same
-- Prisma schema then migrates anywhere later (Supabase cloud, Neon, RDS):
-- the compose file plus `prisma/migrations` (or `db push`) are the portable
-- artifacts, and only DATABASE_URL changes.

CREATE TABLE IF NOT EXISTS database_targets (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL UNIQUE REFERENCES projects(id),
    provider        TEXT NOT NULL,              -- 'local-docker' | 'supabase-cloud' | ...
    database_url    TEXT NOT NULL,              -- URL the app + tooling use
    container_name  TEXT,                       -- local-docker: compose service container
    compose_file    TEXT,                       -- local-docker: path to the compose file
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_database_targets_project ON database_targets(project_id);

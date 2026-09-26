-- 017_build_runs.sql
-- Supervised Build Pipeline state (SPEC-001 §5 DG-1).
-- One row per build_app run; resumable and cancellable.

CREATE TABLE IF NOT EXISTS build_runs (
  id             TEXT PRIMARY KEY,
  project_id     TEXT,
  spec_path      TEXT NOT NULL,
  project_path   TEXT NOT NULL,
  stack_id       TEXT,
  status         TEXT NOT NULL DEFAULT 'pending',
  current_stage  TEXT,
  stage_log      TEXT NOT NULL DEFAULT '[]',
  report         TEXT,
  error          TEXT,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_build_runs_project ON build_runs(project_id);
CREATE INDEX IF NOT EXISTS idx_build_runs_status ON build_runs(status);

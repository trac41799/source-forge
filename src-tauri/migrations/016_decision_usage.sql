-- 016_decision_usage.sql
-- Decision layer: usage audit, review queue, and backend configuration.
-- Part of spec 001-decision-layer (M0 foundation).

CREATE TABLE IF NOT EXISTS decision_usage (
  id TEXT PRIMARY KEY,
  backend_id TEXT NOT NULL,
  model TEXT,
  primitives TEXT,
  answers TEXT,
  confidence REAL,
  policy_outcome TEXT,
  latency_ms INTEGER,
  input_tokens INTEGER,
  cost REAL,
  truncated INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_decision_usage_created
  ON decision_usage (created_at DESC);

CREATE TABLE IF NOT EXISTS decision_reviews (
  id TEXT PRIMARY KEY,
  consumer TEXT NOT NULL,
  question TEXT,
  decided_value TEXT,
  confidence REAL,
  payload TEXT,
  resolved INTEGER NOT NULL DEFAULT 0,
  resolution TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_decision_reviews_open
  ON decision_reviews (resolved, created_at DESC);

CREATE TABLE IF NOT EXISTS decision_config (
  id TEXT PRIMARY KEY DEFAULT 'default',
  backend TEXT NOT NULL DEFAULT 'hosted',
  base_url TEXT NOT NULL DEFAULT 'https://openrouter.ai/api',
  model TEXT NOT NULL DEFAULT 'typesafe/jev-1.13',
  accept_threshold REAL NOT NULL DEFAULT 0.75,
  review_threshold REAL NOT NULL DEFAULT 0.40,
  context_limit INTEGER NOT NULL DEFAULT 32000,
  timeout_ms INTEGER NOT NULL DEFAULT 5000,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO decision_config (id) VALUES ('default');

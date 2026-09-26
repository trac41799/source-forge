-- 015_user_preferences.sql
-- Stores user preferences for stack, deploy target, and provisioning settings.

CREATE TABLE IF NOT EXISTS user_preferences (
  id TEXT PRIMARY KEY DEFAULT 'default',
  preferred_stack TEXT NOT NULL DEFAULT 'nextjs-supabase-vercel',
  default_deploy_target TEXT NOT NULL DEFAULT 'vercel',
  auto_provision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO user_preferences (id) VALUES ('default');

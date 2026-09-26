-- 011_memory.sql
-- Memory layer: facts + session checkpoints.
--
-- NOTE: the vec0 virtual table below requires the sqlite-vec extension, which
-- is NOT currently loaded by db.rs. It is intentionally the LAST statement so
-- that execute_batch (which stops at the first error) still creates the
-- relational tables and indexes above it. db.rs treats this migration as
-- non-fatal; the vec0 failure is expected until sqlite-vec is wired in.

CREATE TABLE IF NOT EXISTS memory_facts (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    user_id       TEXT NOT NULL,
    org_id        TEXT NOT NULL,
    fact_type     TEXT NOT NULL,
    content       TEXT NOT NULL,
    embedding     BLOB,
    metadata      TEXT,
    confidence    REAL DEFAULT 0.5,
    access_count  INTEGER DEFAULT 0,
    last_accessed TEXT,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS session_checkpoints (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    turn_number   INTEGER NOT NULL,
    state_blob    BLOB NOT NULL,
    summary       TEXT,
    token_count   INTEGER,
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_agent ON memory_facts(agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memory_facts(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_org ON memory_facts(org_id);
CREATE INDEX IF NOT EXISTS idx_memories_type ON memory_facts(fact_type);
CREATE INDEX IF NOT EXISTS idx_checkpoints_session_turn ON session_checkpoints(session_id, turn_number);

-- Requires sqlite-vec; expected to fail (non-fatally) until the extension is loaded.
CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
    embedding float[384]
);

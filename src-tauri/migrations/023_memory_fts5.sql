CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_fts USING fts5(
    id UNINDEXED,
    content,
    metadata,
    fact_type,
    agent_id UNINDEXED,
    org_id UNINDEXED
);

INSERT INTO memory_facts_fts (id, content, metadata, fact_type, agent_id, org_id)
SELECT id, content, COALESCE(metadata, ''), fact_type, agent_id, org_id FROM memory_facts
WHERE id NOT IN (SELECT id FROM memory_facts_fts);

CREATE TRIGGER IF NOT EXISTS memory_facts_ai AFTER INSERT ON memory_facts BEGIN
    INSERT INTO memory_facts_fts (id, content, metadata, fact_type, agent_id, org_id)
    VALUES (new.id, new.content, COALESCE(new.metadata, ''), new.fact_type, new.agent_id, new.org_id);
END;

CREATE TRIGGER IF NOT EXISTS memory_facts_ad AFTER DELETE ON memory_facts BEGIN
    DELETE FROM memory_facts_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS memory_facts_au AFTER UPDATE ON memory_facts BEGIN
    DELETE FROM memory_facts_fts WHERE id = old.id;
    INSERT INTO memory_facts_fts (id, content, metadata, fact_type, agent_id, org_id)
    VALUES (new.id, new.content, COALESCE(new.metadata, ''), new.fact_type, new.agent_id, new.org_id);
END;

-- 020_decision_usage_hash.sql
-- Decision layer: record a stable fingerprint of the decision inputs so runs can
-- be compared without ever persisting the raw state (risk R-12).

ALTER TABLE decision_usage ADD COLUMN input_hash TEXT;

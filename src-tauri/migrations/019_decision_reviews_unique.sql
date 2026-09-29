-- 019_decision_reviews_unique.sql
-- Decision layer: deduplicate the review queue (risk R-15).
-- One review row per (consumer, question, decided_value); re-deciding the same
-- thing must not enqueue duplicates.

CREATE UNIQUE INDEX IF NOT EXISTS idx_decision_reviews_dedup
  ON decision_reviews (consumer, question, decided_value);

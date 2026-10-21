-- Relecture n° 2 de la Phase 8, I-1: the smoke test of `timonier doctor` (ADR 0050) is a run of
-- its own kind, which neither the budgets nor the views take by default.
ALTER TABLE run ADD COLUMN smoke INTEGER NOT NULL DEFAULT 0 CHECK (smoke IN (0, 1));

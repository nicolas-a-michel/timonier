-- Architectural role of packages and components (ADR 0017). Part of a node's version.
ALTER TABLE node ADD COLUMN role TEXT;
ALTER TABLE node_claim ADD COLUMN role TEXT;

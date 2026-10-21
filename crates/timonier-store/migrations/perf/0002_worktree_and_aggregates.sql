-- Relecture n° 1 de la Phase 8.

-- Q77: a run on a modified working tree keeps the fingerprint of its changes; NULL on a clean
-- tree. It never enters the reference of the main branch.
ALTER TABLE run ADD COLUMN worktree TEXT;

-- ADR 0043: what each node cost in a run, kept when retention drops the run's spans, for the
-- reference of the regressions and the evolution by commit.
CREATE TABLE aggregate (
    run INTEGER NOT NULL REFERENCES run (id) ON DELETE CASCADE,
    node TEXT NOT NULL,
    calls INTEGER NOT NULL,
    -- Its spans' time less that of the spans they called (timonier_core::perf::map_of).
    self_ns INTEGER NOT NULL,
    -- The time of its spans no span of a node called (Q76).
    entered_ns INTEGER NOT NULL,
    PRIMARY KEY (run, node)
) STRICT;

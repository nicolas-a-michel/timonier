-- T-125, ADR 0060: Windows measures the wall time only, and runs are compared on it. SQLite
-- cannot change a CHECK in place: the table is rebuilt as its documentation says (foreign keys
-- off for the migrations, checked after them; schema.rs), its columns in the same order.
CREATE TABLE run_new (
    id INTEGER PRIMARY KEY,
    started_at_ms INTEGER NOT NULL,
    command TEXT NOT NULL,
    snapshot INTEGER NOT NULL,
    commit_sha TEXT,
    branch TEXT,
    batch TEXT,
    traced INTEGER NOT NULL DEFAULT 1 CHECK (traced IN (0, 1)),
    finished INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1)),
    finished_at_ms INTEGER,
    spans_dropped INTEGER NOT NULL DEFAULT 0,
    exit_code INTEGER,
    fingerprint TEXT,
    machine TEXT,
    cpu_source TEXT CHECK (cpu_source IN ('instructions', 'cpu_time', 'wall_time')),
    accounting TEXT CHECK (accounting IN ('systemd_scope', 'rusage', 'wall_time')),
    wall_us INTEGER,
    user_cpu_us INTEGER,
    system_cpu_us INTEGER,
    largest_process_bytes INTEGER,
    tree_peak_bytes INTEGER,
    block_reads INTEGER,
    block_writes INTEGER,
    io_read_bytes INTEGER,
    io_write_bytes INTEGER,
    instructions INTEGER,
    counter_refused TEXT,
    samples TEXT,
    junit TEXT,
    junit_problem TEXT,
    worktree TEXT,
    smoke INTEGER NOT NULL DEFAULT 0 CHECK (smoke IN (0, 1))
) STRICT;

INSERT INTO run_new SELECT * FROM run;
DROP TABLE run;
ALTER TABLE run_new RENAME TO run;

CREATE INDEX run_by_branch ON run (branch, id);
CREATE INDEX run_by_batch ON run (batch, id);

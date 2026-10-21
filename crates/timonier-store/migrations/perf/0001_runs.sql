-- The runs of tests that `timonier test` measured (ADR 0043, 0045, 0047): facts, never
-- rewritten. Its own database (`.timonier/perf/perf.sqlite`): rebuilding the graph leaves it
-- alone.

-- Created at `run/start`; what was measured comes at `run/finish`, and stays NULL for a run
-- never finished (interrupted).
CREATE TABLE run (
    id INTEGER PRIMARY KEY,
    started_at_ms INTEGER NOT NULL,
    -- JSON array of the command's words.
    command TEXT NOT NULL,
    -- The head snapshot of the graph when the run began, and git's view of it.
    snapshot INTEGER NOT NULL,
    commit_sha TEXT,
    branch TEXT,
    -- The runs of one `timonier test --runs N` share it: the batch a regression is judged on
    -- (ADR 0053).
    batch TEXT,
    -- 0: run with OTEL_SDK_DISABLED, to measure the overhead of the traces (ADR 0049).
    traced INTEGER NOT NULL DEFAULT 1 CHECK (traced IN (0, 1)),
    finished INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1)),
    finished_at_ms INTEGER,
    -- Spans the receiver counted beyond the cap of a run (ADR 0043).
    spans_dropped INTEGER NOT NULL DEFAULT 0,
    exit_code INTEGER,
    fingerprint TEXT,
    -- JSON of timonier_core::perf::Machine.
    machine TEXT,
    cpu_source TEXT CHECK (cpu_source IN ('instructions', 'cpu_time')),
    accounting TEXT CHECK (accounting IN ('systemd_scope', 'rusage')),
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
    -- JSON array of timonier_core::perf::Sample.
    samples TEXT,
    junit TEXT,
    junit_problem TEXT
) STRICT;

CREATE INDEX run_by_branch ON run (branch, id);
CREATE INDEX run_by_batch ON run (batch, id);

CREATE TABLE test (
    run INTEGER NOT NULL REFERENCES run (id) ON DELETE CASCADE,
    -- Its place in the JUnit XML.
    ord INTEGER NOT NULL,
    name TEXT NOT NULL,
    classname TEXT,
    file TEXT,
    line INTEGER,
    status TEXT NOT NULL CHECK (status IN ('passed', 'failed', 'error', 'skipped')),
    duration_us INTEGER NOT NULL,
    started_at_us INTEGER,
    PRIMARY KEY (run, ord)
) STRICT;

-- The spans of a run: the root of each test (synthesized, ADR 0047), then those the receiver
-- took (ADR 0048). Times in nanoseconds since the Unix epoch; a start unknown is NULL.
CREATE TABLE span (
    run INTEGER NOT NULL REFERENCES run (id) ON DELETE CASCADE,
    trace_id TEXT NOT NULL,
    span_id TEXT NOT NULL,
    parent_span_id TEXT,
    -- The test it belongs to (`test.ord`), once known; and how the span named it.
    test INTEGER,
    test_name TEXT,
    test_classname TEXT,
    name TEXT NOT NULL,
    start_ns INTEGER,
    duration_ns INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('unset', 'ok', 'error')),
    synthesized INTEGER NOT NULL CHECK (synthesized IN (0, 1)),
    code_function TEXT,
    code_function_derived INTEGER NOT NULL DEFAULT 0 CHECK (code_function_derived IN (0, 1)),
    code_file TEXT,
    code_line INTEGER,
    -- The node of the graph it is attached to, and the rule that found it.
    node TEXT,
    attached_by TEXT CHECK (attached_by IN ('id', 'name', 'line', 'file')),
    -- JSON object of the other attributes, bounded.
    attributes TEXT NOT NULL,
    PRIMARY KEY (run, trace_id, span_id)
) STRICT;

CREATE INDEX span_by_test ON span (run, test);
CREATE INDEX span_by_node ON span (node, run);

-- The points of the metrics of a run (ADR 0048).
CREATE TABLE metric (
    run INTEGER NOT NULL REFERENCES run (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('sum', 'gauge', 'histogram')),
    time_ns INTEGER NOT NULL,
    value REAL,
    count INTEGER,
    sum REAL,
    attributes TEXT NOT NULL
) STRICT;

CREATE INDEX metric_by_run ON metric (run, name);

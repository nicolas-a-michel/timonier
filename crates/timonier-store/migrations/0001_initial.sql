-- Timonier graph store, initial schema (ADR 0009, 0010, 0011).

-- Snapshots. Exactly one is open (frozen = 0): the working-tree head every change is recorded in.
CREATE TABLE snapshot (
    seq          INTEGER PRIMARY KEY,
    kind         TEXT    NOT NULL CHECK (kind IN ('commit', 'review_mark', 'working_tree')),
    commit_sha   TEXT,
    timestamp_ms INTEGER NOT NULL,
    frozen       INTEGER NOT NULL CHECK (frozen IN (0, 1))
) STRICT;

CREATE UNIQUE INDEX snapshot_single_head ON snapshot (frozen) WHERE frozen = 0;

INSERT INTO snapshot (seq, kind, commit_sha, timestamp_ms, frozen)
VALUES (1, 'working_tree', NULL, CAST(strftime('%s', 'now') AS INTEGER) * 1000, 0);

-- Versions of nodes. A row is valid at snapshot s when valid_from <= s < valid_to
-- (valid_to NULL: still valid). The span is not versioned: it is updated in place.
CREATE TABLE node (
    version_id   INTEGER PRIMARY KEY,
    id           TEXT    NOT NULL,
    scope        TEXT    NOT NULL,
    kind         TEXT    NOT NULL,
    name         TEXT    NOT NULL,
    language     TEXT,
    parent       TEXT,
    signature    TEXT,
    body_hash    BLOB,
    start_line   INTEGER,
    start_column INTEGER,
    end_line     INTEGER,
    end_column   INTEGER,
    valid_from   INTEGER NOT NULL REFERENCES snapshot (seq),
    valid_to     INTEGER REFERENCES snapshot (seq),
    CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

CREATE UNIQUE INDEX node_current_id ON node (id) WHERE valid_to IS NULL;
CREATE INDEX node_current_scope ON node (scope) WHERE valid_to IS NULL;
CREATE INDEX node_current_parent ON node (parent) WHERE valid_to IS NULL;
CREATE INDEX node_history ON node (id, valid_from);
CREATE INDEX node_valid_from ON node (valid_from);
CREATE INDEX node_valid_to ON node (valid_to) WHERE valid_to IS NOT NULL;

-- Versions of merged edges, one per (src, dst, kind) at any snapshot. Endpoints may be absent
-- (dangling, ADR 0009).
CREATE TABLE edge (
    version_id INTEGER PRIMARY KEY,
    src        TEXT    NOT NULL,
    dst        TEXT    NOT NULL,
    kind       TEXT    NOT NULL,
    scope      TEXT    NOT NULL,
    provenance TEXT    NOT NULL,
    confidence REAL    NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    weight     INTEGER NOT NULL CHECK (weight >= 0),
    valid_from INTEGER NOT NULL REFERENCES snapshot (seq),
    valid_to   INTEGER REFERENCES snapshot (seq),
    CHECK (valid_to IS NULL OR valid_to > valid_from)
) STRICT;

CREATE UNIQUE INDEX edge_current_key ON edge (src, dst, kind) WHERE valid_to IS NULL;
CREATE INDEX edge_current_scope ON edge (scope) WHERE valid_to IS NULL;
CREATE INDEX edge_valid_from ON edge (valid_from);
CREATE INDEX edge_valid_to ON edge (valid_to) WHERE valid_to IS NOT NULL;

-- Current state of each scope: the content hash its observations were computed on.
CREATE TABLE scope (
    key          TEXT PRIMARY KEY,
    content_hash BLOB
) STRICT, WITHOUT ROWID;

-- Latest observations of each source in each scope (not versioned: history lives in `edge`).
CREATE TABLE observation (
    scope      TEXT    NOT NULL,
    source     TEXT    NOT NULL,
    src        TEXT    NOT NULL,
    dst        TEXT    NOT NULL,
    kind       TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    weight     INTEGER NOT NULL,
    PRIMARY KEY (scope, source, src, dst, kind)
) STRICT, WITHOUT ROWID;

-- Edge kinds a source analysed completely in a scope, at the scope's current content hash:
-- « verified by LSP at hash X » (spec).
CREATE TABLE verification (
    scope  TEXT NOT NULL,
    source TEXT NOT NULL,
    kind   TEXT NOT NULL,
    PRIMARY KEY (scope, source, kind)
) STRICT, WITHOUT ROWID;

-- Contributions waiting for ownership (ADR 0009). When several scopes provide the same node or
-- edge, the scope with the smallest key owns it (its row above); the others' versions wait here
-- and the next one takes over when the owner lets go. The head thus depends only on what every
-- scope currently provides, never on the order of the updates.
CREATE TABLE node_claim (
    scope        TEXT NOT NULL,
    id           TEXT NOT NULL,
    kind         TEXT NOT NULL,
    name         TEXT NOT NULL,
    language     TEXT,
    parent       TEXT,
    signature    TEXT,
    body_hash    BLOB,
    start_line   INTEGER,
    start_column INTEGER,
    end_line     INTEGER,
    end_column   INTEGER,
    PRIMARY KEY (id, scope)
) STRICT, WITHOUT ROWID;

CREATE INDEX node_claim_scope ON node_claim (scope);
CREATE INDEX node_claim_parent ON node_claim (parent);

CREATE TABLE edge_claim (
    scope      TEXT NOT NULL,
    src        TEXT NOT NULL,
    dst        TEXT NOT NULL,
    kind       TEXT NOT NULL,
    provenance TEXT NOT NULL,
    confidence REAL NOT NULL,
    weight     INTEGER NOT NULL,
    PRIMARY KEY (src, dst, kind, scope)
) STRICT, WITHOUT ROWID;

CREATE INDEX edge_claim_scope ON edge_claim (scope);

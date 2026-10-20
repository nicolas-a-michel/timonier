-- The content of every file when the map was last marked as reviewed (ADR 0024): the review
-- diff counts a dependency only when a file that carries it changed since (Q28). Not graph
-- facts, so no validity interval; only the last review mark keeps its rows.
CREATE TABLE snapshot_content (
    seq          INTEGER NOT NULL REFERENCES snapshot (seq),
    scope        TEXT    NOT NULL,
    content_hash BLOB    NOT NULL CHECK (length(content_hash) = 32),
    PRIMARY KEY (seq, scope)
) STRICT, WITHOUT ROWID;

-- What the LSP refinement still owes when the daemon stops (ADR 0022): the verified files a
-- changed signature made stale, and the lookups of dependents not answered yet. The rest of the
-- queue (files displayed, opened) comes back with the clients. What changes is written at each
-- change, with the batch that made it.
CREATE TABLE refinement_dependent (
    path TEXT PRIMARY KEY
) STRICT, WITHOUT ROWID;

CREATE TABLE refinement_lookup (
    path         TEXT    NOT NULL,
    symbol       TEXT    NOT NULL,
    line         INTEGER NOT NULL,
    col          INTEGER NOT NULL,
    content_hash BLOB    NOT NULL CHECK (length(content_hash) = 32),
    PRIMARY KEY (path, symbol)
) STRICT, WITHOUT ROWID;

-- The files citing a looked-up symbol's name: its dependents when the server cannot say.
CREATE TABLE refinement_lookup_citer (
    path   TEXT NOT NULL,
    symbol TEXT NOT NULL,
    citer  TEXT NOT NULL,
    PRIMARY KEY (path, symbol, citer),
    FOREIGN KEY (path, symbol) REFERENCES refinement_lookup (path, symbol) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

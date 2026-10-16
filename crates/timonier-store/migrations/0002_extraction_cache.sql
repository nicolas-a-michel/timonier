-- Extraction cache (ADR 0013): what tree-sitter found in a file, so that a second indexing of
-- an unchanged repository parses nothing. It is not a graph fact (no valid_from / valid_to):
-- an entry is only valid for the content hash, language, pack version and extractor version
-- it was computed with. `facts` is the indexer's JSON.
CREATE TABLE extraction (
    path TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    extractor INTEGER NOT NULL CHECK (extractor >= 0),
    content_hash BLOB NOT NULL CHECK (length(content_hash) = 32),
    facts TEXT NOT NULL
) STRICT;

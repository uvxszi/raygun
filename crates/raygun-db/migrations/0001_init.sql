-- Device-local bookkeeping. Never synced.
CREATE TABLE local_meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

-- User settings, one row per key so devices can merge them field by field (ARCHITECTURE §10).
-- Sync-ready columns: updated_at_hlc orders writes across devices, origin_device names the writer,
-- deleted is a tombstone (rows are never hard-deleted).
CREATE TABLE setting (
    key            TEXT PRIMARY KEY NOT NULL,
    value          TEXT NOT NULL,              -- JSON
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

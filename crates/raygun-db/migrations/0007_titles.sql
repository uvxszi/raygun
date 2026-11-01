-- Episodic library (ARCHITECTURE §5, M24): titles made of ordered units, starting with manga and
-- chapters. Column prefixes follow Mihon: `remote_*` comes from the source and is overwritten on
-- refresh, `user_*` is the user's and never touched by refresh, `state_*` is app bookkeeping.
-- Synced shape (UUIDv7, HLC, tombstones); not yet in the sync registry.

CREATE TABLE title (
    id                 TEXT PRIMARY KEY NOT NULL,
    kind               TEXT NOT NULL,               -- manga
    source_id          TEXT NOT NULL,               -- <extension>/<provider>
    remote_id          TEXT NOT NULL,               -- the source's id for the title
    remote_title       TEXT NOT NULL,
    remote_cover       TEXT,
    remote_authors     TEXT NOT NULL DEFAULT '[]',  -- JSON array
    remote_artists     TEXT NOT NULL DEFAULT '[]',
    remote_description TEXT,
    remote_genres      TEXT NOT NULL DEFAULT '[]',
    remote_status      TEXT NOT NULL DEFAULT 'unknown',
    remote_url         TEXT,
    user_in_library_at INTEGER,                     -- unix ms; NULL: seen, not in the library
    state_refreshed_at INTEGER,
    updated_at_hlc     INTEGER NOT NULL,
    origin_device      TEXT NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX title_source ON title (source_id, remote_id) WHERE deleted = 0;
CREATE INDEX title_library ON title (kind, user_in_library_at);

CREATE TABLE unit (
    id               TEXT PRIMARY KEY NOT NULL,
    title_id         TEXT NOT NULL,
    kind             TEXT NOT NULL,                 -- chapter
    remote_id        TEXT NOT NULL,
    remote_name      TEXT NOT NULL,
    number           REAL,
    volume           TEXT,
    remote_scanlator TEXT,
    remote_date      INTEGER,
    source_order     INTEGER NOT NULL,              -- position in the source's list (0 = first)
    updated_at_hlc   INTEGER NOT NULL,
    origin_device    TEXT NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX unit_remote ON unit (title_id, remote_id) WHERE deleted = 0;

-- How far a unit was read: page index for manga. Merges as max progress once synced (§10).
CREATE TABLE unit_progress (
    unit_id        TEXT PRIMARY KEY NOT NULL,
    position       INTEGER NOT NULL,
    total          INTEGER,
    completed_at   INTEGER,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

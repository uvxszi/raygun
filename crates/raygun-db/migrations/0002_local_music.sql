-- Local music library (M07). Device-specific file paths, so these tables are never synced
-- (ARCHITECTURE §5: local_* tables).

CREATE TABLE local_library_root (
    path     TEXT PRIMARY KEY NOT NULL,
    added_at INTEGER NOT NULL
) STRICT;

CREATE TABLE local_track (
    id           TEXT PRIMARY KEY NOT NULL,   -- UUIDv7, stable across rescans of the same path
    path         TEXT NOT NULL UNIQUE,
    file_size    INTEGER NOT NULL,
    file_mtime   INTEGER NOT NULL,
    title        TEXT NOT NULL,
    artist       TEXT,
    album_artist TEXT,
    album        TEXT,
    track_no     INTEGER,
    disc_no      INTEGER,
    year         INTEGER,
    genre        TEXT,
    duration_ms  INTEGER,
    art          TEXT,                        -- artwork file name in cache/images
    group_artist TEXT NOT NULL,               -- album artist, else artist, else "Unknown artist"
    group_album  TEXT NOT NULL,               -- album, else "Unknown album"
    scanned_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX local_track_album ON local_track (group_artist COLLATE NOCASE, group_album COLLATE NOCASE);
CREATE INDEX local_track_album_title ON local_track (group_album COLLATE NOCASE);

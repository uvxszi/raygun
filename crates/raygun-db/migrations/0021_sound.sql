-- A song's own sound and lyrics (M46.12): trims, volume, the lyrics offset and edited lyrics
-- (JSON `{synced, plain}`), by track key. Stamped like the other synced tables.
CREATE TABLE song_edit (
    track_key        TEXT PRIMARY KEY NOT NULL,
    start_ms         INTEGER NOT NULL DEFAULT 0,
    end_ms           INTEGER,
    gain             INTEGER NOT NULL DEFAULT 0,
    lyrics_offset_ms INTEGER NOT NULL DEFAULT 0,
    lyrics           TEXT,
    updated_at_hlc   INTEGER NOT NULL,
    origin_device    TEXT NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0
) STRICT;

-- How loud songs measured while they played (M46.12), for loudness normalization. Device-local.
CREATE TABLE local_track_loudness (
    track_key   TEXT PRIMARY KEY NOT NULL,
    lufs        REAL NOT NULL,
    measured_at INTEGER NOT NULL
) STRICT;

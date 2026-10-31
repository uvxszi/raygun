-- Favorites, playlists and play history (M18). Synced tables: UUIDv7 ids, HLC stamps, tombstones
-- (ARCHITECTURE §5). Each row carries the track's metadata itself, so rows saved on two devices
-- never contend for a shared track row. `source` is a portable MediaSource as JSON (http(s) or
-- YouTube), or NULL when the track is found through the local library or the resolver.

CREATE TABLE music_favorite (
    id             TEXT PRIMARY KEY NOT NULL,
    track_key      TEXT NOT NULL,              -- normalized "artist|title" (M14)
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    duration_ms    INTEGER,
    artwork_url    TEXT,
    source         TEXT,
    added_at       INTEGER NOT NULL,           -- unix ms
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX music_favorite_key ON music_favorite (track_key) WHERE deleted = 0;

CREATE TABLE music_playlist (
    id             TEXT PRIMARY KEY NOT NULL,
    name           TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE music_playlist_item (
    id             TEXT PRIMARY KEY NOT NULL,
    playlist_id    TEXT NOT NULL,
    position       REAL NOT NULL,              -- midpoints between neighbours on reorder
    track_key      TEXT NOT NULL,
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    duration_ms    INTEGER,
    artwork_url    TEXT,
    source         TEXT,
    added_at       INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX music_playlist_item_order ON music_playlist_item (playlist_id, position);

-- Append-only; merged across devices as a union (ARCHITECTURE §10).
CREATE TABLE music_play_history (
    id             TEXT PRIMARY KEY NOT NULL,
    track_key      TEXT NOT NULL,
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    duration_ms    INTEGER,
    artwork_url    TEXT,
    source         TEXT,
    played_at      INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX music_play_history_played ON music_play_history (played_at);

-- Lets saved tracks find the same song in the local library. Filled by the app (it owns the
-- normalization); NULL until then.
ALTER TABLE local_track ADD COLUMN track_key TEXT;
CREATE INDEX local_track_key ON local_track (track_key);

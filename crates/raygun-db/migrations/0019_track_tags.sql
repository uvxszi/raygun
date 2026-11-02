-- Genres and moods found for songs (M46.9). Derived from sources and kept on this device only.
CREATE TABLE local_track_tags (
    track_key  TEXT PRIMARY KEY NOT NULL, -- the resolver's normalized "artist|title"
    tags       TEXT NOT NULL,             -- JSON [{ "name": "Rock", "kind": "genre" }]; [] = none found
    fetched_at INTEGER NOT NULL           -- unix ms
) STRICT;

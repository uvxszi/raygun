-- Song lengths Raygun learned (M46.26): from playback, from sources that send them, and from
-- lookups for songs whose source didn't. Kept on this device only.
CREATE TABLE local_track_duration (
    track_key   TEXT PRIMARY KEY NOT NULL, -- the resolver's normalized "artist|title"
    duration_ms INTEGER,                   -- NULL: looked up and not found
    source      TEXT NOT NULL,             -- 'playback', 'source' or 'lookup'
    updated_at  INTEGER NOT NULL           -- unix ms
) STRICT;

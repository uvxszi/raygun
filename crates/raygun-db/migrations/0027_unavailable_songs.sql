-- Songs no source had when they were imported (M46.31): shown as Unavailable until a source is
-- assigned or one is found when played. Device-local.
CREATE TABLE local_track_unavailable (
    track_key TEXT PRIMARY KEY NOT NULL,
    marked_at INTEGER NOT NULL -- unix ms
) STRICT;

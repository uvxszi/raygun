-- Covers found at other sources (M46.40), moved here from the `covers.fallback` value so each is
-- kept and forgotten on its own: by what they're the cover of ("song:<track key>",
-- "album:<artist|album>", "artist:<name>"), the address found (NULL: none was) and the source that
-- gave it. Device-local.
CREATE TABLE local_cover_found (
    key      TEXT PRIMARY KEY NOT NULL,
    url      TEXT,
    source   TEXT,
    found_at INTEGER NOT NULL           -- unix ms
) STRICT;

-- When each song last played on this device. With the times its other kept facts were made or
-- used, it says how recently a song was played or seen, which decides what of it is kept
-- (Settings, Data, Storage).
CREATE TABLE local_track_seen (
    track_key TEXT PRIMARY KEY NOT NULL, -- the resolver's normalized "artist|title"
    seen_at   INTEGER NOT NULL           -- unix ms
) STRICT;

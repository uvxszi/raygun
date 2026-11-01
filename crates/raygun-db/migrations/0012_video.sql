-- Watching progress (M38): one row per thing watched (a movie, an episode, a link), keyed by a
-- stable content key such as `url:<address>` or, later, `stremio:<type>:<id>`. Follows the synced
-- conventions so "continue watching" can travel with device sync later.

CREATE TABLE video_progress (
    id              TEXT PRIMARY KEY NOT NULL,
    key             TEXT NOT NULL,
    title           TEXT NOT NULL,
    subtitle        TEXT,                       -- e.g. "S1 E3 · Pilot"
    poster          TEXT,
    request         TEXT NOT NULL,              -- JSON: how to play it again
    position_ms     INTEGER NOT NULL DEFAULT 0,
    duration_ms     INTEGER,
    user_watched_at INTEGER,                    -- unix ms; NULL while unfinished
    updated_at      INTEGER NOT NULL,           -- unix ms of the last watch
    updated_at_hlc  INTEGER NOT NULL,
    origin_device   TEXT NOT NULL,
    deleted         INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX video_progress_key ON video_progress (key) WHERE deleted = 0;
CREATE INDEX video_progress_recent ON video_progress (updated_at);

-- Lyrics found for songs (M46.46), kept on this device so they show at once and offline. A row
-- without lyrics only remembers when the song's lyrics were last shown (or that none were found).
-- Forgotten when the song wasn't played for a while (Settings → Data → Storage).
CREATE TABLE local_lyrics_cache (
    track_key  TEXT PRIMARY KEY NOT NULL,
    lyrics     TEXT,
    fetched_at INTEGER NOT NULL,
    used_at    INTEGER NOT NULL
) STRICT;

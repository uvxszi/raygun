-- Spoken word (M46.32): an episode's transcript as its feed links it (Podcasting 2.0
-- <podcast:transcript>), as "<type>|<url>".
ALTER TABLE podcast_episode ADD COLUMN remote_transcript TEXT;

-- Captions for episodes and chapters: from the feed's transcript or transcribed. Device-local.
CREATE TABLE local_episode_caption (
    episode_id TEXT NOT NULL,
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    text       TEXT NOT NULL,
    PRIMARY KEY (episode_id, start_ms)
) STRICT;

-- Which stretches of an episode have captions (JSON [[start_ms, end_ms], ...]), where they
-- came from ('feed', 'local', 'online') and whether the whole episode is done.
CREATE TABLE local_caption_progress (
    episode_id TEXT PRIMARY KEY NOT NULL,
    source     TEXT NOT NULL,
    ranges     TEXT NOT NULL DEFAULT '[]',
    complete   INTEGER NOT NULL DEFAULT 0
) STRICT;

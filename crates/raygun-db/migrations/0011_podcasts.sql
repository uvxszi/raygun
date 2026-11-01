-- Podcasts (M37). Rows follow the synced conventions (UUIDv7 ids, HLC stamps, tombstones) so
-- device sync can carry subscriptions and progress later; columns are prefixed like `title`'s:
-- `remote_*` from the feed, `user_*` the user's, `state_*` bookkeeping.

CREATE TABLE podcast (
    id                 TEXT PRIMARY KEY NOT NULL,
    feed_url           TEXT NOT NULL,
    remote_title       TEXT NOT NULL,
    remote_author      TEXT,
    remote_description TEXT,
    remote_artwork     TEXT,
    remote_link        TEXT,
    user_subscribed_at INTEGER,                 -- unix ms; NULL when not subscribed
    state_refreshed_at INTEGER,
    updated_at_hlc     INTEGER NOT NULL,
    origin_device      TEXT NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX podcast_feed ON podcast (feed_url) WHERE deleted = 0;

CREATE TABLE podcast_episode (
    id                  TEXT PRIMARY KEY NOT NULL,
    podcast_id          TEXT NOT NULL,
    guid                TEXT NOT NULL,          -- the feed's id for the episode
    remote_title        TEXT NOT NULL,
    remote_description  TEXT,
    remote_audio_url    TEXT NOT NULL,
    remote_audio_type   TEXT,
    remote_duration_ms  INTEGER,
    remote_published_at INTEGER,                -- unix ms
    remote_artwork      TEXT,
    user_position_ms    INTEGER NOT NULL DEFAULT 0,
    user_played_at      INTEGER,                -- unix ms; NULL while unplayed
    state_found_at      INTEGER NOT NULL,       -- when this device first saw it
    updated_at_hlc      INTEGER NOT NULL,
    origin_device       TEXT NOT NULL,
    deleted             INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX podcast_episode_guid ON podcast_episode (podcast_id, guid) WHERE deleted = 0;
CREATE INDEX podcast_episode_published ON podcast_episode (podcast_id, remote_published_at);
CREATE INDEX podcast_episode_audio ON podcast_episode (remote_audio_url);

-- Downloaded episodes. Files live on this device only, so this table is never synced.
CREATE TABLE local_podcast_download (
    episode_id     TEXT PRIMARY KEY NOT NULL,
    state          TEXT NOT NULL,               -- downloading | done | failed
    path           TEXT,                        -- under the downloads folder, once done
    received_bytes INTEGER NOT NULL DEFAULT 0,
    total_bytes    INTEGER,
    error          TEXT,
    updated_at     INTEGER NOT NULL             -- unix ms
) STRICT;

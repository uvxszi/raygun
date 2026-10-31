-- Stream resolution cache (M14). Device-local: stream URLs expire and differ per network.
CREATE TABLE local_stream_candidate (
    track_key    TEXT NOT NULL,                -- normalized "artist|title"
    provider_id  TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    title        TEXT NOT NULL,
    source       TEXT,                         -- JSON MediaSource, once resolved
    expires_at   INTEGER,                      -- unix ms when the resolved URL stops working
    verified_at  INTEGER,                      -- unix ms of the last successful play
    failed_at    INTEGER,                      -- unix ms of the last failure
    failure      TEXT,
    PRIMARY KEY (track_key, provider_id, candidate_id)
) STRICT;

CREATE TABLE local_stream_provider_stats (
    provider_id TEXT PRIMARY KEY NOT NULL,
    successes   INTEGER NOT NULL DEFAULT 0,
    failures    INTEGER NOT NULL DEFAULT 0
) STRICT;

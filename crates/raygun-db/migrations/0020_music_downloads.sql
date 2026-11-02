-- Songs saved into the app to play offline (M46.10). Device-local: files live on this device.
CREATE TABLE local_music_download (
    id          TEXT PRIMARY KEY NOT NULL,
    track_key   TEXT NOT NULL,            -- the resolver's normalized "artist|title"
    title       TEXT NOT NULL,
    artist      TEXT,
    album       TEXT,
    duration_ms INTEGER,
    artwork_url TEXT,                     -- the source's address, never a proxy link
    source      TEXT,                     -- the MediaSource it was asked with (JSON), if any
    -- queued | downloading | recording | done | failed | needsRecording
    state       TEXT NOT NULL,
    kind        TEXT,                     -- file | recording, once known
    provider    TEXT,                     -- where the audio came from
    file        TEXT,                     -- the saved file's name in the download's folder
    done_bytes  INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER,
    error       TEXT,
    created_at  INTEGER NOT NULL,         -- unix ms
    finished_at INTEGER
) STRICT;

CREATE UNIQUE INDEX local_music_download_key ON local_music_download (track_key);
CREATE INDEX local_music_download_queue ON local_music_download (state, created_at);

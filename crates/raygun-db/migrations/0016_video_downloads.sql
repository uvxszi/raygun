-- Video downloads (M43): device-local, like chapter downloads. One row per progress key (an
-- episode or film); the files live under downloads/video/<id>/.
CREATE TABLE local_video_download (
    id              TEXT PRIMARY KEY NOT NULL,
    key             TEXT NOT NULL UNIQUE,          -- the progress key it plays under
    title_id        TEXT,                          -- the library title, when there is one
    title           TEXT NOT NULL,
    subtitle        TEXT,
    poster          TEXT,
    url             TEXT NOT NULL,                 -- what was downloaded
    headers         TEXT NOT NULL DEFAULT '{}',    -- JSON object
    format          TEXT,                          -- hls, or NULL for a plain file
    subtitles       TEXT NOT NULL DEFAULT '[]',    -- JSON: the remote tracks
    state           TEXT NOT NULL,                 -- queued, downloading, done, failed
    file            TEXT,                          -- the playable file in its folder, once done
    local_subtitles TEXT NOT NULL DEFAULT '[]',    -- JSON: the saved tracks
    done_bytes      INTEGER NOT NULL DEFAULT 0,
    total_bytes     INTEGER,
    done_parts      INTEGER NOT NULL DEFAULT 0,    -- HLS segments
    total_parts     INTEGER,
    error           TEXT,
    queued_at       INTEGER NOT NULL,
    finished_at     INTEGER
) STRICT;

CREATE INDEX local_video_download_state ON local_video_download (state, queued_at);
CREATE INDEX local_video_download_title ON local_video_download (title_id);

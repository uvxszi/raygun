-- Library updates and downloads (M28).

-- When this device first stored a unit (unix ms); NULL for units stored before M28. The updates
-- feed lists units found after their title joined the library.
ALTER TABLE unit ADD COLUMN state_found_at INTEGER;
CREATE INDEX unit_found ON unit (state_found_at) WHERE state_found_at IS NOT NULL;

-- Chapter downloads. Files live on this device only, so this table is never synced.
CREATE TABLE local_manga_download (
    unit_id     TEXT PRIMARY KEY NOT NULL,
    title_id    TEXT NOT NULL,
    state       TEXT NOT NULL,              -- queued | downloading | done | failed
    pages_done  INTEGER NOT NULL DEFAULT 0,
    pages_total INTEGER,
    error       TEXT,
    queued_at   INTEGER NOT NULL,           -- unix ms; the queue runs oldest first
    finished_at INTEGER
) STRICT;

CREATE INDEX local_manga_download_state ON local_manga_download (state, queued_at);

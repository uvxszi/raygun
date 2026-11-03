-- Local Files (M46.27): folders the user added, and what their files became. Device-local: the
-- folders are this device's.
CREATE TABLE local_folder (
    id         TEXT PRIMARY KEY NOT NULL,
    location   TEXT NOT NULL UNIQUE, -- a path, or an Android tree address (content://…)
    name       TEXT NOT NULL,
    path       TEXT,                 -- the folder as a path, when there is one (Android: shared storage)
    added_at   INTEGER NOT NULL,     -- unix ms
    scanned_at INTEGER,              -- unix ms
    counts     TEXT NOT NULL DEFAULT '{}' -- JSON {"songs":0,"comics":0,"books":0,"videos":0}
) STRICT;

-- Comics, books and videos found in a folder: unchanged files aren't read again.
CREATE TABLE local_folder_file (
    folder_id TEXT NOT NULL,
    rel_path  TEXT NOT NULL, -- inside the folder, with "/"
    size      INTEGER NOT NULL,
    modified  INTEGER NOT NULL, -- unix ms
    kind      TEXT NOT NULL,    -- 'comic', 'book' or 'video'
    item      TEXT,             -- what it became: "<source>:<remote id>"
    PRIMARY KEY (folder_id, rel_path)
) STRICT;

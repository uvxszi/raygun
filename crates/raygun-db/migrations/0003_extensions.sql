-- Extension repositories (M13). Synced: the same repositories show up on every device.
CREATE TABLE extension_repo (
    id             TEXT PRIMARY KEY NOT NULL,  -- UUIDv7
    url            TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,              -- the index's own name, refreshed on fetch
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

-- Raygun's official repository. A fixed id lets every device's seed row merge into one.
INSERT INTO extension_repo (id, url, name, updated_at_hlc, origin_device, deleted)
VALUES (
    '0192a000-0000-7000-8000-000000000001',
    'https://raw.githubusercontent.com/uvxszi/raygun/main/repo/index.json',
    'Raygun Extensions',
    0,
    'seed',
    0
);

-- The user's values for extension settings. Synced, merged per key like `setting`.
CREATE TABLE extension_setting (
    extension_id   TEXT NOT NULL,
    key            TEXT NOT NULL,
    value          TEXT NOT NULL,              -- JSON
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (extension_id, key)
) STRICT;

-- Which repository each installed extension came from. Device-local: every device installs
-- extensions itself (ARCHITECTURE §10).
CREATE TABLE local_extension_install (
    extension_id TEXT PRIMARY KEY NOT NULL,
    repo_url     TEXT NOT NULL,
    version      TEXT NOT NULL,
    sha256       TEXT NOT NULL,
    installed_at INTEGER NOT NULL              -- unix ms
) STRICT;

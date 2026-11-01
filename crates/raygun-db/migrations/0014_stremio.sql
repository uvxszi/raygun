-- Stremio addons (M41). Synced conventions, so the addon list can travel with device sync later;
-- the cached manifest is refreshed from the addon itself on each device.

CREATE TABLE stremio_addon (
    id             TEXT PRIMARY KEY NOT NULL,
    transport_url  TEXT NOT NULL,
    manifest       TEXT,                        -- the last manifest fetched (JSON)
    fetched_at     INTEGER,                     -- unix ms
    position       INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX stremio_addon_url ON stremio_addon (transport_url) WHERE deleted = 0;

-- Cinemeta (catalogs and metadata) and OpenSubtitles v3, Stremio's own addons. Fixed ids let every
-- device's seed rows merge into one.
INSERT INTO stremio_addon (id, transport_url, position, updated_at_hlc, origin_device, deleted)
VALUES
    ('0192a000-0000-7000-8000-000000000101', 'https://v3-cinemeta.strem.io/manifest.json', 0, 0, 'seed', 0),
    ('0192a000-0000-7000-8000-000000000102', 'https://opensubtitles-v3.strem.io/manifest.json', 1, 0, 'seed', 0);

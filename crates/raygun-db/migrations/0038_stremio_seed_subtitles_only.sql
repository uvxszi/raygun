-- New installs start with OpenSubtitles v3 only: Cinemeta, whose catalogues list commercial films
-- and series, is no longer added. A database that has run before (it has a device id by then)
-- keeps its addons as they are.
DELETE FROM stremio_addon
WHERE id = '0192a000-0000-7000-8000-000000000101'
  AND NOT EXISTS (SELECT 1 FROM local_meta WHERE key = 'device_id');

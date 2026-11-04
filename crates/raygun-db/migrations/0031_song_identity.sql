-- Songs a playlist held twice by mistake (M46.48): adding to a playlist didn't check what it
-- already had. Each later copy of a song (same playlist, same key) becomes a tombstone; the first
-- one added stays. The tombstones are this device's writes, stamped past every timestamp it has
-- issued or seen (ARCHITECTURE §10), so the repair reaches the other devices too. Copies saved
-- under another spelling of the song are merged by the app once (`collections::repair`).
UPDATE music_playlist_item
SET deleted = 1,
    updated_at_hlc = MAX(
        COALESCE((SELECT CAST(value AS INTEGER) FROM local_meta WHERE key = 'hlc_last'), 0) + 1,
        CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) * 65536
    ),
    origin_device = COALESCE((SELECT value FROM local_meta WHERE key = 'device_id'), origin_device)
WHERE deleted = 0
  AND EXISTS (
      SELECT 1 FROM music_playlist_item AS earlier
      WHERE earlier.playlist_id = music_playlist_item.playlist_id
        AND earlier.track_key = music_playlist_item.track_key
        AND earlier.deleted = 0
        AND (earlier.added_at < music_playlist_item.added_at
             OR (earlier.added_at = music_playlist_item.added_at
                 AND earlier.id < music_playlist_item.id))
  );

-- The clock resumes past those stamps.
UPDATE local_meta
SET value = CAST((SELECT MAX(updated_at_hlc) FROM music_playlist_item) AS TEXT)
WHERE key = 'hlc_last'
  AND CAST(value AS INTEGER) < (SELECT MAX(updated_at_hlc) FROM music_playlist_item);

-- Songs that came in through Import songs: picking a source for one offers the source's title,
-- artist and album by default. Device-local, like the unavailable marks (0027).
CREATE TABLE IF NOT EXISTS local_track_imported (
    track_key   TEXT PRIMARY KEY NOT NULL,
    imported_at INTEGER NOT NULL -- unix ms
) STRICT;

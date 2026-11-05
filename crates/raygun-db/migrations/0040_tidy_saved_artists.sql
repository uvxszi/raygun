-- Saved songs' artists without the quotes a source, or a screenshot read as text, left around
-- them, as `raygun_resolver::tidy_credit` tidies new ones: `"Name"`, `"Name` and `Name"` lose
-- them, while quotes inside a name (`"Weird Al" Yankovic`) and apostrophes stay. Favorites and
-- playlist songs change as this device's writes, stamped past every timestamp it has issued or
-- seen (as 0031 does), so linked devices take the tidied names too; downloads are device-local.

-- Each stored name and its tidied form, worked out once.
CREATE TEMP TABLE tidied_artist (old TEXT PRIMARY KEY NOT NULL, new TEXT);

INSERT INTO tidied_artist (old, new)
SELECT artist,
       NULLIF(trim(CASE
           WHEN quotes = 2 AND opens AND closes THEN substr(text, 2, length(text) - 2)
           WHEN quotes = 1 AND opens THEN substr(text, 2)
           WHEN quotes = 1 AND closes THEN substr(text, 1, length(text) - 1)
           ELSE text
       END, ' ' || char(9, 10, 13, 160)), '')
FROM (
    SELECT artist,
           text,
           length(text) - length(replace(replace(replace(replace(replace(replace(replace(
               text, '"', ''), '“', ''), '”', ''), '„', ''), '‟', ''), '«', ''), '»', ''))
               AS quotes,
           substr(text, 1, 1) IN ('"', '“', '”', '„', '‟', '«', '»') AS opens,
           substr(text, -1, 1) IN ('"', '“', '”', '„', '‟', '«', '»') AS closes
    FROM (
        SELECT artist, trim(artist, ' ' || char(9, 10, 13, 160)) AS text
        FROM (
            SELECT artist FROM music_favorite WHERE artist IS NOT NULL AND deleted = 0
            UNION
            SELECT artist FROM music_playlist_item WHERE artist IS NOT NULL AND deleted = 0
            UNION
            SELECT artist FROM local_music_download WHERE artist IS NOT NULL
        )
    )
);

DELETE FROM tidied_artist WHERE new IS old;

UPDATE music_favorite
SET artist = (SELECT new FROM tidied_artist WHERE old = music_favorite.artist),
    updated_at_hlc = MAX(
        COALESCE((SELECT CAST(value AS INTEGER) FROM local_meta WHERE key = 'hlc_last'), 0) + 1,
        CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) * 65536
    ),
    origin_device = COALESCE((SELECT value FROM local_meta WHERE key = 'device_id'), origin_device)
WHERE deleted = 0 AND artist IN (SELECT old FROM tidied_artist);

UPDATE music_playlist_item
SET artist = (SELECT new FROM tidied_artist WHERE old = music_playlist_item.artist),
    updated_at_hlc = MAX(
        COALESCE((SELECT CAST(value AS INTEGER) FROM local_meta WHERE key = 'hlc_last'), 0) + 1,
        CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) * 65536
    ),
    origin_device = COALESCE((SELECT value FROM local_meta WHERE key = 'device_id'), origin_device)
WHERE deleted = 0 AND artist IN (SELECT old FROM tidied_artist);

UPDATE local_music_download
SET artist = (SELECT new FROM tidied_artist WHERE old = local_music_download.artist)
WHERE artist IN (SELECT old FROM tidied_artist);

-- The clock resumes past those stamps.
UPDATE local_meta
SET value = CAST((
        SELECT MAX(stamp) FROM (
            SELECT MAX(updated_at_hlc) AS stamp FROM music_favorite
            UNION ALL
            SELECT MAX(updated_at_hlc) FROM music_playlist_item
        )
    ) AS TEXT)
WHERE key = 'hlc_last'
  AND CAST(value AS INTEGER) < (
        SELECT MAX(stamp) FROM (
            SELECT MAX(updated_at_hlc) AS stamp FROM music_favorite
            UNION ALL
            SELECT MAX(updated_at_hlc) FROM music_playlist_item
        )
    );

DROP TABLE tidied_artist;

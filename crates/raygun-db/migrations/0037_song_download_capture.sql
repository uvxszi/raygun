-- Song downloads (M46.49): where each saved copy came from, for Check downloads to compare with the
-- song's source and names now; the copy kept while a song downloads again; what a check found.
ALTER TABLE local_music_download ADD COLUMN origin_provider TEXT;   -- the stream's provider id
ALTER TABLE local_music_download ADD COLUMN origin_stream TEXT;     -- the stream's id at its provider
ALTER TABLE local_music_download ADD COLUMN origin_source TEXT;     -- the stream (MediaSource JSON)
ALTER TABLE local_music_download ADD COLUMN origin_title TEXT;      -- the stream's own title
ALTER TABLE local_music_download ADD COLUMN origin_artist TEXT;     -- the stream's own artist
ALTER TABLE local_music_download ADD COLUMN file_ms INTEGER;        -- the saved file's length
ALTER TABLE local_music_download ADD COLUMN replacing TEXT;         -- the file kept while it downloads again
ALTER TABLE local_music_download ADD COLUMN problem TEXT;           -- what a check found (JSON)

-- Recording what plays is gone: a stream that plays is captured in the background instead. Songs
-- that waited to be recorded from a stream go back to the queue to be captured first; ones only
-- YouTube's player had can't be captured, and failed.
UPDATE local_music_download SET kind = 'capture' WHERE kind = 'recording';
UPDATE local_music_download
   SET state = 'queued', kind = 'capture', error = NULL, error_detail = NULL,
       finished_at = NULL, done_bytes = 0, total_bytes = NULL
 WHERE state IN ('needsRecording', 'recording') AND source LIKE '{"kind":"url",%';
UPDATE local_music_download
   SET state = 'failed', kind = NULL,
       error = 'Only YouTube has this song, and it couldn''t be saved from there. Choose another source.'
 WHERE state IN ('needsRecording', 'recording');

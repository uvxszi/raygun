-- Song downloads keep a place in the queue of their own (moved, or put first when tried again),
-- and say why they failed in two parts: a line for the listener and each source's own error.
ALTER TABLE local_music_download ADD COLUMN queued_at INTEGER;   -- unix ms; the queue runs oldest first
ALTER TABLE local_music_download ADD COLUMN error_detail TEXT;   -- the sources' errors, for More info

UPDATE local_music_download SET queued_at = created_at;

DROP INDEX local_music_download_queue;
CREATE INDEX local_music_download_queue ON local_music_download (state, queued_at);

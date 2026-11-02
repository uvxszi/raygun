-- The bitrate of a stream that played (M46.18), so a remembered pick is used again only when it
-- suits the Audio quality wanted now. NULL when the source didn't say.
ALTER TABLE local_stream_candidate ADD COLUMN bitrate_kbps INTEGER;

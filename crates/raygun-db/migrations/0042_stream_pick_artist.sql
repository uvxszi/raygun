-- The artist a stream's source named when it played, so a song can take its details from the
-- source Raygun found for it ("Use details from…"). NULL for picks remembered before, and when
-- the source named none.
ALTER TABLE local_stream_candidate ADD COLUMN artist TEXT;

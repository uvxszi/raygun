-- Headers a source needs sent with its cover requests (M24), e.g. a User-Agent: some hosts
-- answer bare or browser-like requests with a bot challenge. A JSON object.
ALTER TABLE title ADD COLUMN remote_cover_headers TEXT NOT NULL DEFAULT '{}';

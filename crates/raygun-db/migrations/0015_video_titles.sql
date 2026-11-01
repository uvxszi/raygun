-- Video titles (M42): what a video source says a title is (`movie`, `series`, `anime` or `other`)
-- and its release year. NULL for manga and novels.
ALTER TABLE title ADD COLUMN remote_media_type TEXT;
ALTER TABLE title ADD COLUMN remote_year INTEGER;

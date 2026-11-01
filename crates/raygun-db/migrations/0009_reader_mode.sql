-- How the user reads a title (M27): webtoon (vertical strip), pagedLtr or pagedRtl.
ALTER TABLE title ADD COLUMN user_reader_mode TEXT NOT NULL DEFAULT 'webtoon';

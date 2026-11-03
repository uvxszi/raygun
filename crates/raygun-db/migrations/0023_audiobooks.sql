-- Audiobooks (M46.22): a book is a feed of chapters (LibriVox publishes one per book), kept with the
-- podcasts but listed apart. Chapters keep their number from the feed (`itunes:episode`, else the
-- item's place), since book feeds have no dates.
ALTER TABLE podcast ADD COLUMN remote_kind TEXT NOT NULL DEFAULT 'podcast';
ALTER TABLE podcast_episode ADD COLUMN remote_number INTEGER;

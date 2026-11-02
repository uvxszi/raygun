-- Library depth (M46): the user's lists (CloudStream's watch status), favorites and pins on
-- titles, and a category's own display and sort (Mihon's per-category settings).
ALTER TABLE title ADD COLUMN user_list TEXT;          -- reading | planned | completed | onHold | dropped
ALTER TABLE title ADD COLUMN user_favorite_at INTEGER;
ALTER TABLE title ADD COLUMN user_pinned_at INTEGER;
ALTER TABLE category ADD COLUMN user_display TEXT;    -- JSON { mode, sort, descending }; NULL: the library's

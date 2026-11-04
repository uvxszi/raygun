-- A title's reading mode is empty until one is picked for it: it then follows the reader's default
-- mode (Settings → Reader). Every title said `webtoon` before, picked or not.
UPDATE title SET user_reader_mode = '' WHERE user_reader_mode = 'webtoon';

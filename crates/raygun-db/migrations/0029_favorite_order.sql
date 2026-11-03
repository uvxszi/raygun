-- Favorites in the user's own order (M46.44). NULL until a favorite is moved: it then reads
-- as its added time (newest first), so new favorites arrive on top. Synced with the row.
ALTER TABLE music_favorite ADD COLUMN position REAL;

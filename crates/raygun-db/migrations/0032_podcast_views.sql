-- How each show and book lists its episodes, and the speed it plays at (M46.48). The order is
-- JSON (`{"sort":"date","descending":true,"filters":["unplayed"]}`); NULL means the kind's own
-- (podcasts newest first, books in chapter order). A NULL speed leaves the player's as it is.
ALTER TABLE podcast ADD COLUMN user_episode_order TEXT;
ALTER TABLE podcast ADD COLUMN user_speed REAL;

-- When listening last moved an episode on (unix ms), so Continue listening puts the latest first.
ALTER TABLE podcast_episode ADD COLUMN user_listened_at INTEGER;
CREATE INDEX podcast_episode_listened ON podcast_episode (user_listened_at)
    WHERE user_listened_at IS NOT NULL;

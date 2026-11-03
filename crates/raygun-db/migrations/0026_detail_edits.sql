-- The user's own details for songs, albums and artists (M46.30), shown over what sources said.
-- Device-local for now.
CREATE TABLE local_detail_edit (
    kind       TEXT NOT NULL,    -- 'song', 'album' or 'artist'
    key        TEXT NOT NULL,    -- song: track key; album: normalized "artist|album"; artist: normalized name
    title      TEXT,             -- the song's title, the album's title or the artist's name
    artist     TEXT,             -- songs only
    album      TEXT,             -- songs only
    artwork    TEXT,             -- an image address, or "local:<path>" under the downloads folder
    updated_at INTEGER NOT NULL, -- unix ms
    PRIMARY KEY (kind, key)
) STRICT;

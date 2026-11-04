-- Translations of songs' lyrics (M46.48), kept on this device: what lyrics sources gave with their
-- lyrics (`source:<name>`), and what a model on this device (`device`) or an AI translator
-- (`ai:<provider id>`) made. Each line keeps the words it translates (JSON `[{t, s, r}]`: time,
-- words, translation), so it lines up again with whichever lyrics show. Forgotten with the song's
-- kept lyrics (Settings → Data → Storage).
CREATE TABLE local_lyrics_translation (
    track_key  TEXT NOT NULL,
    translator TEXT NOT NULL,
    language   TEXT NOT NULL,
    kind       TEXT NOT NULL,
    by_name    TEXT NOT NULL,
    lines      TEXT NOT NULL,
    made_at    INTEGER NOT NULL,
    PRIMARY KEY (track_key, translator, language, kind)
) STRICT;

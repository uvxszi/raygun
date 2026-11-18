-- Song edits (trims, volume, the lyrics offset and edited lyrics) sync with the library between
-- linked devices (M46.57): the table was stamped from the start; the log now records its rows.
CREATE TRIGGER sync_log_song_edit_insert AFTER INSERT ON song_edit BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'song_edit' AND row_key = NEW.track_key;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('song_edit', NEW.track_key);
END;
CREATE TRIGGER sync_log_song_edit_update AFTER UPDATE ON song_edit BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'song_edit' AND row_key = NEW.track_key;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('song_edit', NEW.track_key);
END;
INSERT INTO local_sync_log (tbl, row_key) SELECT 'song_edit', track_key FROM song_edit;
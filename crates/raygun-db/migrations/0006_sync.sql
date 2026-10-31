-- Device sync (M21). All of these tables are device-local bookkeeping and never sync themselves.

-- One row per synced row that changed, keyed by a local sequence. Triggers keep it current for
-- local writes and merged ones alike, so a row that arrived from another device is forwarded too.
-- Only the latest change per row is kept: a peer needs the row, not its history. `from_peer`
-- names the device a merged row came from, so it isn't sent straight back there.
CREATE TABLE local_sync_log (
    seq       INTEGER PRIMARY KEY AUTOINCREMENT,
    tbl       TEXT NOT NULL,
    row_key   TEXT NOT NULL,
    from_peer TEXT
) STRICT;

CREATE INDEX local_sync_log_row ON local_sync_log (tbl, row_key);

CREATE TRIGGER sync_log_setting_insert AFTER INSERT ON setting BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'setting' AND row_key = NEW.key;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('setting', NEW.key);
END;
CREATE TRIGGER sync_log_setting_update AFTER UPDATE ON setting BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'setting' AND row_key = NEW.key;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('setting', NEW.key);
END;

CREATE TRIGGER sync_log_favorite_insert AFTER INSERT ON music_favorite BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_favorite' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_favorite', NEW.id);
END;
CREATE TRIGGER sync_log_favorite_update AFTER UPDATE ON music_favorite BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_favorite' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_favorite', NEW.id);
END;

CREATE TRIGGER sync_log_playlist_insert AFTER INSERT ON music_playlist BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_playlist' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_playlist', NEW.id);
END;
CREATE TRIGGER sync_log_playlist_update AFTER UPDATE ON music_playlist BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_playlist' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_playlist', NEW.id);
END;

CREATE TRIGGER sync_log_playlist_item_insert AFTER INSERT ON music_playlist_item BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_playlist_item' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_playlist_item', NEW.id);
END;
CREATE TRIGGER sync_log_playlist_item_update AFTER UPDATE ON music_playlist_item BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_playlist_item' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_playlist_item', NEW.id);
END;

CREATE TRIGGER sync_log_history_insert AFTER INSERT ON music_play_history BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_play_history' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_play_history', NEW.id);
END;
CREATE TRIGGER sync_log_history_update AFTER UPDATE ON music_play_history BEGIN
    DELETE FROM local_sync_log WHERE tbl = 'music_play_history' AND row_key = NEW.id;
    INSERT INTO local_sync_log (tbl, row_key) VALUES ('music_play_history', NEW.id);
END;

-- Rows written before this migration still need to reach a newly paired device.
INSERT INTO local_sync_log (tbl, row_key) SELECT 'setting', key FROM setting;
INSERT INTO local_sync_log (tbl, row_key) SELECT 'music_favorite', id FROM music_favorite;
INSERT INTO local_sync_log (tbl, row_key) SELECT 'music_playlist', id FROM music_playlist;
INSERT INTO local_sync_log (tbl, row_key) SELECT 'music_playlist_item', id FROM music_playlist_item;
INSERT INTO local_sync_log (tbl, row_key) SELECT 'music_play_history', id FROM music_play_history;

-- Devices this one trusts. `sent_seq` is the last local_sync_log seq the peer acknowledged.
CREATE TABLE local_sync_peer (
    device_id    TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    public_key   TEXT NOT NULL,              -- Noise static key, hex
    address      TEXT,                       -- last known host:port
    paired_at    INTEGER NOT NULL,
    last_sync_at INTEGER,
    sent_seq     INTEGER NOT NULL DEFAULT 0,
    kinds        TEXT NOT NULL DEFAULT ''    -- kinds last sent to it; a new one means sending all again
) STRICT;

CREATE TABLE local_sync_session (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id      TEXT NOT NULL,
    peer_name    TEXT NOT NULL,
    started_at   INTEGER NOT NULL,
    finished_at  INTEGER,
    sent         INTEGER NOT NULL DEFAULT 0,
    received     INTEGER NOT NULL DEFAULT 0,
    changed      INTEGER NOT NULL DEFAULT 0,  -- received rows that changed something here
    error        TEXT,
    undone       INTEGER NOT NULL DEFAULT 0
) STRICT;

-- What each session changed, as the rows were before it (NULL: the row didn't exist).
CREATE TABLE local_sync_undo (
    session_id INTEGER NOT NULL,
    tbl        TEXT NOT NULL,
    row_key    TEXT NOT NULL,
    old_row    TEXT,
    PRIMARY KEY (session_id, tbl, row_key)
) STRICT;

-- Library categories (M39), Mihon-style: user-named shelves per kind of title (manga, novel),
-- each title in any number of them. Synced conventions, tombstones on both tables.

CREATE TABLE category (
    id             TEXT PRIMARY KEY NOT NULL,
    kind           TEXT NOT NULL,               -- manga | novel
    name           TEXT NOT NULL,
    position       INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX category_kind ON category (kind, position) WHERE deleted = 0;

CREATE TABLE title_category (
    title_id       TEXT NOT NULL,
    category_id    TEXT NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (title_id, category_id)
) STRICT;

CREATE INDEX title_category_category ON title_category (category_id) WHERE deleted = 0;

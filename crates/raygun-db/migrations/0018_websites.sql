-- Websites the user added, opened in Raygun's built-in browser (M46.3).
CREATE TABLE website (
    id             TEXT PRIMARY KEY NOT NULL,
    name           TEXT NOT NULL,
    url            TEXT NOT NULL,
    -- Where the site was left, to reopen there.
    user_last_url  TEXT,
    -- The ad blocker for this site (on unless turned off).
    user_block_ads INTEGER NOT NULL DEFAULT 1,
    position       INTEGER NOT NULL,
    updated_at_hlc INTEGER NOT NULL,
    origin_device  TEXT NOT NULL,
    deleted        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX website_order ON website (position) WHERE deleted = 0;

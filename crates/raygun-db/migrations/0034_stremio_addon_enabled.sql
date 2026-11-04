-- Stremio addons can be turned off without removing them; an addon that is off answers nothing.
ALTER TABLE stremio_addon ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;

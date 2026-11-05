-- Local Files (M46.51): how each folder is read. 'path': by its path (a desktop folder, or one
-- chosen in Raygun's own folder browser on Android); 'tree': through the grant of Android's system
-- folder picker (a content://… address); 'added': Raygun's own folder of files added one by one.
ALTER TABLE local_folder ADD COLUMN access TEXT NOT NULL DEFAULT 'path';
UPDATE local_folder SET access = 'tree' WHERE location LIKE 'content://%';

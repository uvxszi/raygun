-- Raygun ships no sources: a new install starts with no repository and no extension, and offers
-- Paraline as a suggestion. Earlier versions carried their first-party extensions inside the app
-- and seeded a repository on first run.
--
-- A device that has run Raygun before (it holds a repository row other than the untouched seed,
-- or installed something) is marked so the app installs the sources it used to carry from
-- Paraline on its next start (`extensions::move_former_built_ins`). Its repositories stay.
INSERT INTO local_meta (key, value)
SELECT 'extensions.formerBuiltIns', 'pending'
WHERE EXISTS (
        SELECT 1 FROM extension_repo
        WHERE NOT (origin_device = 'seed' AND updated_at_hlc = 0)
    )
   OR EXISTS (SELECT 1 FROM local_extension_install)
ON CONFLICT (key) DO NOTHING;

-- The untouched seed row (never synced: its timestamp is 0) leaves new installs.
DELETE FROM extension_repo WHERE origin_device = 'seed' AND updated_at_hlc = 0;

# M03 — `raygun-db` + typed settings

**Relevant docs:** ARCHITECTURE §5 (data model, sync-ready conventions), §10 (device sync: HLC, per-field merge).

## Goal
A SQLite database in the data root, with migrations, sync-ready bookkeeping and a typed settings service the UI can read and change. Settings survive restarts and moving the portable folder.

## Design decisions
- **Settings live in the database, not `settings.toml`.** They have to sync per key (§10), which needs per-key HLC metadata, and a hand-edited TOML file can't carry that safely. `settings.toml` is reserved for device-local bootstrap overrides read before the DB opens (none yet). ARCHITECTURE §3.2 is updated accordingly.
- **`crates/raygun-db`:**
  - sqlx 0.9 SQLite (bundled), WAL, `foreign_keys=ON`, `busy_timeout`. Migrations are embedded with `sqlx::migrate!`.
  - Only runtime-checked queries (`sqlx::query`), so builds never need `DATABASE_URL` or offline query data.
  - `local_meta(key, value)` is device-local and never synced. It holds `device_id` (UUIDv7, created on first open) and `hlc_last`.
  - **Hybrid logical clock:** `u64` = 48-bit physical milliseconds + 16-bit counter. `tick()` for local writes, `observe(remote)` for later sync. It's persisted, so a clock moved backwards never produces older timestamps.
  - `setting(key PK, value JSON, updated_at_hlc, origin_device, deleted)`: the first synced table. It uses a natural key, because setting names are identical on every device. Entity tables will use UUIDv7 ids.
- **Typed settings** (`raygun-model`): `Settings { theme, language }` with defaults. `SettingChange` is a tagged enum (`{ key: "theme", value: "dark" }`) so the UI can only send valid pairs.
  - Loading overlays stored rows on the defaults. An unknown or invalid row is skipped with a warning, never a crash.
- **Commands:** `settings_get() -> Settings`, `settings_update(change) -> Settings`.
- **UI:** Appearance card with a theme picker (System / Light / Dark / AMOLED), applied via `data-theme` on `<html>`.

## Done-check
- Rust tests:
  - Migrations apply, and the device id is stable across reopen.
  - HLC is monotonic and handles clock regression.
  - Settings round-trip, and an invalid row falls back to the default.
  - Data persists after copying the whole data folder to a new location.
- 📱 Set the theme to Light, force-stop, relaunch: it is still Light.
- 🖥 Release build: `raygun-data/raygun.db` is created and the leak check still passes.

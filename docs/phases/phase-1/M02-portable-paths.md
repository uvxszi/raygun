# M02 — `raygun-paths` + portable mode

**Relevant docs:** ARCHITECTURE §3 (portability). reference-notes → nuclear (`profile.rs`, `app_data_dir` usage: what *not* to do).

## Goal
Every file Raygun writes goes under one resolved data root. On desktop that root sits next to the executable, and the build leaves nothing in `%APPDATA%`, `%LOCALAPPDATA%` or `HKCU`.

## Design
- **`crates/raygun-paths`** (no Tauri dependency, so it's fully unit-testable):
  - `Environment { os, exe, raygun_home, appimage, system_dir }`: every input to resolution, gathered by the shell.
  - `resolve(&Environment) -> Resolution`:
    1. `RAYGUN_HOME` set → `Override`.
    2. Android/iOS → app sandbox (`system_dir`) → `Mobile`.
    3. Portable base = folder of the exe. For an AppImage, the folder of `$APPIMAGE`. On macOS, the folder containing `Raygun.app`. The path under `/AppTranslocation/` → `NeedsChoice(Translocated)`.
    4. `<base>/raygun-data` already exists and is writable → `Portable`.
    5. `<system_dir>/installed-mode` marker exists → `Installed`.
    6. `<base>` is writable (probe: create the dir and a temp file) → `Portable`.
    7. Otherwise → `NeedsChoice(Unwritable)`.
  - `choose_installed(system_dir)` writes the marker and returns `Installed` paths.
  - `Paths` accessors: `db`, `settings`, `secrets`, `extensions`, `models`, `bin`, `cache(kind)`, `downloads`, `webview`, `logs`. `ensure_base_dirs` creates `logs/` and `webview/`; owners create the rest lazily.
- **`raygun-app` setup:**
  - Gathers `Environment`. The only allowed call to Tauri's app-dir API lives here, for `system_dir`.
  - Resolves the root. `NeedsChoice` shows a native `rfd` dialog before any window exists: *Use system folders* / *Quit*.
  - Starts `tracing` with a daily rolling file in `logs/` (7 files kept) and stderr in debug builds.
  - Creates the main window from config (`"create": false`) with `data_directory(paths.webview())`.
- **Command** `storage_info() -> StorageInfo { mode, root, logs, webview }`, shown on the home screen.
- **`clippy.toml`** `disallowed-methods` bans Tauri's `PathResolver` app/data/config/cache/log dir methods.
- **`scripts/leak-check.ps1`**:
  - Copies a release exe into `target/leak-check/Raygun/` and runs it until `raygun-data/webview/EBWebView` appears.
  - Snapshots `%APPDATA%`, `%LOCALAPPDATA%` (2 levels) and `HKCU\Software` (2 levels) before and after the run.
  - Fails on any new entry whose name contains `raygun`. Lists other new entries for review.

## Out of scope
Custom-folder picker in the choice dialog, macOS cookie persistence, the in-place updater.

## Done-check
- `cargo test --workspace` covers every resolution branch, including translocation, AppImage, `.app` and unwritable cases.
- 📱 the emulator home screen shows mode `mobile` with a root inside `/data/user/0/app.raygun.player/`.
- 🖥 `scripts/leak-check.ps1` passes against a release build. `raygun-data/{logs,webview}` exist next to the exe.

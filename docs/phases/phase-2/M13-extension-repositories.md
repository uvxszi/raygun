# M13 — Extension repositories, install UI and extension settings

**Relevant docs:** ARCHITECTURE §7.1 (manifest, `index.json` repositories), §5/§10 (sync-ready tables), M11 (host) and M12 (SDK, CLI).

## Goal
The app can:
- add repositories;
- browse their extensions, and install one after seeing what it may access;
- update and uninstall extensions;
- change an extension's settings.

Raygun's own extensions get updates through the same channel, without an app release.

## Design

### Repository format
A repository is a folder served over HTTP(S):
```
index.json
packages/<id>-<version>.zip      (raygun-extension.json + the entry bundle, nothing else needed)
```
```jsonc
// index.json
{
  "formatVersion": 1,
  "name": "Raygun Extensions",
  "extensions": [{
    "id": "dev.raygun.radio-browser", "name": "Radio Browser", "version": "1.0.0", "apiVersion": 1,
    "description": "…", "author": "…", "provides": ["music.metadata"],
    "permissions": { "net": ["*.api.radio-browser.info"] }, "contentRating": "safe",
    "package": "packages/dev.raygun.radio-browser-1.0.0.zip",   // relative to index.json
    "sha256": "…", "size": 4321
  }]
}
```
- **Index URLs:** `https://`, or `http://` to loopback and `10.0.2.2` (the emulator's host). The plain-http hosts are for testing a local repository.
- **Package checks:**
  - The zip must match the index's `sha256` and be at most 10 MiB.
  - Its manifest must validate and agree with the index entry on id, version and permissions. An index can't show one set of permissions and install another.
  - Entry names are checked: no absolute paths, no `..`, no links (zip-slip).
- **Installing:** files are extracted to `extensions/.staging-<id>`, then swapped into `extensions/<id>`, so a failed install leaves the previous version in place.

### Rust
- **`raygun-ext-host`:**
  - A `repo` module: index parsing and validation, and a version comparison that uses semver ordering.
  - A `package` module: verify and extract a package, install it, uninstall an extension.
  - The host's `load` now takes each extension's user settings; `raygun.settings.get` returns them before the manifest defaults.
- **`raygun-db`, migration `0003_extensions.sql`:**
  - `extension_repo (id uuidv7, url unique, name, updated_at_hlc, origin_device, deleted)` is synced-ready. It is seeded with Raygun's official repository: `https://raw.githubusercontent.com/uvxszi/raygun/main/repo/index.json`.
  - `extension_setting (extension_id, key, value json, updated_at_hlc, origin_device, deleted)` is synced-ready.
  - `local_extension_install (extension_id, repo_url, version, sha256, installed_at)` is device-local, because each device installs extensions itself (ARCHITECTURE §10).
- **Commands:**
  - `extension_repos`, `extension_repo_add(url)`, `extension_repo_remove(id)`. Adding a repository fetches and validates its index first.
  - `extension_catalog(refresh)`: every repository's entries, each with a state (`available`, `installed`, `updateAvailable`, `newerInstalled`). Indexes are cached in memory for 10 minutes; a refresh bypasses the cache. One repository failing to load doesn't hide the others.
  - `extension_install(repo_id, extension_id)` covers both install and update. `extension_uninstall(extension_id)` removes an installed extension and its storage; built-in extensions can't be uninstalled.
  - `extension_settings(extension_id)` returns the manifest's setting definitions with their current values. `extension_setting_set(extension_id, key, value)` validates the value's type (and the option, for a select), stores it and reloads.
  - Every change emits `ExtensionsChangedEvent`.

### Tooling (`raygun-ext`)
- `pack [dir] --repo <repoDir>` builds the extension and writes `packages/<id>-<version>.zip` deterministically (fixed timestamps, sorted entries). It then rewrites `index.json`, keeping the newest version of each id.
- The `repo/` folder in this repository is Raygun's official repository: Internet Archive, plus a new **Radio Browser** extension (keyless public API of community internet-radio stations; search returns live streams).

### UI
- **Settings → Extensions:** a summary card with a "Manage" link.
- **`/extensions` page** with three tabs:
  - **Installed:** rows open a detail page.
  - **Browse:** catalog by repository, with Install / Update / Installed buttons. Install opens a confirmation sheet listing the hosts the extension may access.
  - **Repositories:** add by URL, remove.
- **`/extensions/detail?id=` page:** name, version, origin, permissions, providers, a settings form (text, number, toggle, select; each change saves) and Uninstall.

## Done-check
- Rust tests:
  - Index validation and URL resolution.
  - Package verification failures: bad hash, mismatched id, version or permissions, zip-slip names, oversize.
  - Install swap and uninstall.
  - Settings reaching `raygun.settings.get`.
  - Database round trips.
- CLI tests: `pack` is reproducible (byte-identical twice) and the index keeps only the newest version.
- Vitest: browse / install confirmation / update / uninstall / repositories / settings form.
- 📱 On the emulator:
  - Add a local repository (served from this PC through `adb reverse`) and install Radio Browser from it; a search returns stations that play.
  - Change a setting.
  - Publish a newer version, and Update shows up and works.
  - Uninstall.
  - After pushing, the official GitHub repository lists both extensions.

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust:** 113 tests.
  - Index URL rules; index parsing that skips unusable entries; package URL resolution; semver ordering.
  - Package verification rejects a bad hash, a missing manifest or entry, a zip-slip name, mismatched id, version or permissions, and an oversize package.
  - Install, update swap and uninstall (storage included).
  - User settings reach `raygun.settings.get`, and invalid values are dropped.
  - Database round trips: the seeded official repository stays first; removing then re-adding revives the same row; settings upsert; install records.
  - Catalog states, and setting validation in the app.
  - **The committed `repo/` packages pass the app's own verification** (fflate zips read by the `zip` crate, checksums match).
  - Live test, opt-in: `cargo test -p raygun-ext-host -- --ignored radio_browser`.
- **SDK (Vitest):** 18 tests. New: semver ordering, byte-identical re-packs, and an index that keeps only the newest version with the right checksum and size.
- **App (Vitest):** 74 tests. New, in `extensions/Extensions.test.tsx`: Settings summary, installed list, detail page, saving each kind of setting, two-tap uninstall, no uninstall for built-ins, install sheet with permissions, update with an install error, a failed repository, adding and removing repositories, reloads.

| Check (local repository served from this PC, `adb reverse tcp:8765`) | Result |
|---|---|
| Repositories → add `http://localhost:8765/index.json` | Index fetched and validated; the row took the index's name |
| Browse | Internet Archive "Installed" (built-in, same version); Radio Browser "Install". The official GitHub repository showed `HTTP 404`, since it hadn't been pushed yet |
| Install Radio Browser | Sheet listed `*.api.radio-browser.info`; after confirming: `extension installed … version=1.0.0`, files in `extensions/dev.raygun.radio-browser/`, one reload (the dev watcher skipped the app's own change) |
| Search "swiss jazz" | Both providers answered (Internet Archive 8 tracks, Radio Browser 12 stations, ~1.8 s) |
| Tap a station | Native engine playing (Android reported Raygun's AudioTrack as `started`; a live stream has no position, so the session shows −1) |
| Setting "country" = `DE` | Saved and reloaded; a new "jazz" search returned only German stations |
| Published 1.0.1 (drops duplicate streams), Browse → Refresh | "1.0.0 → 1.0.1 · Update"; after updating, version 1.0.1 installed and `DE` kept |
| Uninstall (two taps) | Folder and storage removed; Internet Archive (built-in) unaffected |
| Long error text | A URL in an error first ran off-screen; error messages now use `overflow-wrap: anywhere` (re-checked on the device) |
| After pushing, Browse → Refresh | The official repository (GitHub raw) lists Internet Archive 1.0.0 (Installed) and Radio Browser 1.0.1 (Install); a package downloaded from GitHub matches its checksum |

## Decisions
- **The repository index lists only the newest version** of each extension. Older zips stay in `packages/` so existing links keep working. Rollback is a later feature.
- **Permissions are checked at three points:**
  1. shown before installing;
  2. compared between the index and the package's manifest;
  3. enforced at runtime (M11).
  An update that changes permissions shows its sheet again, like any install.
- **Uninstall deletes the extension's storage.** Settings live in the synced `extension_setting` table and are kept, so a reinstall gets them back.
- **Packages are reproducible** (fixed DOS timestamps from a local-time date, fixed file order), so re-packing unchanged sources leaves git clean.
- **Repository fetches use the app's shared client**, not an extension's client: the index and packages aren't extension network access.
- **Radio Browser skips HLS stations**, because the native engine has no HLS module yet. Its stream URLs are often plain http, which works because the Rust stream proxy fetches them.

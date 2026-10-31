# M12 — Extension SDK and tooling

**Relevant docs:** ARCHITECTURE §7 (extensions), M11 spec (host API, manifest), ROADMAP Phase 2.

## Goal
Extensions are written in TypeScript with imports and full types, and built with one command. During development, an edit reaches the running app (including the Android emulator) within seconds, with no app rebuild or restart.

## Design

### `packages/sdk` (`@raygun/sdk`)
- **Types:**
  - The `raygun` host API (`log`, `http.fetch`, `html.select`, `storage`, `settings`, `providers`), declared as a global.
  - Shared models (`MediaSource`, `OnlineTrack`-shaped `Track`).
  - Provider contracts. `music.metadata` is the only kind so far: `search({ query, limit }) → { tracks }`.
  - The manifest type.
- **Helpers** (bundled into each extension, no runtime of their own):
  - `defineProvider(provider)` and `registerProvider(provider)` type-check a provider against its kind.
  - `fetchJson(url, init)` throws on a non-2xx status and returns parsed JSON.
- **Kept in step with the app:** a type-level test asserts the SDK's `MediaSource` and track shape are assignable to the generated `@raygun/bindings` types, so a Rust model change that breaks extensions fails `tsc`.

### CLI `raygun-ext` (in `@raygun/sdk`, plain Node ESM, esbuild)
- **`init <dir> --id <id> --name <name>`:** copies the template (manifest, `src/index.ts`, `package.json`, `tsconfig.json`) and fills in the id and name.
- **`build [dir]`:**
  - Bundles `src/index.ts` into the manifest's `entry` (normally `dist/index.js`). The bundle is a single IIFE script targeting ES2022, which QuickJS supports, with no external imports.
  - Validates the manifest's basic fields first: the id format, `apiVersion: 1`, `runtime: "js"`, and an entry that ends in `.js`.
- **`dev [dir]`:** rebuilds on every change and installs the result into a running app.
  - `--into <extensions folder>` writes `<folder>/<id>/` (desktop data root, later).
  - `--android [--serial <device>]` pushes into the app sandbox with `adb push` followed by `run-as app.raygun.player cp`. adb is run with stdin closed, because an open stdin makes `adb shell` hang (see CLAUDE.md).

### Hot reload in the app
- In debug builds, `ExtensionService` checks the `extensions/` folder (ignoring `.data`) once a second, comparing a fingerprint of file paths, sizes and modification times. When the fingerprint changes, it reloads every extension.
- **`ExtensionsChangedEvent`** is emitted after every reload. The UI invalidates its extension list and search results.
- **`extensions_reload` command:** Settings → Extensions gets a Reload button. This also helps in release builds, after copying an extension in by hand.
- Release builds don't poll.

### First-party extensions use the SDK
- `extensions/internet-archive` becomes `src/index.ts` built into `dist/index.js`.
- `dist/` is **committed**, so `cargo build` never needs Node. A Vitest test rebuilds every first-party extension in memory and fails when a committed bundle is stale (like the bindings check). The fix is `pnpm build:extensions`.

## Done-check
- `pnpm build:extensions` rebuilds the bundles; the staleness test passes.
- CLI tests (Vitest, in a temp folder):
  - `init` produces a project that `build`s.
  - `build` rejects a bad manifest.
  - `dev --into` writes the installed copy.
- The Rust live test still passes against the bundled TS build.
- `tsc` covers the SDK, the template and the Internet Archive extension.
- 📱 With `raygun-ext dev --android` running, editing the Internet Archive extension (e.g. its provider name) shows up in the app (Search group heading, Settings) without restarting the app. Reload in Settings works.

## Verification (2026-10-31, Pixel_4 emulator)
- **SDK (Vitest):** 16 tests.
  - The committed first-party bundle matches a fresh build.
  - Manifest checks pass for a valid manifest and reject 8 bad fields.
  - `init` produces a project that builds into one plain script (no `import`, `export` or `require`); `init` refuses a non-empty folder.
  - Manifest problems and build errors are reported.
  - `dev --into` installs every save and leaves no staging copy behind.
  - Type-level checks keep `MediaSource` and `Track` assignable to the generated app models.
- **Rust:** 105 tests, including the extensions-folder fingerprint (storage and staging writes don't trigger reloads) and a check that the built-in bundles parse. The live Internet Archive test passes against the TS build.
- **App (Vitest):** 66 tests. New: Reload in Settings, and following `ExtensionsChangedEvent`. On the emulator, tapping Reload logged a fresh `extensions ready`.

| Check | Result |
|---|---|
| `pnpm dev:android` in `extensions/internet-archive` | First build installed into the sandbox; the app logged `installed extensions changed, reloading` in the same second |
| Edited the provider name in `src/index.ts`, then the manifest name | Both reinstalled automatically; with no app restart, Settings showed "Internet Archive (hot reloaded)" and the Search group heading "Internet Archive (live edit)" |
| Search speed in a debug build | Roughly 20 s for "blues" before (timed from screenshots), 1.6 s after optimizing QuickJS in dev builds; "ragtime piano" answered 27 tracks in 0.8 s (from the new `search answered … elapsed_ms` log line) |

## Decisions
- **The SDK and template are MIT**, not AGPL like the app. Their helpers are bundled into every extension, so a copyleft SDK would force a license on third-party extensions. The host API boundary keeps the app itself AGPL.
- **`dist/` is committed for first-party extensions** (with a `.gitignore` exception), so Rust builds don't need Node; the staleness test guards it.
- **Polling instead of OS file watching** for hot reload: one code path that also sees files pushed over adb, cheap (once a second, debug builds only), and no new dependency.
- **QuickJS is compiled with `opt-level = 3` in dev builds too.** Unoptimized, the interpreter made debug-build searches several times slower. Release builds were never affected.
- **No `URL`, `fetch`, `setTimeout` or `TextEncoder` in the runtime.** The template says so. Shims come only when an extension needs one.

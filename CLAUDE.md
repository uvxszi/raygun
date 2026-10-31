# Raygun

All-in-one multimedia app (music, video, movies/TV, anime, manga, light novels) built on Tauri v2 (Rust + React/TS). License AGPL-3.0-only. Identifier `app.raygun.player`.

## Read first

- `docs/ROADMAP.md`: phases and milestones; work happens one milestone at a time.
- `docs/phases/phase-N/MNN-*.md`: the spec for the milestone in progress. Write one before starting a milestone.
- `docs/ARCHITECTURE.md`: read only the sections the milestone spec lists.
- `docs/reference-notes.md`: findings from the reference repos in `reference/` (git-ignored). Check here before re-exploring a repo.

## Layout

- `apps/raygun/`: React UI (`src/`) and the thin Tauri shell (`src-tauri/`, plus `gen/android`).
- `crates/raygun-app`: service wiring + Tauri commands. `crates/raygun-model`: types shared with TS.
- `packages/bindings`: TS generated from Rust by tauri-specta. Never edit it; run `pnpm bindings`.
- `plugins-mobile/raygun-audio`: native Android audio engine (Media3 in a `MediaSessionService`; reports reach Rust over JNI). The app shell must depend on each mobile plugin directly, or Tauri won't include its Android module.

## Commands

```bash
pnpm install
pnpm dev                 # desktop
pnpm android:dev         # Android emulator (needs ANDROID_HOME and NDK_HOME)
pnpm lint && pnpm type-check && pnpm test
cargo test --workspace   # includes a check that bindings are current
cargo clippy --workspace --all-targets -- -D warnings
pnpm format && cargo fmt --all
pnpm bindings            # regenerate TS bindings after changing commands/types
```

## Android tooling gotchas (for agents)

- Run `tauri android dev|build` in a real terminal tab (Terminal panel), not a headless shell. Without a TTY, the CLI's device prompt loops forever (it once wrote a 13 GB log), and `adb shell` blocks on an open stdin pipe.
- Only one Tauri Android command can run at a time. A stuck one shows `Blocking waiting for file lock on Android` in the next. Find leftover `tauri.js android ...` node processes and stop them.
- Verify on the emulator with `adb install -r .../app-universal-debug.apk`, `adb shell am force-stop app.raygun.player`, a relaunch, then `adb exec-out screencap -p`. App logs: `adb shell "run-as app.raygun.player cat logs/<file>"`.
- Env for headless runs: `ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk`, `NDK_HOME=...\Sdk\ndk\28.2.13676358`.

## Rules

- Domain state and logic live in Rust services. The UI calls generated `commands.*` and holds UI-only state.
- Platform priority: Android only for now (`Pixel_4` emulator). Don't build or verify desktop until the user asks; keep the code cross-platform.
- Desktop builds must be portable: never call OS app-dir APIs or `tauri-plugin-store`. Go through `raygun-paths` (M02).
- Synced tables use UUIDv7 ids, HLC timestamps and tombstones (ARCHITECTURE §5, §10).
- Adapted code from a reference repo keeps a header naming the source repo and its license.
- TS: `type` over `interface`, `const Component: FC<Props>`, no one-letter names. Tests use RTL queries and userEvent, and mock only the bindings/IPC layer.

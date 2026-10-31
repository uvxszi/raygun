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

## Rules
- Domain state and logic live in Rust services. The UI calls generated `commands.*` and holds UI-only state.
- Platform priority: Android first (`Pixel_4` emulator), then Windows. Verify UI work on Android.
- Desktop builds must be portable: never call OS app-dir APIs or `tauri-plugin-store`. Go through `raygun-paths` (M02).
- Synced tables use UUIDv7 ids, HLC timestamps and tombstones (ARCHITECTURE §5, §10).
- Adapted code from a reference repo keeps a header naming the source repo and its license.
- TS: `type` over `interface`, `const Component: FC<Props>`, no one-letter names. Tests use RTL queries and userEvent, and mock only the bindings/IPC layer.

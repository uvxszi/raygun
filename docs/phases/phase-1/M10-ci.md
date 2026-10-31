# M10 — Continuous integration

**Relevant docs:** ROADMAP (platform order: Android only for now), CLAUDE.md (commands).

## Goal
Every push to `main` and every pull request runs the same checks used locally, and builds an installable Android APK as a downloadable artifact.

## Design
- **`.github/workflows/ci.yml`**, two jobs on `ubuntu-24.04`, run concurrently. A newer push cancels a running workflow on the same ref.
  - **`checks`:**
    - Installs the Linux webview/GTK packages Tauri needs to compile.
    - Runs pnpm install (frozen lockfile), Prettier check, ESLint, `tsc`, Vitest, `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test --workspace`. The last includes the check that the TS bindings are up to date.
  - **`android`:**
    - Temurin JDK 21 and the Android SDK (`android-actions/setup-android`), with platform 37 and NDK 28.2.13676358, the versions the project builds with locally.
    - Rust targets `aarch64-linux-android` (phones) and `x86_64-linux-android` (emulators).
    - `pnpm android:build --debug --apk --target aarch64 --target x86_64` builds one universal debug APK, uploaded as `raygun-android-debug-<sha>`.
- **Caching:** pnpm store (setup-node) and Cargo (`Swatinem/rust-cache`, separate keys per job).
- **Debug builds only:** a signed release APK needs a keystore stored as a repository secret. That is a separate decision.
- **Desktop builds** (Windows portable zip, Linux AppImage, macOS) and the Windows leak check join when desktop work resumes.

## Done-check
- The workflow parses, and every command in it passes locally (it mirrors the local check suite).
- `gradlew` is executable in the git index (Linux runners need the bit).
- After the repository is pushed to GitHub: both jobs pass and the APK artifact downloads and installs on the emulator.

## Local verification (2026-10-31)
- The workflow parses (2 jobs; every step has exactly one `uses` or `run`).
- Every `checks` command passes locally: Prettier, ESLint, `tsc`, 58 Vitest tests, rustfmt, clippy `-D warnings`, 88 Rust tests.
- The `android` job's build command (`--target aarch64 --target x86_64`) produces one universal APK with both `arm64-v8a` and `x86_64` libraries. It installed and started on the emulator.
- `gradlew` committed as executable (`100755`).
- Dependencies are now built without debug info in the dev profile: `libraygun_lib.so` went from ~280 MB to ~123 MB per architecture, and a freshly written debug APK from ~570 MB to ~256 MB. Raygun's own crates keep full debug info.
- **Not yet run on GitHub:** the repository has no remote. The first push will show whether the runner setup (SDK packages, NDK path) needs adjusting.

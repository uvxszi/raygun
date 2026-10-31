# Raygun

An all-in-one, cross-platform multimedia app for music, video, movies, TV, anime, manga, light novels and local files. It has an extension ecosystem, AI enhancement (manga upscaling and translation), automatic LAN sync between your devices, and a built-in MCP server so AI agents can control it.

Built with Tauri v2 (Rust + React/TypeScript) for Android, Windows, Linux, macOS and iOS. Desktop builds are fully portable.

**Status:** early development, Phase 1. See [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Development

Requirements: Node 24+, pnpm 12, Rust 1.95+, and for Android: Android SDK + NDK and a JDK. Set `ANDROID_HOME` and `NDK_HOME`.

```bash
pnpm install
pnpm dev            # desktop window
pnpm android:dev    # running emulator or connected device
pnpm test && cargo test --workspace
```

## License

[AGPL-3.0-only](LICENSE).

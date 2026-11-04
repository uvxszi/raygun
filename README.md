# Raygun

Raygun is a media app for music, podcasts, audiobooks, films and series, comics and books. It plays and reads local files and content from sources provided by extensions, Stremio addons and other supported add-on formats. It is built with Tauri v2: a Rust core with a React and TypeScript interface.

## Features

**Music**

- Search across every installed source, with songs, artists, albums and playlists, and results ranked across sources.
- A queue with shuffle, repeat, play next, crossfade and a sleep timer; a 10-band equalizer, loudness normalization, and per-song trims and volume.
- A library of favorites, playlists (sortable, with drag-to-reorder), followed artists, saved albums and local folders.
- Downloads for offline playback and export as files (MP3, FLAC, WAV), with retries and fallback to other sources.
- Discovery: charts from chart extensions, genres, moods, decades and generated mixes.
- Synced and plain lyrics from lyrics extensions, an editor for words and timings, and on-device transcription.
- Import from playlist links, CSV exports, pasted lists or screenshots of song lists.

**Podcasts and audiobooks**

- RSS and Atom feeds, podcast directories from extensions, episode downloads and resume.
- Public-domain audiobooks from LibriVox and Loyal Books, with chapters in order.
- Captions from feed transcripts or speech-to-text (Whisper on the device, or an OpenAI-compatible service).

**Video**

- Stremio addons (catalogs, streams, subtitles), video-source extensions and public-domain films from the Internet Archive.
- A player with HLS and DASH, subtitle search, styling and delay, chapter marks, intro skipping, picture-in-picture and gestures.
- Playback sessions that resume and move to the next episode, fall back to other links when one fails, and remember the last source per series.
- Downloads, Continue watching, and trailers on title pages.

**Comics and books**

- Manga sources (Mihon-style catalogs with filters), novel sources, local CBZ/ZIP and EPUB files.
- A paged and webtoon comic reader and a reflowing novel reader with read-aloud.
- Library categories, status lists, filters and sorting, library updates and chapter downloads.
- Export to CBZ with ComicInfo, PDF, EPUB or plain text.

**Across the app**

- A library per medium plus an All view, Home rows for what's in progress, recently played and popular, and a configurable layout (tabs, sections and their order).
- Device linking: linked devices sync settings, favorites, playlists and history over the local network, and either can play on the other. A web remote works from any browser on the network.
- An MCP server (off by default) that lets AI assistants search, play and manage the queue.
- Websites in a built-in browser kept apart from the app (its own process on Android, its own profile on Windows), with an ad, tracker, popup and redirect blocker.
- A content filter for 18+ material, a separate switch for explicit music, and interface translations.
- AI settings for on-device speech models and online providers (OpenAI-compatible endpoints, Claude, DeepL), with API keys kept in an encrypted store.
- Portable desktop builds that keep all their data in one folder beside the executable.

## Platforms

- **Android** 7.0 or later (API 24; targets API 37). Android is the primary platform.
- **Windows** 10 and 11 (x64), as an installer, an MSI or a portable executable. Requires Microsoft Edge WebView2.

The code is kept cross-platform, but Linux, macOS and iOS builds are not produced or tested yet.

## Extensions and repositories

Raygun is a client: it runs the add-ons people install. The sources it ships and suggests serve only public-domain or openly licensed content.

- **First-party extensions** are built into the app from `extensions/`: Internet Archive (public-domain films and openly licensed audio), Internet Archive comics, LibriVox, Loyal Books, MusicBrainz (metadata only), Pepper&Carrot, Project Gutenberg and xkcd.
- **Repositories** are managed under Browse → ⋯ → Repositories. [Paraline](https://github.com/uvxszi/paraline-raygun), Raygun's own repository of public-domain and openly licensed sources, is added by default and is the only suggestion. Any other repository is added by its address: Raygun reads Raygun indexes, Shosetsu repositories (Lua novel sources), Miru repositories and the Nuclear plugin registry (plugins are converted at install). Stremio addons are added by address too; OpenSubtitles v3 comes preinstalled for subtitles.
- **Writing extensions:** extensions are TypeScript on `@raygun/sdk` (`packages/sdk`), built and tested with its `raygun-ext` CLI (`init`, `build`, `dev --android`). Debug builds of the app hot-reload extensions while they are being developed.
- Raygun does not solve captchas or other bot checks for sources.

## Building from source

Requirements:

- Node.js 24 or later and pnpm 12
- Rust 1.95 or later
- For Android: the Android SDK (platform 37), NDK 28.2.13676358 and JDK 21, with `ANDROID_HOME` and `NDK_HOME` set

```bash
pnpm install
pnpm dev                 # desktop window
pnpm android:dev         # emulator or connected device
pnpm lint && pnpm type-check && pnpm test
cargo test --workspace   # includes a check that the TypeScript bindings are current
cargo clippy --workspace --all-targets -- -D warnings
pnpm bindings            # regenerate the TypeScript bindings after changing commands or shared types
pnpm build:extensions    # rebuild the first-party extensions' bundles
```

Continuous integration (`.github/workflows/ci.yml`) runs these checks and builds a debug Android APK. Tags matching `v*` build the Windows packages and signed Android APKs (`.github/workflows/release.yml`), with release notes from `docs/releases/<tag>.md`.

## Repository layout

| Path                           | Contents                                                                                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/raygun/`                 | The React interface (`src/`) and the Tauri shell (`src-tauri/`, with the Android project in `gen/android`)                                             |
| `crates/`                      | The Rust core: services and commands (`raygun-app`), database, playback, resolver, extension host, networking, MCP server, LAN remote, device sync, AI |
| `plugins-mobile/raygun-audio/` | The native Android plugin: Media3 audio and video playback, storage, speech                                                                            |
| `packages/`                    | `@raygun/sdk`, the Nuclear plugin converter, and TypeScript bindings generated from Rust                                                               |
| `extensions/`                  | First-party extensions                                                                                                                                 |
| `repo/`                        | Raygun's extension repository, generated by `raygun-ext pack`                                                                                          |
| `docs/`                        | Roadmap, architecture, milestone specs and research notes                                                                                              |

## Documentation

- [docs/ROADMAP.md](docs/ROADMAP.md): phases and milestones
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): how the system is put together
- `docs/phases/`: one spec per milestone, with its design decisions and verification
- [docs/feature-parity.md](docs/feature-parity.md): features of the reference apps and where Raygun stands
- [docs/TRANSLATING.md](docs/TRANSLATING.md): adding and updating translations

## License

Raygun is licensed under the [GNU Affero General Public License v3.0 only](LICENSE). The extension SDK (`packages/sdk`) is MIT-licensed. Third-party material is listed in [NOTICE](NOTICE) and in the headers of adapted source files.

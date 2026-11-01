# M36 — Browsing extensions across repositories

**Relevant docs:** M13 (repository format, install UI), M17 (Nuclear plugins), M32 (Shosetsu repositories), M35 (content filter), `docs/feature-parity.md` (Extensions and repositories).

## Goal
- **Several repositories:** add extensions from Raygun's official repository and from others (Shosetsu's, Nuclear's plugin registry, and any third party's) without looking up URLs.
- **One browsable catalog** across all of them.
- **Essentials:** the official repository carries the basic first-party extensions.

## Design
- **Suggested repositories** (`ExtensionService::suggested_repos`, command `extension_suggested_repos`):
  - **The list:** a built-in list of Raygun Extensions, Shosetsu extensions and Nuclear plugins, with a description, each marked `added` when the device has a repository at that URL (case and a trailing slash ignored).
  - **Repositories tab:** offers the ones not yet added with one tap, plus a note that Raygun doesn't review third-party extensions.
  - **Later:** Stremio addons and LNReader plugins join the list when their milestones land (M39, M51).
- **Catalog filters** (`extensions/catalogFilters.ts`, client-side over the loaded catalog):
  - search over name and description;
  - kind chips (All, Music, Manga, Novels, Video, Podcasts, Lyrics, Other, from each entry's `provides`);
  - a language picker;
  - a repository picker.
  - Pickers appear only when there is something to choose, and repositories the filter empties are hidden.
- **Language per entry:**
  - `RepoEntry.lang` (optional in `index.json`, `lang` in the catalog) is filled from Shosetsu scripts' `lang`.
  - Entries show it by its own name ("multi" is "Many languages").
- **Essentials in the official repository:** `pnpm repo:publish` now also packs Project Gutenberg 1.0.0 and MangaDex 1.1.0. The official index lists Internet Archive, LRCLIB, MusicBrainz, Radio Browser, YouTube, Project Gutenberg and MangaDex.
  - **Later additions:** the podcast directory (M37) and public-domain films (M40).
- **NSFW:** `nsfw` entries are already left out of the catalog while the filter is off (M35).

## Out of scope
- Extension icons in the catalog (Shosetsu's `imageURL`): images from arbitrary hosts need the image proxy route; later.
- Ratings or download counts (no repository publishes them).

## Done-check
- **Rust:** suggested-repository URL matching.
- **Vitest:**
  - filters by text, kind, language and repository, with "No extensions match";
  - a suggested repository is added with one tap and an added one shows "Added".
- 📱 **On the emulator:**
  - Repositories shows the suggestions with the official one added; adding Shosetsu works;
  - Browse filters the combined catalog: Novels, then a language, then search.

# M35 — Content filter (NSFW) and languages

**Relevant docs:** ROADMAP re-plan (2026-11-01), `docs/feature-parity.md` (App-wide, Extensions), M13 (repositories), M25 (manga sources), M04 (i18n).

## Goal
- **NSFW toggle:** a switch in Settings, off by default, that hides NSFW content everywhere: extensions, sources and titles.
- **Languages:** a language picker, with the structure set up so contributors can translate easily. It starts from the reference apps' existing translations, and nothing is machine-translated.

## Design

### Content filter
- **The setting:** `showNsfw` (synced like other settings) is turned on and off in a new Settings → Content card.
- **The shared switch:** a `ContentFilter` in `raygun-ext-host`, shared by the host thread, every extension and the app.
  - It is set at startup from the stored setting, before extensions load, and whenever the setting changes. A change also emits `ExtensionsChangedEvent`, and the UI refreshes all queries.
- **Extensions:**
  - `ExtensionHost::providers` leaves out every provider of an extension rated `contentRating: "nsfw"` while the filter is off. Search, sources and streaming all use it.
  - Installed NSFW extensions stay in the Installed list, marked "18+ · Hidden while NSFW content is off", so they can be removed.
  - The repository Browse list leaves out `nsfw` entries entirely.
- **Extensions filter their own results:**
  - The new host call `raygun.content.showNsfw()` (SDK `RaygunApi.content`) tells an extension whether to include adult results.
  - Listings may mark items `nsfw: true`, and the app drops them while the filter is off.
  - MangaDex 1.1.0 leaves out `erotica`/`pornographic` titles while the filter is off, marks them `nsfw`, and adds an "Erotica"/"Pornographic" genre to their details.
- **Titles the app already holds:**
  - A title whose genres include an adult genre (adult, hentai, smut, erotica, pornographic, 18+, R18, NSFW, mature, …) is hidden from the manga and novel libraries.
  - Opening such a title shows a notice with a link to Settings instead of its page (`MangaTitleView.hiddenNsfw`).

### Languages
- **Locale files:** `apps/raygun/src/i18n/locales/<tag>.json`, nested i18next JSON.
  - English is bundled. The others load when chosen (`import.meta.glob`), and missing keys fall back to English.
- **The setting:** `language` now defaults to `system`, which follows the device's preferred languages (exact tag, then the same base language, then English).
  - Settings → Appearance → Language lists "System default (…)" and every available language by its own name (`Intl.DisplayNames`).
- **Translations from the reference apps** (`pnpm i18n:import`, `scripts/i18n-import.mjs`):
  - **Sources:** Nuclear (i18next JSON), Mihon and aniyomi (moko-resources), CloudStream, Shosetsu and RiPlay (Android resources).
  - **Matching rule:** a key gets a translation only when a source's English text is exactly Raygun's, after normalising Android escapes and `...`/`…`.
    - Placeholders are mapped by position (`%1$s` → `{{name}}`), and plural groups by category.
    - A string containing markup is skipped.
  - **Merging:** existing keys are never overwritten, so hand-written translations survive a re-import.
  - **Small locales:** a locale with fewer than 40 matching strings isn't created.
  - **Credits:** `SOURCES.md` lists where every locale's strings came from, and `NOTICE` credits the Apache-2.0 sources.
- **Contributors:** `docs/TRANSLATING.md` explains editing, plurals, adding a locale and the import.

## Out of scope
- **Right-to-left layout:** text is translated, but the layout stays left-to-right until an RTL locale is well along (noted in the guide).
- **Text from extensions** (source names, setting labels).
- **Filters Mihon has that this milestone doesn't:** a three-way "safe / mixed / 18+" filter. The single switch covers it.

## Done-check
- **Rust:**
  - with the filter off, an `nsfw` extension's providers are hidden, and its host call reports `false`; with it on, both providers appear and the call reports `true`;
  - adult-genre detection.
- **Vitest:**
  - every locale file has only English keys with English placeholders;
  - language matching and "system";
  - the language picker saves the choice;
  - the NSFW switch is off by default and saves;
  - the adult-title notice.
- 📱 **On the emulator:**
  - Settings shows the Content card (off) and the language picker;
  - choosing Deutsch turns the interface German where translated; System default goes back;
  - MangaDex browsing works with the filter off and on.

## Decisions
- **One switch rather than Mihon's three levels.** `suggestive` extensions (most novel sources, MangaDex) stay visible and filter their own results.
- **Hide, don't uninstall:** installed NSFW extensions stay listed (marked) so they can be removed, but provide nothing while the filter is off. This matches Mihon's "also apply to installed extensions".
- **Genre matching is a backstop:** sources that know better mark listings `nsfw` themselves. Genres are compared whole and in lower case, so "Adult Life" and "Ecchi" aren't hidden, but "Mature" is, because novel sites use it for explicit content.
- **Exact-match imports only:** short, context-free strings ("Cancel", "Library", "Every 6 hours") match well. Sentences specific to Raygun stay English until someone translates them. About 25–30% of strings came through for the major languages (German 126 of 415, Spanish 132, Russian 133), with 80 languages over the 40-string bar.
- **`zh` is `zh-CN` and `no` is `nb`.** CloudStream's private-use `qt` locale is dropped.
- **Language names:** Android's WebView has no native names for some languages (it echoed "be", "as", "ba"). The picker falls back to the English name, then the tag.

## Verification (2026-11-01, Pixel_4 emulator, fresh app data)
- **Checks:** all pass.
  - Rust: `cargo test --workspace` (new host and genre tests) and clippy.
  - Front end: `pnpm lint`, `pnpm type-check` and `pnpm test` (218 app tests, including 84 i18n tests over all 80 locale files).
- **Settings:** shows Appearance → Language ("System default (English)") and the new Content card with "Show NSFW (18+) content" off.
- **Language picker:** the list shows every language by its own name.
  - Choosing Deutsch switched the translated strings at once: Einstellungen, Erscheinungsbild, Thema, Hell/Dunkel, Sprache, Alle 12 Stunden, Startseite/Bibliothek/Suchen.
  - Choosing System default went back to English.
- **NSFW switch:** turns on and off and stays as set.
- **MangaDex:** Popular loads normally with the filter off (the extension calls `raygun.content.showNsfw()` on every list).

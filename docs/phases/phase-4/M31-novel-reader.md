# M31 — Novel sources and novel reader

**Relevant docs:** ARCHITECTURE §5 (episodic library), M24 (manga foundation), M27 (reader), M28 (updates), reference-notes (shosetsu: extension contract, `getPassage`).

## Goal
- Light novels and web novels read in Raygun as manga does: browse a source, add titles to the library, and read chapters with progress kept.
- The reader is made for text: reflowing paragraphs, adjustable typography, scrolling or paginated reading, and progress stored as a character offset.

## Design

### `novel.source` capability
- **Calls:** the catalogue calls of `manga.source` (`popular`, `latest`, `search`, `details`, `chapters`), with the same shapes and the same checks in the host. Pages are replaced by:
  `passage({ chapterId }) → { html?: string, text?: string, baseUrl?: string }` (Shosetsu's `getPassage`).
- **The host never passes source HTML to the webview.** `raygun-ext-host::novel` turns a passage into blocks: `paragraph { text }`, `heading { text }`, `separator`, `image { url }`.
  - **Dropped:** scripts, styles, frames, forms and SVG, with their contents.
  - **Breaks:** block elements and `<br>` end a paragraph; whitespace collapses.
  - **Images:** relative URLs resolve against `baseUrl`; only http(s) images are kept.
  - **Plain text:** paragraphs split at blank lines.
  - **Limits:** 20,000 blocks, 2 MB of text.
- **SDK:** `NovelSourceProvider` and `Passage` types in `@raygun/sdk`.

### First-party source: Project Gutenberg (`extensions/gutenberg`)
- Public-domain books.
  - **Catalogue:** gutenberg.org's own OPDS feeds for popular (by downloads), latest (by release date), search and details (authors, subject headings as genres, summary, cover from the standard cover path).
  - **Chapters:** the book's HTML from gutenberg.org, cut between the "START/END OF THE PROJECT GUTENBERG EBOOK" markers and split at `<h2>` headings (`<h3>` when a book has fewer than two `<h2>`). A contents section is dropped.
  - Chapter ids are `<book>/<index>`. A passage is the section's HTML, with page-number spans removed and images made absolute.
- Shosetsu's community sources arrive with M32.

### Storage and services
- Titles and chapters use the episodic library with `kind = 'novel'`. Progress `position` is the character offset where reading stands, `total` is the chapter's length, and reaching the end completes it.
- The catalogue code of `MangaService` moves to a shared `Catalog` (sources, browse, title and refresh, library, read state, progress, reading order), used by `MangaService` and the new `NovelService`.
- **Commands:** `novel_sources`, `novel_browse`, `novel_title`, `novel_set_in_library`, `novel_library`, `novel_chapter` (blocks, with images through the image proxy and neighbouring chapters), `novel_set_progress`, `novel_set_read`.
- **Reader preferences** are one synced setting, `novelReader`:
  - font size 14–30 px;
  - line spacing 1.2–2.2;
  - font serif or sans;
  - width narrow or wide;
  - mode scroll or paged.

### UI
- **Library:** Music | Manga | Novels. The Novels view mirrors manga (grid with unread counts, Browse sources).
- **Shared pages:** the manga source and title pages are shared, by a `medium`, at `/novel/browse`, `/novel/source` and `/novel/title`. Download controls are manga-only for now, and partly read novel chapters show a percentage.
- **Reader** (`/novel/read`):
  - **Header:** title and chapter, plus an **Aa** panel for the preferences.
  - **Scroll mode:** one column. Progress is the offset of the first paragraph on screen, saved while reading.
  - **Paged mode:** CSS columns one screen wide. Taps on the left or right third or swipes turn pages (arrow keys too), and the middle third shows or hides controls. The footer shows the page number and a percentage.
  - **Opening:** a chapter reopens at its saved offset.
  - **End of chapter:** the end completes the chapter and offers the next one. The next chapter's text is fetched ahead.

## Out of scope
- Shosetsu compatibility (M32), EPUB import, chapter downloads and TTS (M33).
- Library updates for novels (the M28 job stays manga-only for now). Translation (Phase 5).

## Done-check
- **Rust:**
  - passages become blocks (scripts and styles dropped, `<br>` and blocks split, headings, images resolved, plain text);
  - the Gutenberg fixture flow (catalogue, chapter split, passage) runs in the host against recorded responses;
  - novel titles store and read as manga do.
- **Vitest:**
  - the Novels library view;
  - the shared title page in novel mode (no download controls, percent progress);
  - the reader renders blocks, restores the saved offset, saves progress, applies preferences, and offers the next chapter at the end.
- 📱 **On the emulator:**
  - browse Gutenberg and open a book;
  - add it to the library;
  - read a chapter in scroll mode and in paged mode;
  - change the font size;
  - leave and come back to the same place;
  - finish a chapter and go to the next.

## Decisions
- **Gutenberg over OPDS, not gutendex:** gutendex.com (a community JSON mirror) answered lists in about 0.3 s, but its search timed out after 40 s and details sometimes stalled. Gutenberg's own OPDS feeds answer in about 0.5 s from one host.
- **Book cache:** the extension keeps the last book's sections in memory, so the chapter list, passages and next chapters download the book once.
- **Manga and novels share the catalogue:** `Catalog` in Rust, a `medium` prop and command map in the UI. The model types keep their `Manga*` names; renaming them would touch every manga call for no behaviour change.
- **Progress is saved and mirrored into the cached chapter:** reopening a chapter within the cache window resumes at the right place. A pending save is flushed when the chapter closes.
- **Numeric search values:** route search values that parse as numbers (book ids such as `84`) are kept as text.

## Verification (2026-10-31, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace` ran 240 tests (the Gutenberg fixture flow among them), and the live `--ignored gutenberg` run read the real Frankenstein. Clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (120 app tests, 5 new for novels) are clean.
- **Browsing:** Library → Novels → Browse sources → Project Gutenberg listed popular books with covers.
- **Title pages:** Frankenstein has 28 chapters, starting with Letter 1. Pride and Prejudice has 63 chapters, with authors, subjects and summary, and opened about 2 s after the tap.
- **Library:** added the book; the Novels library shows it with 63 unread.
- **Scroll reading:** serif text with headings and illustrations through the image proxy. Progress read 68% deep in a chapter. Leaving and pressing Continue reopened Section 1 at its saved paragraph, and the title page showed "25% read".
- **Text settings:** the font went to 22 px and the same paragraph stayed at the top (30% before and after a change). Switching to Pages opened at page 5 of 44, where the saved offset was.
- **Paged reading:** taps on the right third and left swipes turned pages, and text flowed across them.
- **End of chapter:** the last page showed the end card, and the next tap completed the chapter and opened the next one (Section 2, with its illustration).
- **Next chapter:** loading the following chapter took about 2 s.

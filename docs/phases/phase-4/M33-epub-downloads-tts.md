# M33 — EPUB import, novel downloads, text-to-speech

**Relevant docs:** M28 (download queue, `/local` proxy route), M30 (file import through the file chooser), M31 (novel reader, `PassageBlock`), M06 (the native Android audio plugin).

## Goal
- Novels from files and offline reading: an EPUB from the device becomes a library novel, and online novel chapters can be downloaded like manga chapters.
- The novel reader can read aloud with the phone's voices.

## Design

### EPUB import (`raygun-app::epub`, `NovelService`)
- **Parsing** (`zip` + `quick-xml`):
  - `META-INF/container.xml` → the OPF package.
  - **OPF metadata:** `dc:title`, `dc:creator` (authors), `dc:description`, `dc:subject` (genres) and the cover (EPUB 3 `properties="cover-image"`, or EPUB 2 `<meta name="cover">`).
  - **Spine:** the `manifest` items in reading order.
  - **Chapter names** come from the table of contents (EPUB 3 `nav` or EPUB 2 `toc.ncx`), else the chapter's first heading, else "Chapter N".
  - **Limits:** at most 100 MB unpacked, at most 5,000 entries, and only paths inside the book.
- **Storage:**
  - The book is unpacked to `downloads/books/<sha256>/`. Every path segment is rewritten to `[A-Za-z0-9._-]` (`_` for anything else, `x` before a leading dot), so the proxy's `/local` route can serve its images.
  - The title is a `novel` with `source_id = "local.epub"` and `remote_id = <sha256>`, added to the library at once. Units are spine items (`remote_id` = their path in the book).
  - Importing the same file again keeps the title and its progress.
- **Covers and images:**
  - Stored covers are `local:books/<hash>/<path>` and become proxy URLs whenever they're shown. `Catalog` turns `local:` covers into `/local/` links.
  - Chapter images resolve against the chapter's path in the book, then map to the unpacked file.
- **Reading:** `novel_chapter` on a `local.epub` title reads the unpacked XHTML through the same `html_blocks` converter. Refresh does nothing for local titles.
- **Importing:** `novel_import_epub(contents: base64)` returns the title id. The Novels library gets an "Import EPUB" button using the webview's file chooser, as backups do.

### Novel chapter downloads (`MangaDownloadService`, now any chapter)
- **Worker:** novel chapters go through the same queue (`local_manga_download`) and worker.
  - For a `novel` title it fetches the passage and writes its blocks as JSON to `downloads/novel/<title>/<chapter>/passage.json`, through a `.part` folder renamed when complete. Image URLs stay remote.
  - Manga keeps `downloads/manga/…`; cancel and delete remove whichever folder exists.
- **Reading offline:** `novel_chapter` reads a finished download first, so a downloaded chapter reads offline.
- **UI:** the title page's download buttons and "Download unread" work for novels. The Novels library links to the shared Downloads page.

### Text-to-speech (Android `TextToSpeech` in the `raygun-audio` plugin)
- **Kotlin:**
  - `ttsSpeak({ items: [{ id, text }], rate })` flushes and queues each paragraph as its own utterance.
  - `ttsStop()`.
  - `ttsAvailable()` reports whether an engine and voice are ready.
  - Progress (`start` with the paragraph id, `done`, `error`) goes to Rust over JNI (`NativeBridge.onTts`), like engine reports.
- **Rust:** `TtsService` forwards commands and emits `TtsEvent { state, paragraph }`. Off Android it reports unavailable.
- **Commands:** `tts_available`, `tts_speak(paragraphs, first, rate)`, `tts_stop`.
- **Reader:**
  - A play button in the footer reads aloud from the paragraph on screen, highlights the paragraph being read and keeps it in view.
  - **Choosing where it starts** (added during the milestone): hold any paragraph to read aloud from there. While reading, tap a paragraph to jump to it. A toolbar offers previous paragraph, read this paragraph again, next paragraph and stop.
  - **Speed:** 0.6×–2× in steps of 0.2, in the text settings.
  - At the end of a chapter it goes on to the next one and keeps reading. Stop or leaving the reader stops speech.

## Out of scope
- Speech while the app is in the background (needs a foreground service), voice choice, EPUB footnote popups, styles from the book, downloading images of novel chapters, and fixed-layout EPUBs.

## Done-check
- **Rust:**
  - EPUB parsing (EPUB 3 nav and EPUB 2 NCX names, cover, spine order, unsafe paths and size limits);
  - import stores a library novel and reads chapters with local images;
  - re-import keeps progress;
  - novel downloads write and read passages;
  - the TTS service off Android reports unavailable.
- **Vitest:**
  - the import button calls the command with the picked file;
  - novel chapters show download buttons;
  - the reader's speech button sends the paragraphs from the one on screen, highlights the paragraph being read, and stops.
- 📱 **On the emulator:**
  - import an EPUB (a Gutenberg EPUB pushed to Downloads), read it with its images;
  - download a Gutenberg chapter and read it in airplane mode;
  - listen to a chapter with highlighting.

## Decisions
- **XML parsed leniently:** the OPF, NCX and nav documents go through `scraper`'s HTML parser, as real books often aren't well-formed XML.
  - Namespaced attributes (`xlink:href`) are found by local name, because `attr()` only looks at attributes without a namespace.
- **Splitting files into chapters:** a file listing several table-of-contents entries is split at their anchors (`path#start#end` unit ids). Gutenberg's Pride and Prejudice packs 61 chapters into 9 files.
  - Only the EPUB 3 nav's `toc` section counts, so page lists and landmarks are ignored.
- **SVG cover pages:** their `<image>` is shown as a picture. A page with no text and no table-of-contents name is called "Cover" (or "Illustration").
- **Novel downloads share the manga queue and worker.** One Downloads page lists both, with passage JSON in `downloads/novel/…`.
- **Speech is a small `Speech` class in the existing audio plugin,** reporting over JNI like the player. One utterance per paragraph keeps highlighting exact; long paragraphs are cut at sentence ends under 3,000 characters.
- **Keeping the spoken paragraph in view:** an instant scroll that only moves when the paragraph is off screen. Smooth `scrollIntoView` is dropped by Android's WebView inside the app's scrolling container.

## Verification (2026-10-31, Pixel_4 emulator)
- **Checks:** all pass. `cargo test --workspace` ran 257 tests; clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (125 app tests, 9 novel tests) are clean.
- **Real EPUBs:** `--ignored real_epub` on Gutenberg's Frankenstein gives 33 chapters (Cover, Letter 1…). Pride and Prejudice gives 65 chapters, with Chapter I–LXI split out of 9 files.
- **EPUB import:**
  - Library → Novels → Import EPUB opened the system picker, which only offers EPUB files.
  - Frankenstein imported with its cover, author, subjects and 33 chapters, and no download buttons.
  - The Cover chapter shows the cover image from the unpacked files, and Letter 1 reads normally.
- **Novel downloads:**
  - Chapter I of the Gutenberg Pride and Prejudice downloaded in 2.2 s to `downloads/novel/<title>/<chapter>/passage.json`.
  - In airplane mode it opened and read in full.
  - The title page's download badge didn't refresh for novels; fixed (novel title queries are now refreshed too).
- **Reading aloud:**
  - The speaker button reads from the paragraph on screen and highlights each paragraph as it is spoken.
  - Holding a paragraph started reading from it. Next jumped ahead and the view followed (37%), and previous went back.
  - Stop ends speech.

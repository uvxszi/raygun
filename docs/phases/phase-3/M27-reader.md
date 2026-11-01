# M27 — Manga reader

**Relevant docs:** ARCHITECTURE §3.2 (cache folders), M24 (manga foundation), reference-notes (mihon: reader modes; mihon_img_upscale: page priority and cache ideas).

## Goal
Reading feels like a real manga reader:
- three modes (vertical strip, paged left-to-right, paged right-to-left), remembered per title;
- tap zones and swipes in paged mode, a page slider, double-tap zoom;
- the next pages and the next chapter fetched ahead;
- pages already seen load instantly from a disk cache.

## Design

### Disk page cache (`raygun-net` image route)
- `/image/<token>` serves from `cache/pages/<token>` when present.
- Otherwise it fetches the image whole (at most 30 MB) and writes it to the cache before answering, but only when the answer is a 2xx with an `image/*` type.
- **Writes are atomic** (temp file, then rename), so a failed download never leaves a broken page.
- **Size cap:** 512 MB. When a write pushes the folder over it, the least recently used files are removed, by modification time; reading a file touches it.
- Covers use the same cache, so the library opens fast offline too.
- Tokens are a hash of URL and headers, so a page stays cached across sessions. The cache lives in `cache/`, so wiping it only costs re-downloads.

### Reading mode (migration `0009`)
- `title.user_reader_mode`: `webtoon` (the default), `pagedLtr` or `pagedRtl`.
- `manga_chapter` returns the title's mode; `manga_set_reader_mode(titleId, mode)` stores it.

### Reader UI
- **Webtoon:** the M24 strip, plus prefetch.
- **Paged:** one page fitted to the screen.
  - **Tap zones:** the left third, right third and middle. Middle shows or hides the controls; the side zones go back or forward, mirrored in RTL.
  - **Gestures and keys:** horizontal swipes turn pages, as do the arrow keys.
  - **Double-tap zoom:** toggles 2×, panned by dragging, and resets on page change.
  - **The last page** leads to an end card with Next chapter.
- **Controls:** the top bar holds back, title and chapter, and a mode picker. The bottom bar holds a page slider (reversed in RTL) and previous/next chapter buttons.
- **Prefetch:**
  - the 3 pages after the current one start loading;
  - near the end of a chapter, the next chapter's page list is fetched and its first 3 pages warmed;
  - turning to it is then instant.
- Progress saving as in M24, in every mode.

## Out of scope
- Spreads (two pages side by side), Mihon's vertical paged mode, reading-direction auto-detection from tags.
- Downloads for offline reading (M28); AI upscaling (M35).

## Done-check
- **Rust:**
  - the page cache (first fetch stored, second served from disk without the origin, non-images and failures not stored, eviction past the cap, atomic writes);
  - reader mode storage.
- **Vitest:**
  - paged mode: tap zones turn pages, mirrored in RTL;
  - the slider jumps;
  - the mode picker stores the mode;
  - the end card leads to the next chapter;
  - the next chapter is prefetched near the end.
- 📱 **On the emulator:**
  - switch a title to paged RTL;
  - tap through pages;
  - double-tap zoom;
  - reach the next chapter;
  - reopen a read chapter and see its pages served from the cache (log or timing).

## Decisions
- The cache keys files by the proxy's image token, which is a hash of URL and headers, so no index file is needed. LRU order comes from file modification times, and reads touch the file.
- Each image is fetched whole before it is served on the cached path; there is no streaming. Manga pages are small, and this lets the cache store only complete, verified images.
- Zoom is a 200% width image inside a scrollable container. This is simpler than a pinch transform and works with native scrolling on Android.
- A double tap waits 250 ms before acting on a single tap, so tap zones don't turn the page under a zoom gesture.

## Verification (2026-10-31, Pixel_4 emulator)
- All checks pass: `cargo test --workspace`, clippy, `pnpm lint`, `pnpm type-check` and `pnpm test`.
- MangaDex title "I Met the Male Lead in Prison":
  - **Paged RTL:** switched to paged RTL from the mode picker. The slider is mirrored, and tapping the left third moves forward (page 2 → 4).
  - **Zoom:** double-tap zooms to 2× and keeps the page's aspect ratio.
  - **Mode remembered:** after leaving, Continue opened Chapter 2 at its saved page, still in paged RTL.
  - **Cache:** `cache/pages` held the pages read so far (9 files, about 29 MB), and reopening them loaded without network fetches.

# M32 — Shosetsu compatibility

**Relevant docs:** ARCHITECTURE §7 (extensions, permissions), M13 (repositories, packages), M17 (Nuclear compatibility: a foreign format converted at install), M31 (`novel.source`). Reference: shosetsu `kotlin-lib` (GPL-3.0, the Lua API) and `shosetsuorg/extensions` (GPL-3.0, the official scripts), cloned into `reference/`.

## Goal
Shosetsu's community novel sources work in Raygun:
- add a Shosetsu repository;
- install its Lua scripts from the Extensions screen;
- browse, add and read their novels like any `novel.source`.

## Design

### Repository and install (`raygun-ext-host::repo`, `raygun-app::extensions`)
- **Index:** `parse_index` recognises a Shosetsu `index.json` (`scripts`, `libraries`). Each script becomes a repository entry:
  - id `shosetsu.<lang>.<file>`, version `ver`;
  - provides `novel.source`, net `*`, format `shosetsu`;
  - package `src/<lang>/<file>.lua`, SHA-256 from the index.
  - Libraries and their hashes stay on the index.
  - Scripts asking for another `libVer` major than 1 are left out.
- **Install:**
  - **Script:** download it and check its hash, then read its metadata header (`-- {"id":…,"dep":["novelvault>=1.0.0"]}`).
  - **Libraries:** download every library it needs, following their own headers; each checked against the index hash.
  - **Files written:** a generated `raygun-extension.json` (runtime `shosetsu`, entry `index.lua`), `index.lua` and `lib/<name>.lua`. These go into the usual `extensions/<id>/`, so update and uninstall work as for any extension.

### Runtime (`raygun-ext-host::shosetsu`)
- **Interpreter:** each extension gets a Lua 5.2 state (`mlua`, vendored, the dialect LuaJ runs) on its own worker thread, because Shosetsu's HTTP calls are blocking. The host thread awaits replies, so other extensions keep running.
- **Sandbox:**
  - **Libraries:** base, string, table, math and bit32 only. No io, os, package, debug, `dofile` or `loadfile`, and `load` takes text only.
  - **Limits:** 64 MB of memory, and an instruction hook that stops a call after 60 s.
- **The kotlin-lib globals, re-made:**
  - **Collection helpers:** `map`, `filter`, `first`, `flatten`, `mapNotNil`, `map2flat`, `pipeline`, `wrap`, `pageOfElem`, plus `NovelInfo`, `NovelChapter`, `Novel`, `NovelStatus` and `ChapterType`, as a Lua prelude adapted from kotlin-lib's resources.
  - **Constructors:** `Listing`, all the filter constructors, `Require` (from `lib/`), `Log`, `delay` (capped), `Reverse` and `AsList`.
  - **HTTP:** `GET`, `POST`, `Request`, `RequestDocument`, `GETDocument`, `HeadersBuilder`, `FormBodyBuilder`, `RequestBody`, `MediaType`, `RequestBuilder`, and responses with `body():string()`, `code()`, `isSuccessful()` and `headers()`.
  - **Constants:** `QUERY`, `PAGE`, `KEY_CHAPTER_URL`, `KEY_NOVEL_URL`.
- **Jsoup-like DOM** over `scraper`'s tree, shared and mutable:
  - **Documents and elements:** `select`, `selectFirst`, `text`, `ownText`, `attr` (including `abs:`), `hasAttr`, `hasClass`, `id`, `tagName`, `nodeName`, `html`, `outerHtml`, `remove`, `before`, `after`, `prepend`, `append`, `children`, `child`, `childNode(s)`, `childNodeSize`, `textNodes`, `parent`, sibling navigation, `getElementById`, `body`, `traverse` with `NodeVisitor`, `empty`, and `removeAttr` (accepted, ignored).
  - **Element lists:** `size`, `get`, `isEmpty`, `first`, `last`, `text`, `attr`, `select` and `remove`.
  - **Text nodes:** `text`, `isBlank` and `getWholeText`.
- **A Jsoup-compatible selector engine** (CSS selectors don't cover what scripts use):
  - **Basics:** groups, the four combinators, relative selectors (`> p`), type, `#id`, `.class` and attribute operators (`= ^= $= *= ~= |= !=`).
  - **Pseudo-classes:** `:has`, `:not`, `:contains`, `:containsOwn`, `:matches`, `:matchesOwn`, `:eq/:lt/:gt`, the `nth-*`, first/last/only child and of-type ones, `:empty` and `:root`.
- **Network:** any public http(s) host.
  - Literal loopback, private and link-local addresses and `localhost` are refused, and a DNS resolver drops private answers, so a script can't reach the LAN.
  - Bodies are capped at 8 MiB, with Shosetsu's 10-minute response cache left out.

### `novel.source` on top of a script
| Call | Script |
|---|---|
| `popular` / `latest` | the first / second listing (the first again when there is one), called with `{PAGE = startIndex + page − 1}` plus filter defaults; a non-incrementing listing has one page |
| `search` | `search({QUERY, PAGE, …filter defaults})`; refused when `hasSearch` is false |
| `details`, `chapters` | one `parseNovel(link, true)` cached for a minute: title, cover, authors, artists, description, genres and tags, status (publishing, completed, paused or unknown), and chapters by `order` |
| `passage` | `getPassage(link)`: HTML (with `baseUrl` from `expandURL`) or text, by `chapterType` |

- **Ids:** novel and chapter ids are the script's (shrunk) links.
- **Filter defaults** follow the filter types: text "", switch and checkbox false, tri-state and dropdown 0.

## Out of scope
- Script settings (`updateSetting`), the listing names beyond the first two, search filters in the UI, cookies, Cloudflare (`hasCloudFlare` sources fail like any blocked site), JS-type extensions and styles from Shosetsu repositories.

## Done-check
- **Rust:**
  - the selector engine (combinators, attributes, the Jsoup pseudos, relative selectors);
  - DOM operations;
  - the sandbox (no io, os or debug, memory and time limits);
  - network refusals;
  - Shosetsu index parsing and install (dependency closure, hashes);
  - a fixture script exercising listings, search, `parseNovel` and `getPassage` against a local server, through `Require`, `map`, `NovelInfo` and `pageOfElem`;
  - a live ignored test that installs NovelFull from the official repository and lists novels.
- **Vitest:** the Extensions screen shows Shosetsu entries with "any site" access.
- 📱 **On the emulator:**
  - add the official Shosetsu repository and install a source;
  - browse it under Novels;
  - open a novel and read a chapter.

## Decisions
- **Lua 5.2 through `mlua`, vendored:** this matches LuaJ's dialect and cross-compiles for Android with the NDK. LuaJ also accepts Lua 5.3's `\u{XXXX}` escapes, which the official `unhtml` library uses, so they are rewritten to byte escapes at load.
- **One thread per script:** kotlin-lib's HTTP calls block. The host thread awaits replies, so a slow site never stalls other extensions. A call may take 80 s, because some scripts pause a second between chapter-list pages.
- **Jsoup behaviour followed exactly where scripts depend on it:**
  - `select` and `selectFirst` include the element itself; ReadFromNet needs this.
  - Descendant combinators stop at the scope.
  - `map` passes 0-based indexes over element lists.
- **`os` holds only the clock:** `time`, `clock` and `date` exist because FreeWebNovel paces its requests with `os.time()`. The rest of `os`, and all of `io`, `debug` and `package`, are absent.
- **Network:** scripts reach any public host. Literal private addresses, local names, private DNS answers and redirects to them are refused, and the install sheet says "Any public website (never this device or your local network)".
- **Libraries:** a script's own `Require("x")` calls count as dependencies too, because NovelBuddy uses `unhtml` without declaring it.

## Verification (2026-10-31, Pixel_4 emulator and live sites)
- **Checks:** all pass. `cargo test --workspace` ran 251 tests; clippy, `pnpm lint`, `pnpm type-check` and `pnpm test` (121 app tests) are clean.
- **Live installs** from the official repository, each listing, parsing a novel and reading chapter 1 (`--ignored shosetsu_official`):
  - NovelFull, NovelFull (NET), NovGo, ReadNovelFull, NovelBuddy, FreeWebNovel, WuxiaWorld.site, LightNovelHeaven, PawRead, NovelArrow and WTR-LAB work. Martial God Asura alone has 6,736 chapters.
  - **Refused by their sites with HTTP 403:** Royal Road, ScribbleHub, NovelHall, Honeyfeed and FimFiction answer automated clients that way. Raygun doesn't work around bot checks.
  - **Outdated scripts:** LightNovelPlus (the site moved) and Parahumans (one of its blogs refuses with HTTP 403).
  - **Rate-limited:** repeated test runs got GitLab to answer HTTP 429. Some hashes then failed to verify; the committed index and scripts match exactly.
- **On the emulator:**
  - **Install:** added `https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json` in Extensions → Repositories; it was named "Shosetsu extensions". Browse listed its scripts with "It may reach any website". The install sheet showed the any-website access. NovelFull (NET) installed with 4 libraries and loaded.
  - **Reading:** Library → Novels → Browse sources listed NovelFull (NET) next to Project Gutenberg. Popular showed covers and titles. "Infinite Mana In The Apocalypse" opened with its author, genres, description and 5,930 chapters, and Chapter 1 read in the novel reader.

# LNReader plugins

Raygun runs LNReader's novel plugins: the compiled JavaScript plugins built from
LNReader/lnreader-plugins, as the LNReader app runs them.

## Adding a repository

Browse → Extensions → Repositories → add the repository's plugin list, the `plugins.min.json`
(or `plugins.json`) an LNReader repository publishes. Its plugins are listed with their language
and installed like any extension (as `lnreader.<plugin id>`); each may reach any public website,
as LNReader's do.

## How they run

Each plugin runs in its own QuickJS runtime, inside the wrapper LNReader evaluates it in
(CommonJS, with a `require`). `require` hands out what LNReader's does, rebuilt over Raygun:

| Library | In Raygun |
|---|---|
| `cheerio` (`load`) | The cheerio API over pages parsed in Rust; selectors are Raygun's Jsoup-compatible engine, with jQuery's `:first`, `:last`, `:eq()`, `:gt()`, `:lt()`, `:even` and `:odd` at a selector's end taken from the whole match. |
| `htmlparser2` (`Parser`) | The page's open, text and close events, from its parsed tree. |
| `dayjs` | Parsing (ISO dates, numeric formats such as `DD/MM/YYYY`), `add`, `subtract`, `format`, `toISOString`. |
| `urlencode` | UTF-8, and other charsets (GBK, Shift_JIS...) through Rust. |
| `@libs/fetch` | `fetchApi` and `fetchText` over Raygun's requests, with the extension's cookies; forms post as `application/x-www-form-urlencoded`. |
| `@libs/storage` | `storage` kept across restarts; `localStorage` and `sessionStorage` for the run. |
| `@libs/novelStatus`, `@libs/filterInputs`, `@libs/defaultCover`, `@libs/isAbsoluteUrl` | As in LNReader. |

The plugin becomes a `novel.source`:

| Raygun | The plugin |
|---|---|
| Popular, Latest | `popularNovels(page, { showLatestNovels })` |
| Search | `searchNovels`; with an empty search and filters, `popularNovels` with them |
| Filters | Picker → a choice, Switch → a toggle, Checkbox and XCheckbox groups → tags (free text has no Raygun filter) |
| A title | `parseNovel`: name, cover, author, artist, summary, genres, status, chapters; a novel listed page by page (`totalPages`, `parsePage`) has every page's chapters |
| A chapter | `parseChapter`'s HTML |
| Settings | `pluginSettings` (switches, choices, text) in the extension's settings; the plugin reads them through `storage.get` |

A site that answers with a browser check fails with that, the M46.48 way, even when the plugin
reads the refused page as an empty list. Raygun never passes the check itself.

## Limits

- Plugins that decrypt pages (`@libs/aes`) or talk protobuf (`fetchProto`) don't run; they say
  so when called.
- Answers are read as text in UTF-8: `fetchText` with another charset reads with replacement
  characters.
- `customJS` and `customCSS`, which restyle LNReader's own reader, aren't used.
- Sites behind a browser check (many, Royal Road among them) answer only to a browser.

## Checking

`lnreader_live` reads LNReader's own repository and runs plugins of sites that publish original
or licensed fiction (Webnovel, Inkitt, Scribble Hub, Royal Road, Wuxiaworld) from listing to a
chapter's text; sites behind a browser check are reported and skipped:

```sh
cargo test -p raygun-ext-host -- --ignored lnreader_live --nocapture
```

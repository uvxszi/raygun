# Raygun extensions

An extension gives Raygun the content of one site or service: songs and their streams, lyrics,
charts, comics, books, films and series, podcasts or audiobooks. It is a small JavaScript bundle
with a manifest. Raygun calls it through **providers** of fixed kinds and shows what it answers in
its own screens (Search, Browse, the library, the reader, the players), side by side with every
other source, so a new source needs no screens of its own.

Extensions are written in TypeScript with `@raygun/sdk`, checked and tried with the `raygun-ext`
tool, and published in a repository that anyone can add to Raygun.

- [Getting started](getting-started.md): create, build, check, try and run an extension.
- [Manifest](manifest.md): every field of `raygun-extension.json`.
- [Host API](host-api.md): the `raygun` global and the SDK's helpers.
- [Repositories](repositories.md): publishing extensions for others to install.
- [Other formats](other-formats.md): the other apps' extensions and addons Raygun runs.

## Kinds

Each kind has a page with its calls, what Raygun reads in the answers, and a complete example on
a legal site. `raygun-ext init --kind <kind>` creates that example as a new extension.

| Kind                                            | What it gives Raygun                                            | Example                  |
| ----------------------------------------------- | --------------------------------------------------------------- | ------------------------ |
| [`music.metadata`](music-metadata.md)           | Song search, and optionally artists, albums, playlists, events  | Openverse                |
| [`music.stream`](music-stream.md)               | Streams for songs found by other sources                        | Internet Archive         |
| [`lyrics`](lyrics.md)                           | Lyrics for the playing song                                     | Wikisource               |
| [`music.charts`](music-charts.md)               | Charts in Browse, songs for genre and mood pages                | Internet Archive         |
| [`music.tags`](music-tags.md)                   | Genres of a song                                                | MusicBrainz              |
| [`music.info`](music-info.md)                   | Artist facts and song credits                                   | MusicBrainz              |
| [`music.recommend`](music-recommend.md)         | Similar artists, top songs, new releases and radios             | Openverse                |
| [`manga.source`](manga-source.md)               | Comics: lists, search, chapters, pages                          | Pepper&Carrot            |
| [`novel.source`](novel-source.md)               | Books: lists, search, chapters, their text                      | Project Gutenberg        |
| [`video.source`](video-source.md)               | Films and series: lists, search, episodes, videos               | Internet Archive         |
| [`podcast.directory`](podcast-directory.md)     | Podcasts to find and follow                                     | NASA, LibriVox           |
| [`audiobook.directory`](audiobook-directory.md) | Audiobooks to find, with their chapters                         | LibriVox                 |
| [`dictionary`](dictionary.md)                   | Words looked up in the book reader                              | Free Dictionary API      |

One extension may provide several kinds; it lists them all in its manifest's `provides`.
MusicBrainz's genres, for example, are a `music.tags` provider with the id `genres`.

## The tools

`raygun-ext` comes with the SDK in a Raygun checkout (`packages/sdk`); neither is published on
npm. From the checkout's root it runs as `pnpm raygun-ext`; inside an extension's folder as
`pnpm exec raygun-ext`.

| Command                                    | What it does                                                                                         |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `init <dir> --kind <kind> --id --name`     | Creates an extension from the kind's template.                                                      |
| `build [dir]`                              | Bundles `src/index.ts` into the manifest's `entry`.                                                 |
| `validate [dir]`                           | Checks the manifest, the types, the providers and their calls, and the hosts the code fetches from. |
| `test [dir] <provider> <method> [args]`    | Runs one call in Raygun's own extension host and says what Raygun keeps of the answer.              |
| `dev [dir] --into <folder>` / `--android`  | Rebuilds on every save and installs into an app on this computer or a phone.                        |
| `pack [dir] --repo <folder>`               | Adds a package to a repository folder and rewrites its `index.json`.                                |
| `pack-all <folder> --repo <folder>`        | Packs every extension in a folder.                                                                  |

## How extensions run

- **Sandbox.** Each extension runs in its own QuickJS runtime (ES2022) inside Raygun, on Android
  and desktop alike. There is no `fetch`, `URL`, `setTimeout`, DOM, Node API or file access: only
  the [`raygun` global](host-api.md). The bundle is one plain script, without a module loader.
- **Network.** `raygun.http.fetch` reaches only the hosts in the manifest's `permissions.net`,
  redirects included. `*` allows any public host and never this device or the local network.
  Raygun shows the list before an extension is installed.
- **Calls.** Raygun calls a provider's methods with JSON and reads back JSON. An extension may be
  in several calls at once; they take turns at its runtime between `await`s. A call nobody waits
  for any more is stopped, with its requests.
- **Many sources.** Searches ask up to 16 sources at once, the ones that answered well first.
  Answers show as they come. A source that fails three times in a row sits out for a minute,
  then twice as long after each further failure, up to 15 minutes; one that averages over 4 s is
  asked after the quick ones.
- **Reading answers.** Raygun checks every answer: entries missing what they need are dropped one
  by one while the rest still show, text is trimmed, and addresses must be `http(s)`.
  `raygun-ext test` prints what Raygun keeps and why it drops the rest.

### Limits

| What                               | Limit                                                                                                  |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------ |
| A call                             | 30 s; 12 s for song searches, streams, lyrics, artist info and audiobooks; 15 s for a catalogue search row |
| Script without waiting             | 5 s, counted again after each request or sleep                                                         |
| Memory                             | 64 MB per extension, 1 MB of stack                                                                     |
| A request                          | 20 s, 10 redirects, 8 MB of body (read as text)                                                        |
| Requests at once                   | 6 per site, 32 across all extensions                                                                   |
| `raygun.time.sleep`                | 10 s per call                                                                                          |
| `raygun.storage`                   | 1 MB per extension                                                                                     |
| A package                          | 10 MB                                                                                                  |

### Errors

Throw an `Error` whose message says what went wrong in a plain sentence: "This chapter is no
longer in the book". Raygun shows it under the source's name, and its log keeps the stack trace.
For HTTP failures, throw `HTTP <status> from <address>` (as `fetchJson` and `fetchText` do):
Raygun reads the status, and what the request met, to say whether the site refused, rate-limited,
lost the page, failed or wants a browser check. An answer that has nothing, such as no lyrics or
no search results, is not an error: answer `null` or an empty list.

### Adult content

An extension whose content is for adults sets `"contentRating": "nsfw"`: Raygun then hides it
while NSFW content is off (music extensions while Explicit music is off). Extensions that carry
some adult titles read `raygun.content.showNsfw()` and leave those out, or mark them `nsfw`, and
mark explicit songs `explicit`.

## Content rules

- **Paraline**, Raygun's own repository, accepts only sources of public-domain or openly licensed
  content: works in the public domain, or under Creative Commons or a similar open license, and
  open data such as MusicBrainz's.
- **Third-party repositories** are their authors' responsibility. Raygun lists what each extension
  can reach before it's installed, and runs each in its sandbox.
- **No bot-check circumvention.** Raygun has no API that solves or bypasses captchas, Cloudflare
  challenges or DRM, and won't add one. A source that meets such a check fails with a message
  that says so.
- **Be a good client.** Use a site's public API where it has one, keep to its rate limits
  (`raygun.time.sleep`), and name your extension in the `User-Agent` when an API asks clients to
  identify themselves (MusicBrainz and Wikimedia do).

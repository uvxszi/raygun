# Other apps' plugins and servers

Raygun is a client: it runs the repositories and servers people add themselves. Besides its own
extensions it reads several other apps' plugin formats, and connects to media servers people
run. This page lists the formats looked at, what Raygun does with each, and why.

Two ways in:

- **Repositories** are added by address in Browse → Extensions → Repositories. Raygun reads the
  index, lists the extensions with what they provide, and installs them through the normal
  Extensions screen.
- **Servers** are added in Browse → Sources → Servers with their address and a sign-in. A
  connection becomes an extension of its own (runtime `server`), so its films, music, books and
  comics show wherever sources do. Sign-ins stay in Raygun's encrypted secrets store on the
  device.

## Formats

| Format | What it is | In Raygun | How, or why not |
|---|---|---|---|
| Raygun extensions | JavaScript sources on `@raygun/sdk` | Supported | Raygun's own runtime (QuickJS). |
| Shosetsu | Lua novel sources, `index.json` repositories | Supported | Lua 5.2 sandbox with Jsoup- and OkHttp-like libraries (`raygun-ext-host::shosetsu`). |
| Miru | JavaScript video, comic and novel sources | Supported | QuickJS with Miru's API rebuilt (`raygun-ext-host::miru`). |
| Nuclear | TypeScript music plugins | Supported | Converted at install (`packages/nuclear-compat`). |
| Stremio addons | HTTP addon protocol | Supported | Browse → Sources → Addons (`raygun-app/src/stremio`). |
| Podcast feeds | RSS and Atom | Supported | Followed by address or found through directories. |
| [Jellyfin and Emby](jellyfin.md) | Media servers | Added | Server connection: films and series, music, lyrics. |
| [Subsonic, OpenSubsonic](subsonic.md) | Music servers (Navidrome, Gonic, Airsonic, Ampache...) | Added | Server connection: music and lyrics. |
| [Audiobookshelf](audiobookshelf.md) | Audiobook server | Added | Server connection: audiobooks. |
| [OPDS 1.x](opds.md) | Book and comic catalogues (Komga, Kavita, Calibre, Calibre-Web, Project Gutenberg...) | Added | Server connection: comics streamed page by page (OPDS-PSE), EPUB books chapter by chapter. |
| Komga and Kavita APIs | Their own REST APIs | Through OPDS | Both serve OPDS, which covers browsing and reading. Their APIs would add read progress kept on the server. |
| [Kodi add-ons](kodi.md) | Python plugins in `addons.xml` repositories | Supported | Video and music plugins run unchanged in RustPython with Kodi's modules rebuilt over Raygun (`raygun-ext-host::kodi`); the libraries they import install with them. |
| [LNReader plugins](lnreader.md) | JavaScript novel sources (CommonJS) | Added | QuickJS with what LNReader hands its plugins rebuilt: cheerio, htmlparser2, dayjs, urlencode and its `@libs` (`raygun-ext-host::lnreader`). |
| Paperback (0.8, 0.9) | JavaScript comic sources (browserify bundles) | Feasible, not yet | Sources bundle their own cheerio; the app supplies `App.create*` constructors, a request manager and its own type system. The sources listed in public repositories are mostly aggregators of other sites. |
| Sora / Sulfur (now Luna) | JavaScript video modules with a JSON manifest | Feasible, not yet | A small API (`fetchv2` and four functions returning JSON) that maps onto `video.source`. The project is archived in favour of Luna. |
| Aidoku | WebAssembly comic sources (`.aix`) | Feasible, not yet | `wasmi` (a pure-Rust interpreter) runs on Android and Windows. The host side is large: about 80 imports (`std`, `net`, `html`, `json`, `defaults`, `canvas`, `js`) exchanging postcard-encoded values, picture descrambling on a canvas, and an embedded JavaScript context; sources built before Aidoku 0.7 use an older interface. |
| [Mihon, Tachiyomi](mihon.md) | Kotlin extensions in APKs, with public source code | Converted from source | Stores (`index.min.json`, `repo.json`, `index.pb`) are read as repositories; an extension built on a theme Raygun has ported (Madara, MangaThemesia) is converted from its Kotlin into runtime `mihon` at install, the rest are listed with the reason they can't be. No APK is loaded. |
| Aniyomi | Kotlin anime extensions in APKs | Not yet | Its stores read as Mihon's; anime extensions are left out. |
| Cloudstream | Kotlin plugins (`.cs3`, Android bytecode) | Not feasible | As for Mihon. Its repositories are recognized and named when added. |
| Plex | Media server | Feasible, not yet | Its API takes an `X-Plex-Token`; signing in goes through plex.tv. |
| DLNA / UPnP media servers | ContentDirectory over SOAP | Feasible, not yet | Discovery by SSDP (Raygun already holds a multicast lock for Connect) and a folder tree to browse, which fits search-first sources poorly. |

## Server connections

A connection's folder in the extensions folder holds `raygun-extension.json` and `server.json`
(`ServerConfig`: kind, address, name, user, what the secret is). The secret itself is kept in the
encrypted secrets store under `server:<extension id>` and handed to the extension host at each
load. Addresses Raygun stores and syncs (covers, chapter files, page images) carry no sign-in:
when Raygun fetches one under a connected server, `raygun_net::server_auth` adds the server's
query parameters or headers, the way a browser sends a site's cookie. Streams are resolved when
they play, signed in, and asked for again after six hours.

A server on the home network needs Android's local network access; the add sheet asks for it
first. Plain `http` is accepted, since home servers rarely have certificates; an address typed
without a scheme tries `https` first, then `http`.

Each server kind has live checks against public demo servers, and the repository formats against
their own public repositories:

```sh
cargo test -p raygun-ext-host -- --ignored servers::tests --nocapture
cargo test -p raygun-ext-host -- --ignored lnreader_live --nocapture
cargo test -p raygun-ext-host -- --ignored kodi_official_live --nocapture
cargo test -p raygun-ext-host -- --ignored kodi_media_ccc_live --nocapture
```

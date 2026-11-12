# Nuvio providers

Raygun runs the providers of Nuvio repositories: JavaScript stream scrapers that, given a movie
or an episode by its TMDB id, answer with streams. Nuvio (tapframe/NuvioStreaming, GPL-3.0)
evaluates them in QuickJS with globals of its own; Raygun rebuilds those globals over its own
runtime, and the providers' streams play beside the Stremio addons' in a play session.

## Adding a repository

Sources → Add takes the repository's `manifest.json`, or the folder it sits in (Nuvio adds
`manifest.json` itself), or the project's GitHub page. The manifest names the repository and
lists its providers:

```json
{
  "name": "Example providers",
  "version": "1.0.0",
  "scrapers": [
    {
      "id": "example",
      "name": "Example",
      "version": "1.0.0",
      "filename": "providers/example.js",
      "supportedTypes": ["movie", "tv"],
      "enabled": true,
      "logo": "https://example.org/logo.png",
      "contentLanguage": ["en"]
    }
  ]
}
```

Each provider installs on its own, as `nuvio.<provider id>`, from beside the manifest (a leading
`/` is dropped, as Nuvio drops it) or from its own address. Providers may reach any public
website, never the device's network; every request goes through Raygun's own, so Offline Mode,
the per-site request limits and the extension's cookies apply. A provider has its own switch in
Browse → Sources, like any source.

Listed but not installable ("Not for Raygun"), with the reason:

- `enabled: false`: its repository turned it off (Nuvio never runs these either).
- `supportedPlatforms` without `android`, or `disabledPlatforms` with it: Raygun's runtime
  stands in for Nuvio's Android one.
- `supportedTypes` with neither movies nor series (`tv`, `series`, `show` and `other` count as
  series, as in Nuvio).
- A file that defines no `getStreams`: nothing would be called.

## How they run

Each provider runs in its own QuickJS runtime, wrapped as Nuvio wraps it (a CommonJS module:
`module`, `exports`, `require`). Its runtime holds what a converted plugin may (96–256 MB by the
device's memory) and gets twice the CPU time of a Raygun extension.

| Nuvio gives | In Raygun |
|---|---|
| `fetch(url, { method, headers, body, redirect, signal })` | Raygun's requests. As in Nuvio, a request that fails answers status 0 with the reason instead of throwing, and `json()` reads an answer that isn't JSON as `null`; `arrayBuffer()` has the answer's bytes. |
| `require('cheerio')`, `cheerio-without-node-native`, `react-native-cheerio`, the global `cheerio` | The cheerio Raygun's LNReader runtime has, over pages parsed in Rust. |
| `require('crypto-js')`, `CryptoJS` | crypto-js 3, with `enc.Base64url` added. |
| `crypto.subtle` | `digest` (MD5, SHA-1, SHA-256/384/512), HMAC `sign`/`verify`, AES-CBC, AES-CTR and AES-GCM `encrypt`/`decrypt`, PBKDF2 `deriveBits`/`deriveKey`, raw `importKey`/`exportKey`, `generateKey`; `getRandomValues`, `randomUUID`. |
| `atob`, `btoa`, `URL`, `URLSearchParams`, `Headers`, `FormData`, `TextEncoder`, `TextDecoder` | As in a browser (UTF-8; `TextDecoder` also reads Latin-1). |
| `setTimeout`, `setInterval`, `AbortController`, `AbortSignal.timeout` | Timers over Raygun's sleep; an interval runs at most a minute. |
| `SCRAPER_ID` | The provider's id in its repository. |
| `SCRAPER_SETTINGS` | The provider's settings: the fields its `onSettings()` declares (text, select, toggle) are the extension's settings. |
| `TMDB_API_KEY` | The extension's "TMDB API key" setting, empty until set. Raygun has no key of its own to give. |

## What it's asked

The provider becomes a [`video.streams`](../video-streams.md) source, the kind Raygun's own
extensions can provide too. For a movie or an episode of a title from the
Stremio addons (Cinemeta's, say), Raygun calls `getStreams(tmdbId, mediaType, season, episode)`:

- `tmdbId` is the title's TMDB id when its details give it (Cinemeta's `moviedb_id`, a `tmdb:`
  id), else its IMDb id, as Nuvio passes it when it can't find TMDB's.
- `mediaType` is `movie` or `tv`; a provider is asked only for the types it lists.
- `season` and `episode` count as TMDB and IMDb do. An anime entry's episode is asked by the
  IMDb episode its addon maps it to; one without that mapping, and live channels, aren't asked.

Its answer, an array of Nuvio's stream objects, becomes a source in the session (named after the
provider) and on the title page:

| Nuvio's stream | In Raygun |
|---|---|
| `url` (or `{ url }`) | The stream. An HLS (`.m3u8`, or `type: "hls"`) or DASH address plays as one. |
| `infoHash`, `fileIdx`, `sources` | A torrent's magnet link, played by Raygun's torrent engine while torrents are on. |
| `name` (or `provider`) and `quality` | The stream's name; the quality gives its height for the Quality menu. |
| `title`, `size`, `language`, `seeders` | Its description; the size is read for the Sources sheet. |
| `headers` | Sent with every request for the stream. |
| `subtitles` (`url`, `language`, `name`) | Subtitle tracks, beside the subtitle addons'. |

A provider that answers nothing after a request it made was refused says what it met
("example.org answered HTTP 503"). Providers are asked a few at a time, each within 20 seconds;
one that keeps failing sits out a while, as other sources do. In a play session each provider's
streams join as it answers, so a slow one never holds up the addons'.

## Limits

- `WebAssembly` isn't there; providers that need it fail.
- Keys other than raw ones (`pkcs8`, `spki`, `jwk`) and signatures other than HMAC (RSA, ECDSA)
  aren't in `crypto.subtle`.
- Raygun only has TMDB ids the title's details carry. A provider that looks titles up on TMDB
  itself needs its "TMDB API key" setting.

## Checking

`cargo test -p raygun-ext-host nuvio` runs providers written for these tests (a stand-in for the
Internet Archive serving a public-domain film, Night of the Living Dead) through the host as the
app calls them.

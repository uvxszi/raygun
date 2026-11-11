# The host API

An extension runs in QuickJS with the JavaScript of ES2022 (`JSON`, `Promise`, `Map`, `Set`,
`Date`, regular expressions with Unicode property escapes, `encodeURIComponent`…), base64's
`atob` and `btoa`, and one global of Raygun's: `raygun`. There is no `fetch`, `URL`,
`URLSearchParams`, `setTimeout`, `TextDecoder`, `Intl`, DOM, `require` or Node API;
`raygun-ext validate` warns when the code uses one. The SDK's types describe it all
(`packages/sdk/src/types.ts`), and its helpers come in the bundle.

## `raygun.http.fetch(url, init?)`

```ts
import { fetchJson, fetchText } from '@raygun/sdk';

type Answer = { results?: { title: string }[] };

export const search = async (words: string) => {
  // A raw request: the status is the caller's to check.
  const response = await raygun.http.fetch(
    `https://api.openverse.org/v1/audio/?q=${encodeURIComponent(words)}`,
    { headers: { Accept: 'application/json' } },
  );
  if (response.status === 429) throw new Error('Openverse is busy; try again in a minute');
  if (!response.ok) throw new Error(`HTTP ${response.status} from Openverse`);
  const answer = await response.json<Answer>();

  // The helpers throw `HTTP <status> from <address>` for a status that isn't 2xx.
  const page = await fetchText('https://www.gutenberg.org/ebooks/84');
  const same = await fetchJson<Answer>('https://api.openverse.org/v1/audio/?q=bach');
  return { answer, page, same };
};
```

- **Hosts**: only those in the manifest's `permissions.net`, for the request and each redirect
  (10 at most).
- **`init`**: `method` (`GET` by default), `headers`, and `body` as text (send JSON with
  `JSON.stringify` and a `Content-Type`).
- **The answer**: `status`, `ok` (2xx), `url` after redirects, `headers` with lower-case names
  (repeated ones joined with `, `), and `text()` and `json()`. Bodies are read as UTF-8 text, up
  to 8 MB; binary bodies can't be read.
- **Failures**: a request that can't be made (no network, an unknown host, a refused host, 20 s
  without an answer) rejects. An HTTP error status resolves; check `ok`.
- **User-Agent**: Raygun's own (people may change it in its settings) unless `headers` names one.
  After a browser check on a site was passed by hand (an experimental setting), requests to that
  site that name none go with the browser's, which the pass holds for, while its cookies last.
- **Cookies**: each extension keeps its own, saved across restarts: a `Set-Cookie` comes back on
  later requests to its site. A `Cookie` header in `headers` replaces them for that request.
  Clearing the extension's data forgets them. A browser check passed by hand adds the browser's
  cookies for the checked address (such as Cloudflare's `cf_clearance`).
- **Turns**: 6 requests at once to one site and 32 across every extension; more wait their turn.
  `Promise.all` over a few requests is fine.

## `raygun.html.select(html, selector)`

CSS selectors over HTML, as a list of elements in document order. It throws for an invalid
selector.

```ts
export const episodes = (html: string) =>
  raygun.html.select(html, 'figure figcaption a[href]').map((link) => ({
    name: link.text.trim(),
    href: link.attr('href'),
    // Selects again inside the element.
    pictures: link.select('img').map((image) => image.attrs.src ?? ''),
  }));
```

An element has `text` (all the text inside it), `html` (its outer HTML), `inner` (its inner
HTML), `attrs` and `attr(name)` (`null` when missing), and `select(selector)`. `select` on an
element parses its HTML again as a fragment, so select table rows (`tr`) from the document.

## Limits

- **Memory**: an extension's runtime holds at most a sixty-fourth of the device's memory, between
  64 and 128 MB. Code converted from another app (Nuclear, Miru, LNReader, Mihon) gets a
  twenty-fourth, between 96 and 256 MB, and all runtimes together an eighth (256 MB to 1 GB).
  Keep pages as strings and read them with `raygun.html.select` or regular expressions; arrays of
  numbers cost 16 bytes an item.
- **CPU budget**: 5 s of script without waiting (a `fetch` or a `sleep` starts it again), regular
  expressions included.
- **Stack**: 1 MB of nested calls; deeper recursion stops with "Maximum call stack size
  exceeded".

A call that runs out says so ("… needed more memory than a source may use", "… kept the
processor busy too long"), with what it held under More info. A source that keeps failing, or
keeps running out, sits out a while so searches don't wait on it.

## `raygun.time.sleep(ms)`

Waits without using the CPU budget, at most 10 s per call: to pace an API's rate limit, or to
give up on slow requests in time.

```ts
let last = 0;

/** One request a second, as MusicBrainz asks. */
export const paced = async (url: string) => {
  const wait = last + 1100 - Date.now();
  if (wait > 0) await raygun.time.sleep(wait);
  last = Date.now();
  return raygun.http.fetch(url);
};

/** What's come of `work` after 8 s, so the call answers before Raygun stops waiting. */
export const within = async <T>(work: Promise<T>, fallback: T) =>
  Promise.race([work, raygun.time.sleep(8000).then(() => fallback)]);
```

## `raygun.storage`

JSON values kept per extension across restarts and updates, up to 1 MB in all; clearing the
extension's data empties it. Calls are synchronous.

```ts
type Token = { value: string; expires: number };

export const token = (): Token | null => {
  const saved = raygun.storage.get<Token>('token');
  return saved && saved.expires > Date.now() ? saved : null;
};

export const remember = (value: string) => {
  // Throws when the store would grow past 1 MB.
  raygun.storage.set('token', { value, expires: Date.now() + 3_600_000 });
};

export const forget = () => {
  for (const key of raygun.storage.keys()) raygun.storage.remove(key);
};
```

`get` answers `null` for a missing key. Keep caches of pages in memory (module-level variables
live as long as the extension is loaded) rather than in storage.

## `raygun.settings.get(key)`

The person's value for one of the manifest's [settings](manifest.md#settings), else its
`default`, else `null`. Changing a setting reloads the extension, so values can be read once.

```ts
const LANGUAGES: Record<string, string> = { English: 'en', French: 'fr' };

export const language = () => LANGUAGES[raygun.settings.get<string>('language') ?? ''] ?? 'en';
```

## `raygun.needsSetup(reason)`

Says the extension can't work until someone sets it up, and throws: its key or sign-in is
missing, or its own API refused it (an HTTP 401 or 403 from its API with the person's key).
Raygun then asks the extension nothing, no calls and no requests, until its settings change, and
lists it with the sources that need a key rather than the ones that failed. Use the SDK's
`needsSetup`, which throws a plain error in a Raygun that predates it. A setting marked
[`required`](manifest.md#settings) covers an empty key without a call at all.

```ts
import { needsSetup } from '@raygun/sdk';

export const signedFetch = async (url: string, key: string) => {
  const response = await raygun.http.fetch(url, { headers: { 'X-Api-Key': key } });
  if (response.status === 401 || response.status === 403) {
    needsSetup('the API refused the key');
  }
  return response;
};
```

## `raygun.content.showNsfw()`

Whether NSFW (18+) content is shown; off by default. While it is off, leave adult titles out, or
mark them `nsfw: true` and Raygun hides them.

## `raygun.youtube`

YouTube through its own web API, answered by Raygun; on desktop yt-dlp answers what that API
doesn't. Converted Nuclear plugins reach it as their `Ytdlp` API. The requests go out as the
extension's own, so its `permissions.net` must name `www.youtube.com`, `music.youtube.com` and
`*.googlevideo.com`.

| Call                           | What it gives                                                                                                     |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| `search(query, limit?)`        | Songs, then music videos, from YouTube Music (10 without a limit, at most 50).                                    |
| `stream(video, options?)`      | A video's audio (an id or an address), checked to play to its end, with the `headers` to send; `targetKbps` and `decodable` (AAC over Opus) pick the format. |
| `playlist(playlist)`           | Every video of a playlist (an id or an address with `list=`).                                                     |

## `raygun.log` and `console`

`raygun.log.debug`, `info`, `warn` and `error` (and `console.log`, `info`, `debug`, `warn`,
`error`) write to Raygun's log under the extension's id, and to `raygun-ext test`'s output.
Arguments that aren't text are written as JSON.

## `raygun.providers.register(provider)`

Registers a provider; call it (or the SDK's `registerProvider`) at the top level, while the
script first runs. Raygun refuses a kind missing from `provides`, an id that isn't
`[a-z0-9][a-z0-9_-]*` (64 characters at most), and an id registered twice. `raygun.apiVersion`
is `1`.

## The SDK's helpers

`@raygun/sdk` holds the types and a few helpers, bundled into the extension when it uses them:

| Helper                                 | What it does                                                                                     |
| -------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `defineProvider(provider)`             | Checks a provider against its kind's contract (types only) and returns it.                       |
| `registerProvider(provider)`           | `raygun.providers.register`.                                                                     |
| `fetchJson(url, init?)`                | Fetches and parses JSON; throws `HTTP <status> from <url>` for a status that isn't 2xx.          |
| `fetchText(url, init?)`                | The same, for text.                                                                              |
| `query(params)`                        | A query string: `query({ q: 'a b', page: 2 })` is `q=a%20b&page=2`; lists repeat their name.    |
| `resolveUrl(base, href)`               | A link resolved against its page's address, as a browser does.                                   |
| `selectValue(filters, filter)`         | A select filter's choice in a search, else its default, else its first option.                   |
| `toggleValue(filters, filter)`         | A toggle filter's state, else its default, else `false`.                                          |
| `tagsValue(filters, filter)`           | The tags included and excluded, among the filter's options.                                      |

```ts
import { query, resolveUrl } from '@raygun/sdk';

export const address = `https://archive.org/advancedsearch.php?${query({
  q: 'collection:(librivoxaudio)',
  'fl[]': ['identifier', 'title'],
  rows: 30,
  output: 'json',
})}`;

// "https://www.gutenberg.org/cache/epub/84/images/plate1.jpg"
export const picture = resolveUrl(
  'https://www.gutenberg.org/cache/epub/84/pg84-images.html',
  'images/plate1.jpg',
);
```

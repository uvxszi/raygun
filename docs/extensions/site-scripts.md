# Site scripts: `site.scripts`

Site scripts improve websites people open in Raygun's websites browser (Browse → Sources →
Websites, Library → Websites): scripts and styles that Raygun adds to the pages of the sites an
extension names, like a browser's userscripts or content scripts. They can change how a site
looks and behaves, and they can **offer** what a page shows to Raygun: a video to play in
Raygun's player, a song to queue, save or download.

Nothing of a site-scripts extension runs in Raygun itself, and Raygun has nothing of any site
built in: everything site-specific is in extensions like these.

## How they run

- **In the page, with the page's privileges.** A script runs in the page's own JavaScript world,
  as if the page had one more script of its own. It can read and change the page, and make the
  requests the page could make, and nothing more.
- **Apart from Raygun.** The browser is a separate process on Android (`:web`, with its own
  cookies, storage and cache) and a separate window with its own browser profile on desktop.
  Pages there have no way into Raygun's own webview, its files, its settings or other extensions.
  A script can't reach Raygun's host API (`raygun.*`); it gets `raygunPage` instead.
- **Only data goes back.** `raygunPage.offer` hands Raygun an address, a title and a few words.
  The browser shows the offer; nothing happens until someone picks an action, and Raygun checks
  the offer again before acting on it: the extension must be installed and on, its patterns must
  match the page, and addresses must be `http(s)`.
- **The page sees it too.** Scripts share the page's world, so the page could call what they
  call: offers are only ever data that someone acts on, never code.
- **Where they run is shown before installing.** A repository lists each site-scripts
  extension's match patterns (`sites` in its index, written by `raygun-ext pack`), Raygun shows
  them before installing, and refuses a package whose patterns differ from what its repository
  lists.

People turn an extension's scripts on and off for each of their sites in the browser's menu and
in Settings → Browse → Websites; the extension's own switch (Browse → Sources) turns them off
everywhere.

## The manifest

A site-scripts extension has `"runtime": "site"`, provides `site.scripts` and nothing else, takes
no `permissions`, and its `entry` is one of its scripts. Its `site` section says where and how its
scripts run:

```json
{
  "id": "dev.example.tube-tidy",
  "name": "Tube tidy",
  "version": "1.0.0",
  "apiVersion": 1,
  "runtime": "site",
  "entry": "dist/tidy.js",
  "provides": ["site.scripts"],
  "site": {
    "scripts": [
      {
        "matches": ["https://www.example-tube.com/*", "https://*.example-tube.com/watch*"],
        "excludeMatches": ["https://www.example-tube.com/embed/*"],
        "js": ["dist/tidy.js"],
        "css": ["styles/tidy.css"],
        "runAt": "document-start",
        "allFrames": false
      }
    ]
  },
  "settings": [{ "key": "hideComments", "type": "boolean", "label": "Hide comments", "default": false }]
}
```

`site.scripts` holds 1 to 16 groups. Each group:

| Field            | Required | Meaning                                                                                          |
| ---------------- | -------- | ------------------------------------------------------------------------------------------------ |
| `matches`        | yes      | 1 to 32 match patterns: the pages the group runs on.                                             |
| `excludeMatches` | no       | Up to 32 patterns of pages it doesn't run on, even when `matches` names them.                    |
| `js`             | no       | Up to 16 scripts, run in order. Each `dist/<name>.js` is built from `src/<name>.ts` when there.  |
| `css`            | no       | Up to 16 style sheets, added at the start of the page.                                           |
| `runAt`          | no       | `document-start` (before the page's scripts), `document-end` (once parsed) or `document-idle` (once loaded; when left out). |
| `allFrames`      | no       | `true` runs it in frames too; the top page only when left out.                                   |

A group has at least one file. Paths are relative to the extension's folder, without `..`.

### Match patterns

A pattern is `<scheme>://<host><path>`, a subset of WebExtensions' match patterns:

| Part     | Takes                                                                                       |
| -------- | ------------------------------------------------------------------------------------------- |
| `scheme` | `https`, `http`, or `*` for both.                                                           |
| `host`   | `example.com`; `*.example.com` for it and every subdomain; `*` for every site.              |
| `path`   | Starts with `/`; `*` matches any characters. The query counts as part of the path.          |

A page's port and fragment don't count. Hosts are in lower case. `*://*/*` runs on every site:
people are told so before they install it.

Scripts run when a page starts; a site that changes its address without loading a new page
(most video sites do) keeps the scripts it started with, which watch for those changes
themselves.

## The script API: `raygunPage`

Each script runs inside a function Raygun wraps around it, with `raygunPage` in scope (it isn't a
global, so the page can't read the extension's settings). `import type {} from '@raygun/sdk/site'`
gives TypeScript its types.

```ts
import type { PageOffer } from '@raygun/sdk/site';

// The extension's settings as the person set them, else their defaults.
const hide = raygunPage.settings.hideComments === true;

// Where the browser is: off screen (Raygun in the background, the screen off, the window
// minimized), and on Android whether a call is on or anything plays sound.
if (raygunPage.state.background) console.log('playing off screen');
const stop = raygunPage.on('state', (state) => {
  if (state.call) console.log('a call took the sound');
});

// An offer: the browser shows it with what Raygun can do with it.
const offer: PageOffer = {
  kind: 'video',
  url: 'https://www.example-tube.com/watch?v=abc',
  title: 'A video',
  artist: 'Its channel',
  artworkUrl: 'https://img.example-tube.com/abc.jpg',
  durationMs: 212_000,
};
raygunPage.offer(offer);
// Taken back when the page moves on: one offer by its address, or all of them.
raygunPage.withdraw(offer.url);
raygunPage.withdraw();

export { hide, stop };
```

| Member              | What it is                                                                       |
| ------------------- | -------------------------------------------------------------------------------- |
| `extension`         | The extension's id.                                                              |
| `version`           | Its version.                                                                     |
| `platform`          | `android` or `desktop`.                                                          |
| `settings`          | The settings' values by key: text and choices are strings.                       |
| `state`             | `{ background, call, audioActive }`, as the browser said last.                   |
| `on('state', fn)`   | Calls `fn` with the new state whenever it changes; returns a function that stops it. |
| `offer(offer)`      | Offers something on the page to Raygun.                                          |
| `withdraw(url?)`    | Takes back the offer with this address, or all of this page's offers.            |

### Offers and what Raygun does with them

| Kind       | Play in Raygun                                   | Add to queue | Save to library | Download                                   |
| ---------- | ------------------------------------------------ | ------------ | --------------- | ------------------------------------------ |
| `video`    | YouTube's player for a YouTube video, else Open with's routing (a stream plays, a page goes to Search) | YouTube videos | –               | Desktop: the video (yt-dlp); phones: its sound as a song |
| `song`     | Raygun's music player (YouTube, else found by title and artist) | yes          | yes             | As a song                                  |
| `playlist` | Raygun's Search with its address (a YouTube playlist's card, a Spotify playlist's import) | –            | –               | –                                          |

Offers with the same address replace each other; the browser keeps a page's newest 20 and lists
the newest 5. On Android "Play in Raygun" pauses the page; a video or playlist then leaves the
browser for Raygun's own screen, and the site opens where it was left next time. Desktop lists
offers in the window's "In Raygun" menu (Windows).

## Background playback

Raygun's browser keeps a page that plays going while it's off screen: on Android with a media
notification and lock screen controls fed by the page's own `navigator.mediaSession` (its
metadata and action handlers) and media elements, on desktop with the window minimized (Settings
→ Browse → Websites → Keep playing in the background). The controls reach the page through its
media session handlers when it set them, else its playing media element.

Some sites pause themselves when they believe they're hidden or idle. That's site-specific, so it
belongs in a site-scripts extension: watch `raygunPage.state.background` and resume what the site
paused on its own, but not what the person paused (through the page, or the notification, which
calls the page's own `pause` handler) or what a call interrupted (`state.call`).

## Example: Wikipedia reading

A reading time under Wikipedia articles' titles and roomier text. It creates with:

```sh
pnpm raygun-ext init ../my-extensions/wikipedia-reading --kind site.scripts --id dev.example.wikipedia-reading --name "Wikipedia reading"
```

<!-- example: site.scripts dev.example.wikipedia-reading Wikipedia reading -->

```json
{
  "id": "dev.example.wikipedia-reading",
  "name": "Wikipedia reading",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "site",
  "entry": "dist/index.js",
  "description": "Wikipedia articles with a reading time and wider text.",
  "website": "https://en.wikipedia.org",
  "provides": ["site.scripts"],
  "site": {
    "scripts": [
      {
        "matches": ["https://en.wikipedia.org/wiki/*", "https://en.m.wikipedia.org/wiki/*"],
        "js": ["dist/index.js"],
        "css": ["styles/index.css"],
        "runAt": "document-end"
      }
    ]
  },
  "settings": [
    { "key": "readingTime", "type": "boolean", "label": "Show a reading time", "default": true },
    {
      "key": "wordsPerMinute",
      "type": "number",
      "label": "Words read in a minute",
      "default": 230
    }
  ]
}
```

```ts
// Wikipedia reading: site scripts (`site.scripts`) for Wikipedia's articles in Raygun's websites
// browser. They run in the page, with the page's own privileges and nothing of Raygun's: the
// manifest's `matches` say on which pages, `runAt` when (here once the article is parsed), and
// `styles/index.css` comes with them. `raygunPage` holds this extension's settings, the
// browser's state, and `offer`, which hands something on the page to Raygun as data.
//
// Try it: add en.wikipedia.org in Browse → Sources → Websites, then
// `pnpm exec raygun-ext test https://en.wikipedia.org/wiki/Ada_Lovelace` says what runs there,
// and `--bundle out.js` writes the script Raygun injects, to paste into a browser's console.
import type {} from '@raygun/sdk/site';

const { readingTime, wordsPerMinute } = raygunPage.settings;

/** The article's words, counted in its main text. */
const wordsIn = (element: Element) =>
  (element.textContent ?? '').split(/\s+/).filter((word) => word.length > 0).length;

const article = document.querySelector('#mw-content-text');
const heading = document.querySelector('#firstHeading');
if (readingTime !== false && article && heading) {
  const perMinute = typeof wordsPerMinute === 'number' && wordsPerMinute > 0 ? wordsPerMinute : 230;
  const minutes = Math.max(1, Math.round(wordsIn(article) / perMinute));
  const note = document.createElement('p');
  note.className = 'raygun-reading-time';
  note.textContent = `${minutes} min read`;
  heading.after(note);
}
```

The template's `tsconfig.json` adds the DOM's types (`"lib": ["ES2022", "DOM", "DOM.Iterable"]`),
and `styles/index.css` holds the styles.

## Try it

```sh
pnpm exec raygun-ext validate
pnpm exec raygun-ext test https://en.wikipedia.org/wiki/Ada_Lovelace --bundle out.js
```

`validate` builds the scripts, checks the manifest, the types and the files. `test` says which
groups run on the address and, with `--bundle`, writes the one script Raygun's browser injects
for the extension (its files, its settings, `raygunPage` and the pattern checks): paste it into a
desktop browser's console on that page to try it there. `--set key=value` tries other settings.

In Raygun, `raygun-ext dev --android` (or `--into <extensions folder>` on desktop) installs every
save; a site picks up its scripts the next time it opens.

## Publishing

`raygun-ext pack` puts the manifest, the scripts and the styles in the package and lists the
match patterns as the entry's `sites` in the repository's `index.json`. Without a repository, a
package's address works too: Browse → Sources → Add with the address of its `.zip` installs it
after showing where its scripts run.

When people add a website whose address one of their site-scripts extensions matches, Raygun
says so; when they open a site that an extension in one of their repositories matches, the
browser offers to install it (only from repositories they added).

### Raygun's own

Raygun's repository folder (`repo/`) has site scripts that Paraline, which lists only open
sources, doesn't:

| Extension                                 | Sites                                       | Adapted from                                       |
| ----------------------------------------- | ------------------------------------------- | -------------------------------------------------- |
| YouTube enhancements (`dev.raygun.noutube`) | www, m and music.youtube.com              | [NouTube](https://github.com/nonbili/NouTube) (AGPL-3.0) |

It blocks ads, keeps playing in the background, skips sponsored segments (SponsorBlock), plays
the original audio track, hides Shorts and more, each a setting, and offers the video or song
shown. Install it with Browse → Sources → Add and its package's address:
`https://raw.githubusercontent.com/uvxszi/raygun/main/repo/packages/dev.raygun.noutube-1.0.0.zip`
(a newer version has its number in the name). An extension added this way gets no updates from a
repository; adding the newer package's address updates it.

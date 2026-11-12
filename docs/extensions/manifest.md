# The manifest: `raygun-extension.json`

Every extension has a manifest at the top of its folder. Raygun reads it before anything else and
refuses an extension whose manifest breaks a rule below, **including any field it doesn't know**,
so a typo such as `"permission"` keeps the extension from loading. `raygun-ext validate` checks
every rule.

```json
{
  "id": "dev.example.peppercarrot",
  "name": "Pepper&Carrot",
  "version": "1.0.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Pepper&Carrot, David Revoy's webcomic, free under CC BY 4.0.",
  "author": "Example",
  "website": "https://www.peppercarrot.com",
  "provides": ["manga.source"],
  "permissions": { "net": ["www.peppercarrot.com"] },
  "contentRating": "safe",
  "settings": [
    {
      "key": "language",
      "type": "select",
      "label": "Language",
      "options": ["English", "French"],
      "default": "English"
    }
  ]
}
```

## Fields

| Field           | Required | What it is                                                                                     |
| --------------- | -------- | ---------------------------------------------------------------------------------------------- |
| `id`            | yes      | Reverse-DNS in lower case: two or more labels of `a-z`, `0-9` and `-`, at most 100 characters. |
| `name`          | yes      | What Raygun shows: 1–80 characters.                                                            |
| `version`       | yes      | Semver (`1.4.0`, `2.0.0-beta.1`). Every published change needs a new version.                  |
| `apiVersion`    | yes      | `1`.                                                                                           |
| `runtime`       | yes      | `"js"`, or `"site"` for [site scripts](site-scripts.md).                                       |
| `entry`         | yes      | The bundle, relative to the folder: `dist/index.js`. A `.js` path without `..` or `\`.         |
| `provides`      | yes      | Every provider kind the extension registers.                                                   |
| `description`   | no       | One or two sentences, shown in the extension lists and before installing.                      |
| `author`        | no       | Who made it.                                                                                   |
| `website`       | no       | The site the sources read (`https://`): each source's menu opens it in Raygun's browser.       |
| `permissions`   | no       | `{ "net": [...] }`: the hosts the extension may reach.                                         |
| `contentRating` | no       | `safe` (when left out), `suggestive` or `nsfw`.                                                |
| `settings`      | no       | Settings people can change on the extension's page.                                            |
| `site`          | for `site` | Where and how [site scripts](site-scripts.md) run: their match patterns, files and timing.  |

### `id`

The id is the extension's identity: updates replace the installed copy with the same id, and an
installed copy overrides a built-in one. Start it with a domain or a name you own, and never
change it. Providers are known as `<id>/<provider id>`.

### `provides`

The kinds of every provider the script registers: `music.metadata`, `music.stream`, `lyrics`,
`music.charts`, `music.tags`, `music.info`, `music.recommend`, `manga.source`, `novel.source`,
`video.source`, `video.streams`, `podcast.directory`, `audiobook.directory`. Raygun refuses to
register a provider whose kind isn't listed.

### `permissions.net`

The hosts `raygun.http.fetch` may reach. A request elsewhere, or a redirect elsewhere, fails with
"network access to … is not permitted by the extension's manifest".

| Pattern           | Reaches                                                               |
| ----------------- | --------------------------------------------------------------------- |
| `api.example.org` | That host only.                                                       |
| `*.example.org`   | Its subdomains (`a.example.org`, `a.b.example.org`), not itself.      |
| `*`               | Any public host; never this device or the local network.             |

List `example.org` and `*.example.org` for both. Addresses that Raygun fetches itself need no
permission: covers, pages, streams, subtitles and feeds an answer names. Raygun shows the list
before the extension is installed, and `*` as "Any public website".

`permissions.webview` is reserved and has no effect; leave it out.

### `contentRating`

- `safe`: for everyone.
- `suggestive`: some mature themes; shown like `safe`.
- `nsfw`: for adults. Raygun hides the extension's providers while NSFW content is off, and those
  of a music extension while Explicit music is off.

People can mark any extension, or a whole repository, 18+ or not themselves; their mark decides.

### `settings`

Each setting is an object with:

| Field      | Required        | What it is                                                                       |
| ---------- | --------------- | -------------------------------------------------------------------------------- |
| `key`      | yes             | Unique, not empty; what `raygun.settings.get` takes.                             |
| `type`     | yes             | `text`, `number`, `boolean` or `select`.                                         |
| `label`    | no              | What Raygun shows; `itemsPerSearch` reads "Items per search" without one.        |
| `options`  | for a `select`  | The choices, as text.                                                            |
| `default`  | no              | A string, a number, a boolean, or one of a select's options.                     |
| `required` | no              | `true` for a key, a secret, an address or a user name it can't work without.     |

Raygun draws the settings on the extension's page and keeps people's values across updates;
`raygun.settings.get(key)` gives the value, else the default, else `null`. A value that no longer
fits (a choice the extension dropped) reads as the default. Changing a setting reloads the
extension.

While a `required` setting is empty (no value, or only spaces), the extension needs setup:
Raygun calls none of its providers and sends none of its requests, leaves it out of searches and
lookups without counting it as failing, and lists it under one line, "N sources need a key or
sign-in", whose entries open its settings. A key the extension's own API refuses is said with
[`needsSetup`](host-api.md#raygunneedssetupreason), which does the same until the settings
change.

```json
{
  "settings": [
    { "key": "apiKey", "type": "text", "label": "API key", "required": true },
    { "key": "apiSecret", "type": "text", "label": "API secret", "required": true }
  ]
}
```

## What Raygun writes

Raygun's Nuclear converter writes a `nuclear` field into the manifests it makes; leave it out of
your own.

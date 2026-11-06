# Repositories

A repository is a folder on the web with an `index.json` and the packages it lists. People add it
to Raygun once, then install its extensions and get their updates. Raygun starts with its own,
Paraline; anyone can publish another.

## Make one

Put your extensions side by side in one folder, then pack them all into the repository folder,
from the Raygun checkout's root:

```sh
pnpm raygun-ext pack-all ../my-extensions --repo ../my-repo --name "My sources"
```

or one at a time, from an extension's folder:

```sh
pnpm exec raygun-ext pack --repo ../../my-repo
```

For each extension, `pack`:

1. validates it, and stops at errors (an extension Raygun would refuse is never published);
2. builds it and zips its manifest and bundle into `packages/<id>-<version>.zip`;
3. copies its `icon.png` (or `icon.webp`, `icon.jpg`), when it has one, to `icons/<id>.png`;
4. rewrites `index.json` from the packages: the newest version of each extension.

Packages are reproducible: the same sources give the same bytes. A published version never
changes, because installs check its checksum: when the code changed under a version that's
already in `packages/`, `pack` keeps the published package and says so. Bump the version to
publish the change. Older packages stay, for anyone who linked them.

`--name` names the repository; the index keeps its name afterwards (a new one is named after its
folder).

## `index.json`

```json
{
  "formatVersion": 1,
  "name": "My sources",
  "extensions": [
    {
      "id": "dev.example.peppercarrot",
      "name": "Pepper&Carrot",
      "version": "1.0.0",
      "apiVersion": 1,
      "description": "Pepper&Carrot, David Revoy's webcomic, free under CC BY 4.0.",
      "author": null,
      "provides": ["manga.source"],
      "permissions": { "net": ["www.peppercarrot.com"], "webview": false },
      "contentRating": "safe",
      "package": "packages/dev.example.peppercarrot-1.0.0.zip",
      "icon": "icons/dev.example.peppercarrot.png",
      "sha256": "5dee8c9444cf12a84ee5a79848d754730c6fd19c1a75ac57d7c99c9dbc88d745",
      "size": 4355
    }
  ]
}
```

| Field                    | What it is                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------ |
| `formatVersion`          | `1`.                                                                                       |
| `name`                   | The repository's name in Raygun.                                                           |
| `extensions[]`           | One entry per extension, from its manifest: `id`, `name`, `version`, `apiVersion`, `description`, `author`, `provides`, `permissions`, `contentRating`. |
| `package`                | The zip, relative to the index or an absolute `https` address.                            |
| `icon`                   | An icon, relative to the index or absolute; optional.                                     |
| `sha256`, `size`         | The zip's SHA-256 (lower-case hex) and its size in bytes.                                  |

Raygun leaves out entries it can't use (another `apiVersion`, a malformed id, version or
checksum) and shows the rest. Before installing, it checks the zip's checksum, its size (10 MB at
most), its paths, and that its manifest has the entry's id, version and permissions: a package
whose permissions differ from the ones listed isn't installed. `raygun-ext pack` writes every
field, so the index needs no editing.

An index may also name packages in other repositories by their absolute address, and mix
packages of the [other formats](other-formats.md) Raygun runs (a `format` field of `nuclear`,
`shosetsu` or `miru`); Paraline does both.

Stremio addons go in a list of their own, `addons`, so versions of Raygun before 1.3.0 skip them.
Each is an entry as above whose `package` is the addon's `https` manifest address (ending in
`/manifest.json`), with `sha256` empty and `size` 0. Installing one adds the addon, and
uninstalling it removes the addon:

```json
"addons": [
  {
    "id": "org.stremio.opensubtitlesv3",
    "name": "OpenSubtitles v3",
    "version": "1.0.0",
    "apiVersion": 1,
    "provides": ["stremio.subtitles"],
    "package": "https://opensubtitles-v3.strem.io/manifest.json",
    "sha256": "",
    "size": 0
  }
]
```

## Host it

Any `https` server works. On GitHub, push the repository folder to a public repository; its raw
address is the index's address:

```text
https://raw.githubusercontent.com/<owner>/<repository>/<branch>/index.json
```

or, with the repository folder inside the project, `…/<branch>/repo/index.json`. jsDelivr
(`https://cdn.jsdelivr.net/gh/<owner>/<repository>@<branch>/index.json`) and GitLab's raw
addresses work too. Plain `http` is accepted only for `localhost` and the Android emulator's
`10.0.2.2`, for testing.

## Add it in Raygun

Browse → ⋯ → Extension repositories → Add repository, then the index's address. Its extensions
appear in Browse → Extensions, with their network access shown before installing, and updates
appear there once a newer version is in the index.

On desktop, a repository can also be an index on the computer itself: its path, or a `file:`
address. That's the quickest way to try a repository before publishing it.

## Publishing changes

1. Change the code, and bump `version` in the manifest.
2. `pack` (or `pack-all`) again.
3. Commit and push the repository folder: the new package, its icon and `index.json`.

People with the extension installed see the update in Raygun.

## Content

What a repository's sources reach is its author's responsibility; see the
[content rules](README.md#content-rules). Paraline, Raygun's own repository, accepts only sources
of public-domain or openly licensed content.

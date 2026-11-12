# Kodi add-ons

Raygun reads Kodi repositories and runs their video and music plugins
(`xbmc.python.pluginsource`): their folders become Raygun's lists, their videos and songs play in
Raygun's players. Plugins are Python 3 programs written against Kodi's own modules; Raygun runs
them unchanged in its own Python, with Kodi's modules rebuilt over Raygun.

Code: `crates/raygun-ext-host/src/kodi/` (`index` repositories, `package` installs, `addon`
settings and strings, `python` the interpreter and its sandbox, `lib/` the Python side,
`catalog` the mapping, `runtime` the extension).

## Adding a repository

Browse → Sources → Add (or Extensions → Repositories) with the repository's address. Any of
these works:

- its `addons.xml`, or `addons.xml.gz` (a large `addons.xml` is read from the `.gz` beside it);
- the repository's own add-on: its `addon.xml`, or the `repository.*.zip` people share. Raygun
  reads the index its `xbmc.addon.repository` extension names (`<info>`).

Video plugins provide a video source; music plugins (radio, podcasts, songs) provide music; a
plugin providing both provides both. Skins, services, scripts, picture plugins and the like are
listed as not for Raygun, saying what they do in Kodi; libraries (`script.module.*`), resources
and repositories are left out. A plugin's zip shared on its own is refused with a pointer to its
repository, which has what it needs.

An index may list one add-on several times: once for each system a compiled add-on is built for
(`<platform>android-aarch64</platform>`, its zip in a folder of its own named by `<path>`,
`pvr.example+android-aarch64/…`, its pictures beside it), sometimes in several versions. Raygun
lists each add-on once (`kodi::index::one_each`), as Kodi on this device would install it: made
for this system (`all`, the system, or the system and processor, as Kodi names them), then one
its Python runs (Python 3), then the highest version (Kodi's version order: `~` before a
release, an `n:` epoch first). Libraries are chosen the same way. A source made only for other
systems is listed as not for Raygun ("Made for Kodi on Android").

## Installing

`kodi::package::assemble` reads the plugin's zip and installs, beside it, the libraries its
`addon.xml` imports (`<requires>`), and theirs, from its own repository first, then Kodi's own
(`mirrors.kodi.tv/addons/omega`), as Kodi does. The extension's folder holds `addon/` (the
plugin), `lib/<id>/` (each library), `kodi.json` (what was installed, the plugin's settings and
its libraries' folders) and a generated `raygun-extension.json` (runtime `kodi`, network: any
public host, the plugin's settings).

Some of what plugins import Raygun answers itself, so it isn't installed:

| Import | In Raygun |
|---|---|
| `xbmc.python`, `xbmc.*`, `kodi.*` | Kodi's own parts: Raygun's `xbmc`, `xbmcgui`, `xbmcplugin`, `xbmcaddon`, `xbmcvfs`, `xbmcdrm` |
| `script.module.requests`, `script.module.urllib3` | Raygun's `requests` and `urllib3`, over its own HTTP |
| `script.module.inputstreamhelper`, `inputstream.adaptive`, `inputstream.ffmpegdirect` | Raygun's player opens HLS and DASH itself; DRM is refused |
| `script.common.plugin.cache` | Raygun's `StorageServer`, a cache in the plugin's folder |
| `plugin.video.youtube` | Its `plugin://plugin.video.youtube/play/?video_id=…` addresses play in YouTube's player |
| `resource.*`, `service.*`, `context.*`, `pvr.*`, `skin.*` | Gone without (pictures, background services, menus) |

Refused, saying why: a plugin written for Python 2 (`xbmc.python` before 3.0, Kodi 18 and
older), and one needing a library with compiled code (Pillow, PyCryptodome, lxml: any `.so`,
`.pyd` or `.dll`). Another plugin a plugin hands items to (`plugin.video.vimeo`...) isn't run from
inside it: those items say so.

## Running

### The interpreter: RustPython

Plugins run in [RustPython](https://github.com/RustPython/RustPython) 0.6 (MIT), a Python 3
interpreter written in Rust, compiled into Raygun.

| Choice | Size | Compatibility | Sandbox | Effort |
|---|---|---|---|---|
| **RustPython 0.6** (chosen) | Compiled in: about 20 MB of code on Windows as release builds make it (size-optimized, with LTO), plus a 1.8 MB zip of the standard library's Python half | Current Python 3 syntax, most of the standard library (its native half in Rust); pure-Python libraries run | By construction: built without `host_env`, the interpreter has no files, sockets, processes or environment of its own | Cargo builds it for Android and Windows like any crate |
| CPython through PyO3 | libpython and its standard library for each ABI, downloaded on demand | Everything, C extensions included | None in-process: a plugin could read Raygun's database and its secrets; audit hooks aren't a boundary | A loader per platform, prebuilt runtimes to host, pin and update |
| CPython on WASI (wasmtime) | A WebAssembly engine compiled in, CPython for WASI downloaded | Pure Python only (as here) | Strong (WASI) | No sockets in WASI: HTTP through a host call anyway; a JIT on Android |

RustPython wins on the sandbox (nothing reaches the device unless Raygun hands it over), on
building (no prebuilt libraries per ABI to host and update) and on being one code path on every
platform. It's slower than CPython, which matters little: plugins mostly wait on the network
(a listing takes 0.1–2 s, about what the requests take). Interpreter code can't be downloaded on
demand like the voice engine's model (stores forbid apps loading code they download, and Raygun keeps
one build for every store), so it's compiled in, and the release profile builds it for size
(`opt-level = "s"`).

Two things are pinned for it: `third_party/rustpython-host_env` (its `wmi` module didn't build on
Windows without `ctypes`, see `third_party/README.md`), and the 0.16.5 parser crates and
malachite 0.12 its 0.6.0 release was built with (`Cargo.toml`, `Cargo.lock`).

### The sandbox

Each plugin has its own interpreter on its own thread, started at the first call and let go after
five minutes without calls. It sees Kodi's paths, mapped by `python::files`:

| Kodi path | What it is |
|---|---|
| `special://home/addons/<id>/` | The plugin and its libraries, read-only |
| `special://profile/` (`/kodi/home/userdata/`) | The plugin's own writable folder (its `addon_data`), 256 MB at most |
| `special://temp/` | Its temporary folder |
| `special://xbmc/` | Nothing |

`os`, `open()`, `io`, `xbmcvfs`, `shutil`, `tempfile` and `sqlite3` (over the SQLite Raygun
links; `ATTACH` and `VACUUM INTO` refused) all go through these. Paths are resolved lexically and
each part checked, so none leaves its folder.

The network goes through `_raygun.fetch` (`python::net`): Raygun's HTTP client, public hosts
only (names resolved by Raygun, so no address on the device or the local network), Offline Mode
honoured, bodies capped at 32 MB. `http.client`'s connections are replaced as it's imported, so
`urllib.request` works unchanged; `requests` and `urllib3` are Raygun's over the same call;
`socket`, `ssl`, `select`, `subprocess` and `signal` are stand-ins that can't connect or start
anything. Cookies stay where the plugin keeps them (a `requests` session, `http.cookiejar`), as in
Kodi.

Each run has 45 seconds: waits on the network and `time.sleep` end there, and a run that loops is
interrupted from outside; between runs, threads a plugin left running get no network and no waits. A call (a search that runs the plugin twice, a stream behind a choice)
has 75. Memory isn't capped per plugin: RustPython has no allocator hook, so a plugin that
allocates without end could take the app with it. Plugins from Kodi's own repository are reviewed
by Kodi's team; others are as trustworthy as their repository.

### Kodi's modules

`lib/xbmc*.py` rebuild what plugins use: list items with their labels, art, info labels (both
`setInfo` and Kodi 20's `InfoTag` setters), properties, paths, MIME types and subtitles;
`addDirectoryItem(s)`, `endOfDirectory`, `setResolvedUrl`, `setContent`, `setPluginCategory`;
`xbmcaddon.Addon` with its settings (Raygun's values, else the plugin's defaults; what the plugin
sets itself is kept in its data folder) and its `strings.po`; `xbmcvfs` (`translatePath`, files,
folders); `xbmc.log`, `getInfoLabel`, `getCondVisibility`, `executebuiltin`,
`executeJSONRPC` (the queries plugins make about Kodi and its add-ons), `Keyboard`, `Player`,
`PlayList` (`.pls` and `.m3u` files too), `Monitor`; dialogs answered without a screen: a keyboard or
input with the words searched for, a choice by the server picked (else the first), a yes/no question
with no, notifications and messages kept for the error a failed listing shows.

A run starts the way Kodi starts one: `sys.argv` is the `plugin://` address, a handle, the query
and `resume:false`; the plugin's own modules (and its libraries') are imported afresh each run, so
module-level state is as new as in Kodi.

## How plugins map onto Raygun

`catalog` turns what a plugin lists into Raygun's calls; ids are the plugin's own addresses
(`/?mode=list&page=2`), so a title kept in the library opens the same folder later.

**Video plugins** (`video.source`):

- *Popular* is the plugin's top folder. Its folders are also the *Section* filter, and a folder
  opens as a list of its own (the source page with `folder=`), so the whole tree is browsable.
- *Search* opens the plugin's own search entry (`Search` in its language, or an address with
  `search`), answering its keyboard (`xbmc.Keyboard`, `Dialog().input`) with the words; a plugin
  that sends Kodi to its results (`Container.Update`) is followed, a "New search" menu entered.
- A title page is a folder: its videos are episodes (numbers, seasons and dates from their info
  labels); a folder holding only folders shows their videos as seasons.
- "Next page" entries (by their label in the usual languages, or the same address with a bigger
  page number) become further pages.
- An episode's streams are what the plugin resolves (`setResolvedUrl`) or plays (`Player().play`,
  `PlayMedia`). Kodi's `url|Header=value&…` headers, `inputstream.adaptive`'s manifest type and
  headers, MIME types and `.m3u8`/`.mpd` addresses say what to play; subtitles come along. When a
  plugin asks which stream (`Dialog().select`), each choice is a server. DRM-protected streams are
  refused, saying so.

**Music plugins** (`music.metadata` and `music.stream`):

- Their folders are playlists (Search → Music → Playlists, *Featured*); songs or stations listed
  at the top are a playlist named after the plugin.
- A song whose address is a stream plays at once; one the plugin resolves plays through the
  plugin's own `music.stream` provider, which answers only for songs it listed (no run needed to
  find them) and resolves them when played.
- Search runs the plugin's search, as for videos.

## In Kodi's own repository

`kodi_official_survey` (November 2026, Kodi's Omega repository, from a desktop in Europe): of its
236 video and music plugins,

| Outcome | Plugins |
|---|---|
| Installed and listed their top folder | 145 (61%) |
| Installed, listed nothing (an account, a device on the network or a region needed first) | 23 (10%) |
| Installed, listing failed | 61 (26%) |
| Not installed (Pillow, or a library that wouldn't download) | 7 (3%) |

Most failures are the plugins' own: sites gone or changed since the plugin was last updated,
sign-ins and device codes, region locks, HTTP and TLS errors from their servers. Some are Raygun's
to look at: a line RustPython's parser refuses (Audiobookshelf), XML namespaces read through
`xmltodict`, terms a plugin asks to accept (answered no), one site's HTTP/2 error. The survey only
lists top folders; media.ccc.de, SomaFM and the Internet Archive are checked further
(browse, search, play).
## What doesn't work

- DRM (Widevine): Raygun's player has none.
- Plugins whose zips hold compiled code, or that import libraries that do (Pillow,
  PyCryptodome, lxml, NumPy).
- Kodi's own windows (`WindowXML`, PyXBMCt dialogs): they draw nothing; plugins that need them to
  sign in can't be set up.
- Items a plugin hands to another plugin (other than YouTube's addresses).
- Settings that are actions (sign-in buttons, "clear cache"), and settings depending on others.
- Live TV through PVR clients and IPTV Manager.
- Questions a plugin asks before it starts (accepting its terms, signing in with a code shown on
  screen): yes/no questions are answered no, so such a plugin lists nothing until it can be set up
  through its settings.

## Checking

```sh
cargo test -p raygun-ext-host kodi::                                       # fixtures: a plugin served locally, settings, sandbox, SQLite
cargo test -p raygun-ext-host -- --ignored kodi_official_live --nocapture  # Kodi's repository, read
cargo test -p raygun-ext-host -- --ignored kodi_media_ccc_live --nocapture # media.ccc.de: browse conferences, play a talk
cargo test -p raygun-ext-host -- --ignored kodi_somafm_live --nocapture    # SomaFM: stations, one station's stream
cargo test -p raygun-ext-host -- --ignored kodi_archive_org_live --nocapture # Internet Archive: collections, search
cargo test -p raygun-ext-host -- --ignored kodi_official_survey --nocapture # every video and music plugin of Kodi's repository
```

`kodi_official_survey` installs every video and music plugin of Kodi's repository and lists its
top folder (`KODI_SURVEY=plugin.a,plugin.b` narrows it).

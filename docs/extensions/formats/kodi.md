# Kodi add-ons

Raygun reads Kodi repositories and lists their plugins, but can't run them: Kodi's add-ons are
Python programs.

## Adding a repository

Browse → Extensions → Repositories → add the repository's address. Any of these works:

- its `addons.xml`, or `addons.xml.gz` (a large `addons.xml` is read from the `.gz` beside it);
- the repository's own add-on: its `addon.xml`, or the `repository.*.zip` people share. Raygun
  reads the index its `xbmc.addon.repository` extension names (`<info>`).

The repository's plugins are listed with what they provide (video, music, pictures), their
version, author and summary. Skins, services, scripts and the `script.module.*` libraries aren't
content and are left out. Each plugin is marked **Can't install**.

## Why they can't run

A Kodi plugin is a Python 3 program started with a `plugin://` address. It answers by calling
Kodi's own modules: `xbmcplugin.addDirectoryItem` to list folders and videos,
`xbmcplugin.setResolvedUrl` to hand over a stream, `xbmcgui.ListItem` for each item's details,
`xbmcaddon.Addon` for its settings and strings, `xbmcvfs` for files, `xbmc` for logging and
playback. Most plugins also import `script.module.*` libraries (requests, BeautifulSoup,
inputstreamhelper, resolveurl) that are other add-ons, some with C extensions.

Running a useful share of them would take:

1. **A Python interpreter on Android and Windows.** RustPython is pure Rust and builds for both,
   but adds several megabytes, runs Python an order of magnitude slower than CPython, and its
   standard library has gaps (`ssl` and parts of `socket`, which `requests` needs). Embedding
   CPython instead means shipping its runtime and standard library for each Android ABI.
2. **The `xbmc*` modules**, rebuilt over Raygun: about 60 functions and classes used commonly
   (directory items, list items with their info labels, settings, localized strings, dialogs,
   the `special://` paths), mapping a plugin's folder tree onto `video.source` titles and episodes.
3. **The `script.module.*` dependencies**, installed from the same repositories, and the
   pure-Python ones only.
4. **Network rules** like the other runtimes': which hosts a plugin may reach, timeouts, and no
   access to the local network or the device's files.

Until then the repositories are readable and their plugins findable, so nothing about a Kodi
repository is a mystery when it's added.

## Checking

`kodi_official_live` reads Kodi's own repository (`mirrors.kodi.tv`), gzipped:

```sh
cargo test -p raygun-ext-host -- --ignored kodi_official_live --nocapture
```

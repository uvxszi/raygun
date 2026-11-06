# Mihon extensions

Raygun reads Mihon (Tachiyomi) extension stores and converts their extensions from source code
into Raygun extensions of runtime `mihon`, which run in Raygun's own JavaScript runtime on
Android and Windows alike. No APK is loaded: a store's APKs are compiled Android bytecode, but the
code they're built from is public (Keiyoushi's is `keiyoushi/extensions-source`, Apache-2.0), and
most of it is a theme plus a few values.

## Adding a store

Browse → Extensions → Repositories → add the store's address, as Mihon takes it:

- a legacy list, `index.min.json` (`[{ name, pkg, apk, lang, code, version, nsfw, sources }]`),
  with `repo.json` beside it;
- `repo.json` itself (`{ meta: { name, signingKeyFingerprint, … }, index_v2 }`);
- a store, `index_v2`: protobuf (`index.pb`, often gzipped) or the same as JSON.

Raygun follows a list to its repository's newer index, as Mihon does. Keiyoushi's
`https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json` lists only a
"update your app" placeholder now; its `repo.json` names `index.pb`, which lists 1463
extensions. Each becomes a catalogue entry `mihon.<lang>.<name>` (from its package name
`eu.kanade.tachiyomi.extension.<lang>.<name>`), with:

- its version name as the version (`1.6.5`: the extensions-lib version and the version code), so
  updates go by the code as in Mihon;
- its content warning as the rating (`NSFW` → `nsfw`, hidden while NSFW content is off; `MIXED`
  → `suggestive`);
- `net` limited to its sources' hosts and their subdomains;
- one provider per source (an extension often has one per language), with Mihon's source id as
  the provider id.

Anime extensions (Aniyomi's `animeextension` packages) are left out; a store of only those is
refused with the reason. Code: `raygun-ext-host::mihon::index` (`detect`, `load`,
`to_repo_index`, `find`).

## Converting

Installing an entry reads its code (`mihon::source`): the folder is named by its icon's address
(`cdn.jsdelivr.net/gh/<owner>/<repo>@<ref>/src/<lang>/<name>/res/…`), else by the layout
Keiyoushi and its forks share (`src/<lang>/<name>` of the store repository's `-source` twin).
One GitHub API call lists the folder (`git/trees/<ref>:<folder>`); the build file and the Kotlin
files are then read from raw.githubusercontent.com. Without the API (60 calls an hour without a
sign-in) the main file is looked for by the names its class takes.

The build file (`build.gradle.kts`, or the older `build.gradle`) names the theme. A Kotlin parser
(`mihon::kotlin`: lexer, parser, syntax tree) reads the source class, and `mihon::convert` turns
it into the values its theme takes:

```kotlin
@Source
abstract class ComicsValley : Madara() {
    override val mangaSubString = "comics-new"
    override val chapterMode = ChapterMode.MangaAjax
    override val chapterDateFormat = DateTimeFormatter.ofPattern("dd/MM/yyyy", Locale.ROOT)
}
```

becomes

```js
mihon.theme("madara", {
  variant: "ajax",
  sources: [{ id: "1103204227230640533", name: "Comics Valley", lang: "all", baseUrl: "https://comicsvalley.com" }],
  overrides: {
    mangaSubString: (t) => "comics-new",
    chapterMode: (t) => "MangaAjax",
    chapterDateFormat: (t) => K.dateFormat("dd/MM/yyyy", K.locale("root")),
  },
});
```

A value may use string templates over the source's `baseUrl`, `lang` and `name`, the class's
constants, the theme's own value (`super.seriesGenreSelector + ", .x"`), another of its values
(`archiveSelector()`), its messages (`intl["…"]`), lists and pairs, locales, date formats,
regular expressions and `when (lang)`. `configureClient` may set a rate limit (and timeouts),
`configureHeaders` plain headers. Overrides that change nothing Raygun does (related titles, the
WebView's address, settings screens) are skipped. Older builds' constructor arguments
(`Madara("Name", "https://…", "en", SimpleDateFormat(…))`) are read too.

### Sources without a theme

Most of Keiyoushi's other extensions extend `KeiSource` directly, with `suspend` functions
(`getPopularManga`, `getSearchMangaList`, `fetchMangaUpdate`, `getPageList`…) over
`client.get(url, Fetch.DOCUMENT).asJsoup()` and `parseAs<T>()`. `mihon::transpile` turns the
source class, and the classes its files declare (DTOs, filters, enums), into JavaScript
(`mihon::kotlin::emit`):

```js
mihon.source({
  sources: [{ id: "4242", name: "Fixture Stand", lang: "en", baseUrl: "https://stand.example" }],
  renames: { thumbnail: "cover_url" },
  build: (self, C, D) => {
    self.define("getPageList", [[1, 1, async ($chapter) => { … }]]);
    C.prop("DATE_FORMAT", async () => KT.call("SimpleDateFormat", ["yyyy-MM-dd", …]));
    D.MangaDto = KT.defineClass({ name: "MangaDto", params: ["slug", "title"], … });
  },
});
```

Every call is awaited, as a `suspend` function's would be. Member calls go through
`mihon/kotlin.js` (`KT`), which knows at run time what a value is: Kotlin's standard library on
strings, lists, maps and numbers, Jsoup on documents, OkHttp's requests, URLs and responses,
kotlinx.serialization (JSON decoded into the declared classes, `@SerialName` included),
coroutines (`async`/`awaitAll`, `Mutex`, `Semaphore`), `Calendar`, `java.time` dates and zones.
A local shadowing a parameter (`val page = page + 1`) gets a name of its own, and default
arguments are worked out in the body, in order.

Only what Raygun calls has to convert: the functions above and whatever they reach by name.
Unreached code that can't be converted becomes a stub that fails with its reason if it's ever
called. Settings screens aren't converted: preferences read their defaults.

### Themes from their code

A theme Raygun has no port of (and an extension Raygun's port of its theme refuses) converts
with the theme's own code: installing reads `lib-multisrc/<theme>` beside the extension's folder
(`source::fetch_theme`), and the theme's classes become layers beneath the source's, as Kotlin
inherits them (`mihon.source({ layers: [theme, source] })`: a call finds the topmost layer that
has it, `super` the layers beneath). The theme's other source classes (`MadaraNoAjax` beside
`Madara`) are left out. A theme's build may name only `lib:i18n`, whose messages
(`assets/i18n/messages_<lang>.properties`) come with the converted code for `Intl`.

Two themes' pieces are written by Raygun instead of converted:

- ComiciViewer's and GigaViewer's `ImageInterceptor`, which puts a page's shuffled tiles back
  together: the pages' addresses carry the recipe (`#raygun-unscramble=comici:<tiles>`,
  `#raygun-unscramble=giga`) and Raygun's image fetcher reorders the tiles
  (`raygun_net::images::Unscramble`);
- Madara's `decryptChapterData` (its chapter protector): the runtime's AES in Rust.

### Requests, cookies and filters

Interceptors run as OkHttp's do (`addInterceptor`, `addNetworkInterceptor`, an `Interceptor`
class or lambda): each gets the chain's request, may change it (`newBuilder().header(…)`), calls
`proceed` and may change the answer (`response.newBuilder()…`, `toResponseBody`). Keiyoushi's
`addCookie(…)` adds cookies to the source's requests, and its `GraphQLErrorInterceptor`,
`graphQLBody` and `parseGraphQLAs` are the runtime's. An interceptor that draws pictures
(`BitmapFactory`) is refused, as there are none to draw.

A source's `getFilterList()` is converted when it can be: its `Select`, `Sort`, `CheckBox`,
`TriState` and `Group` filters become Raygun's select, toggle and tag filters (keys `f0`, `f1`…
by place), and a search sets their states before `getSearchMangaList` runs. Text filters
aren't shown. A filter list that can't be converted leaves the source without filters. Madara
and MangaThemesia, run by hand, have their standard sets (status, order, type, genres read from
the site).

### Refusals

Anything else is refused with the reason, which the catalogue then shows for that version
("Raygun can't convert it: its code uses proceed(), which Raygun's converter doesn't read yet"):

- an interceptor drawing pictures (image descrambling other than the two recipes above);
- one of Keiyoushi's libraries (`lib:speedbinb`, `lib:unpacker`, `lib:textinterceptor`…);
- a call, object or name the runtime doesn't have (`emit::METHODS`, `GLOBALS` and `OBJECTS`,
  which a test keeps equal to `kotlin.js`'s tables), such as Next.js page data
  (`extractNextJs`), a cookie jar read in code (`loadForRequest`), `textNodes()` or WebViews;
- what JavaScript can't say: `break` or `continue` out of a lambda, a vararg before other
  parameters;
- code that runs when the source starts (`init`).

Before the package is installed it's loaded once in a host of its own (`convert::validate`):
its script must run and register every source. Its `net` is the sources' hosts and their
subdomains, plus the hosts its code names in full addresses (an API beside the site, an image
server). The manifest keeps `mihon: { pkg, versionCode,
converter, theme, source }`; when `CONVERTER_VERSION` grows, the catalogue offers the installed
extensions again, as it does for Nuclear conversions.

## The runtime

`runtime = "mihon"` runs the converted script after `mihon/prelude.js`, `mihon/kotlin.js` and
the themes:

| Mihon leaned on | In Raygun |
|---|---|
| Jsoup | Documents parsed in Rust (the LNReader runtime's document store), Raygun's Jsoup-compatible selectors (`:contains`, `:has`, `:eq`…), Jsoup's `text()` and `ownText()` whitespace rules, `attr("abs:…")`, siblings and parents |
| OkHttp, `KeiSource`'s client | `raygun.http` with the source's Referer and headers, the accept headers of a page load or an XHR, form bodies, its rate limit; Raygun's own limits beneath (6 requests at once per site, 32 in all) and the extension's cookie jar |
| Java's dates | `SimpleDateFormat`/`DateTimeFormatter` patterns with their zones, month names in 20 languages, Thai Buddhist years; `Calendar`; `LocalDate`, `LocalDateTime`, `ZonedDateTime`, `Instant` as Unix milliseconds |
| Kotlin | Its standard library, kotlinx.serialization and coroutines as the converted code calls them (`KT`), and `K`'s helpers (`substringAfter`, `trimIndent`, `String.format`…) |
| AES (Madara's chapter protector) | OpenSSL-style `EVP_BytesToKey` and AES-CBC in Rust |

Titles are known by their path (`/manga/one/`), chapters by theirs; covers and pages go through
the image proxy with the source's Referer. A site's browser check fails the call the usual way,
so the experimental "check by hand" flow works for converted sources too. Raygun sends no view
counts.

Themes so far (ported from Keiyoushi's `lib-multisrc`, Apache-2.0):

| Theme | Variants |
|---|---|
| Madara | `Madara` (`admin-ajax.php` listings), `MadaraNoAjax` (archive pages); every chapter mode (manga page, admin-ajax, manga ajax, paginated, query) |
| MangaThemesia | `MangaThemesia`, `MangaThemesiaAlt` (slugs with a changing number, looked up in the site's list) |

## Coverage

`mihon_convert_survey` converts every extension of Keiyoushi's store (1463, as of
2026-11-05) and loads each conversion as installing does (one that doesn't load counts as
refused):

| | Converted | Of |
|---|---|---|
| Madara | 202 | 240 |
| MangaThemesia | 100 | 113 |
| ZeistManga | 28 | 29 |
| GigaViewer | 19 | 19 |
| Keyoapp | 15 | 16 |
| MangaCatalog | 13 | 13 |
| PizzaReader | 10 | 10 |
| Iken | 8 | 9 |
| Masonry, UzayManga, YiiMadara | 21 | 21 |
| Other themes (56) | 31 | 222 |
| No theme | 351 | 771 |
| All | 798 (54.5%) | 1463 |

Madara and MangaThemesia extensions their ports refuse convert with the theme's code (50 and 24
of them). The most common refusals now: Next.js page data (`extractNextJs()` 52, ComiciViewer
among them), cookie jars read in code (`loadForRequest()` 29), `break`/`continue` out of a lambda
(25), `textNodes()` (16), picture-drawing interceptors (15), Keiyoushi's libraries
(`textinterceptor` 12, `unpacker` 9, `publus` 8, `speedbinb` 8).

`mihon_popular_live` with a sample of 50 converted sources (2026-11-05): 29 listed their first
popular page, among them sources on themes converted from code (ZeistManga, Liliana,
MangaCatalog, YiiMadara). Of the others, eight asked for a browser check, three sites were down,
gone or slow, and ten stopped on code the runtime doesn't run right yet (selectors Raygun's
engine didn't read, now fixed for namespaced attributes and the `i` flag, and values missing
where the Kotlin had them). GigaViewer sources convert but their series list (a GraphQL query)
still fails at run time.

```sh
cargo test -p raygun-ext-host --lib -- --ignored mihon_index_live --nocapture
MIHON_SOURCE_DIR=<unpacked keiyoushi/extensions-source> \
  cargo test -p raygun-ext-host --lib -- --ignored mihon_convert_survey --nocapture
cargo test -p raygun-ext-host --lib -- --ignored mihon_popular_live --nocapture
```

Without `MIHON_SOURCE_DIR` the survey lists the source repository with one GitHub API call and
reads the files from raw.githubusercontent.com. `MIHON_SURVEY_DETAIL=<theme>` lists that theme's
refusals one by one (`(none)` for sources without a theme), `MIHON_SURVEY_ONLY=<pkg>,…` surveys
only those packages, and `MIHON_SURVEY_DUMP=<dir>` keeps the code of conversions that don't load
(of all of them with `MIHON_SURVEY_DUMP_ALL=1`). `mihon_popular_live` loads a sample
(`MIHON_SAMPLE`, 24 by default; `MIHON_SAMPLE_ONLY=<pkg>,…` picks them) of the converted sources
and asks each for its first popular page.

## Running source code in a container

A second tier for extensions that can't be converted: their own compiled code, run apart from
Raygun behind the same `manga.source` interface.

**What's built.** Settings → Browse → "Allow running source code (experimental)"
(`Settings::run_source_code`, off by default, this device only, turned on after its notes are
read). While it's on, an extension the catalogue lists as unconvertible offers "Download APK"
(`extension_mihon_apk`, `ExtensionService::download_mihon_apk`): the store's APK is downloaded
and read without running it (`mihon::apk`: the binary manifest's package, version code,
`tachiyomi.extension` feature and `tachiyomi.extension.class`; the APK Signature Scheme v2/v3
block's certificates), and kept in the extensions folder (`.apk/<package>.apk`) only when its
package, version code and signing certificate's SHA-256 match the store
(`signingKeyFingerprint`). `mihon_apk_live` checks a few of Keiyoushi's APKs this way.

**What isn't.** The container itself, on either platform: after the check the catalogue says
"Not available on this device yet". The rest of this section is what it takes.

**Size and download.** Desktop needs a Java runtime: a jlinked JRE 21 with `java.base`,
`java.net.http`, `java.logging` and `jdk.crypto.ec` is about 45 MB (Adoptium's full JRE 21 is
about 50 MB zipped), fetched on demand into Raygun's tools folder under the data root with a
pinned version and SHA-256, the way `ytdlp.rs` fetches yt-dlp and FFmpeg. dex2jar (about 3 MB,
Apache-2.0) turns an APK's `classes.dex` into a jar; each converted APK is cached by its SHA-256.
Android runs the dex as it is.

**Mihon's API.** The classes extensions are compiled against have to exist at run time:
extensions-lib 1.4–1.6 (`Source`, `CatalogueSource`, `HttpSource`, `ParsedHttpSource`,
`SourceFactory`, `ConfigurableSource`, `SManga`, `SChapter`, `Page`, `MangasPage`, `Filter`,
`FilterList`, `SMangaUpdate`), Keiyoushi's `core` (`KeiSource`, `keiyoushi.network`, `utils`),
and what Mihon injects through Injekt: `NetworkHelper` (an OkHttp 5 client with a cookie jar),
`Json` (kotlinx.serialization), `Application` (shared preferences). The libraries themselves
(OkHttp, Okio, Jsoup, kotlinx.serialization and coroutines, RxJava 1, Kotlin's standard library)
ship in the runtime, about 9 MB of jars. Mihon's `ExtensionLoader` checks the APK's package,
`tachiyomi.extension.class` metadata, the lib version (1.4 to 1.6) and the signing certificate's
SHA-256 against the store's `signingKeyFingerprint`; the container would do the same before
loading anything.

**Limits.**
- Desktop: one sidecar process per extension, started on the first call and stopped after five
  idle minutes, with `-Xmx256m`, a 30-second budget per call and JSON-RPC over stdio. Its only
  network is an HTTP proxy Raygun runs on the loopback, which applies the extension's `net`
  hosts and Raygun's per-site limits (`-Dhttp.proxyHost`, and OkHttp's `proxy` set by the
  stubs); its working folder is the extension's data folder, and the stub `Application` hands
  out nothing else.
- Android: a service in a process of its own (`android:process=":mihon"`), loading the dex with
  an `InMemoryDexClassLoader` (Android 8+) whose parent holds the stubs, bridged to Rust over
  JNI as the player's reports are. An `android:isolatedProcess` service would be safer (no
  storage, no permissions) but has no network either, so every request would go through the main
  process; OkHttp's `Call.Factory` makes that possible.

**What breaks.** Sources that need a WebView (solving checks in a hidden one, reading pages a
script draws, `lib:cronet`) don't run in either; Raygun never solves browser checks itself. On
desktop there's no `android.graphics`, so image unscramblers (`lib:speedbinb`, `lib:publus`,
ComiciViewer, GigaViewer) need stubs drawing with Java2D, and anything else touching Android
classes (`Base64`, `Log`, `SharedPreferences`) needs a stub of its own.

**Next steps.** The extensions-lib and Keiyoushi `core` stubs as a Kotlin library (built with
the Android plugin, and as a jar for desktop), a JSON-RPC host loading a checked APK's
`tachiyomi.extension.class`, the desktop JRE and dex2jar fetched with pinned SHA-256s into the
tools folder, and the Android `:mihon` service; the kept APKs are what they'd load.

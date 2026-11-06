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
kotlinx.serialization (JSON decoded into the declared classes, `@SerialName` and `@JsonNames`
included), Keiyoushi's Next.js helpers (`extractNextJs`, `extractNextJsRsc`: the App Router's
flight stream with its references resolved, or `__NEXT_DATA__`, searched with the predicate or
the type's required fields), coroutines (`async`/`awaitAll`, `Mutex`, `Semaphore`), `Calendar`,
`java.time` dates and zones.
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
`Madara`) are left out.

### Keiyoushi's libraries

An extension (or its theme) may build on Keiyoushi's libraries (`lib/<name>` beside the
extensions):

- `lib:i18n`: its messages (`assets/i18n/messages_<lang>.properties`) come with the converted code
  for `Intl`;
- `lib:textinterceptor` and `lib:imagedecoder` are the runtime's own: text drawn as a page (a web
  comic's notes, in the system's sans-serif face) and pictures decoded;
- any other library (`lib:unpacker`, `lib:speedbinb`, `lib:publus`, `lib:clipstudioreader`…) is
  read when installing (`source::fetch_library`, after `convert::libraries_to_read`) and its code
  converts with the extension's, as it's compiled into it.

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
`graphQLBody` and `parseGraphQLAs` are the runtime's. `client.cookieJar` is the extension's own
jar (`loadForRequest`, `saveFromResponse`, OkHttp's `Cookie.parse` and `Cookie.Builder`), and so
is Android's `CookieManager`. Keiyoushi's `XorInterceptor` (MANGA Plus-style XORed pages) is a
recipe as well: each page's request gives its key when the pages are listed, and the address
carries it (`#raygun-unscramble=xor:<hex>`) for Raygun's image fetcher. `QuickJs.create()
.evaluate(script)` runs a site's script in a JavaScript engine of its own (5 s, 64 MB, nothing of
the extension's).

### Pictures the source draws

A source whose client draws, decodes or rewrites its pictures (Android's `BitmapFactory`,
`Bitmap` and `Canvas`, `lib:textinterceptor`, `lib:imagedecoder`, or an answer's bytes read and
made into a new body) gets its pages' pictures through its own client, as Mihon does:

- its pages' addresses end with `#raygun-source=<provider id>` (the converted code marks them,
  `ExtensionHost::manga_pages` adds the id; the address before it keeps its own fragment, which
  the source's code may read);
- Raygun's image fetcher (`raygun_net::images::fetch_image`, for the reader and chapter
  downloads alike) asks the source (`ExtensionHost::manga_image`, the provider's `image` call),
  which sends the request through its interceptors and answers the bytes they give;
- answers keep their bytes when they aren't text (`raygun.http.fetch`'s `binary`), as
  `Uint8Array`s that bodies, `byteStream()`, `source()`, okio's `Buffer`, Java's byte streams and
  `ByteBuffer` read;
- an interceptor decrypting answers (`response.aesDecrypt(cipher)`, `cipherSource`) counts too;
- pictures are held in Rust (`raygun_net::bitmap`, `mihon/pictures.rs`): `BitmapFactory` decodes
  PNG, JPEG, WebP, GIF and JPEG XL; a `Canvas` draws through its matrix (translations, scales,
  rotations, mirrors) inside its clip; `compress` writes JPEG or PNG.

A `break` or `continue` out of a lambda (`?.let { continue }`, `forEach { … break }`) or in an
expression (`?: continue`) is a `KT.Jump` that the loop's body catches.

A source's `getFilterList()` is converted when it can be: its `Select`, `Sort`, `CheckBox`,
`TriState` and `Group` filters become Raygun's select, toggle and tag filters (keys `f0`, `f1`…
by place), and a search sets their states before `getSearchMangaList` runs. Text filters
aren't shown. A filter list that can't be converted leaves the source without filters. Madara
and MangaThemesia, run by hand, have their standard sets (status, order, type, genres read from
the site).

### Refusals

Anything else is refused with the reason, which the catalogue then shows for that version
("Raygun can't convert it: its code uses proceed(), which Raygun's converter doesn't read yet"):

- one of Keiyoushi's libraries whose code couldn't be read;
- a call, object or name the runtime doesn't have (`emit::METHODS`, `GLOBALS` and `OBJECTS`,
  which a test keeps equal to `kotlin.js`'s tables), such as WebViews or `MultipartBody`;
- what JavaScript can't say: a vararg before other parameters;
- code that runs when the source starts (`init`).

Before the package is installed it's loaded once in a host of its own (`convert::validate`):
its script must run and register every source. Its `net` is the sources' hosts and their
subdomains, the hosts its code names in full addresses (an API beside the site, an image
server) and, as Mihon lets an extension reach any address (a CDN its answers name, an API host
put together at run time), any public host (`*`), as Nuclear and Shosetsu conversions may. The
manifest keeps `mihon: { pkg, versionCode,
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
| Keiyoushi's and Java's crypto | `ByteArray`s as lists of numbers 0–255 (text where bytes go is its UTF-8); `Base64` (Android's flags, Kotlin's `Default`/`UrlSafe`); `aesCbc`/`aesEcb`/`aesGcm`/`aesCtr`, `Cipher`, `MessageDigest`, `Mac`, `SecretKeyFactory` (PBKDF2), `decodeHex`, `md5`/`sha256` in Rust (`runtime.rs`'s `crypto`) |
| Keiyoushi's other helpers | `MarkdownDescription.Builder`, `toMarkdown` and `ratingStars` as plain text (Raygun shows descriptions as text), `tryParseZonedDateTime`, `Instant.tryParse`, Jsoup's `attrOrNull`, `selectLast`, `textNodes`, `createElement`, `:containsData` |
| Android's graphics | `Bitmap`, `BitmapFactory`, `Canvas`, `Matrix`, `Rect`, `Paint`, `Color` over pictures held in Rust (`raygun_net::bitmap`) |
| Bytes | Answers' bytes as `Uint8Array`s; okio's `Buffer` and `ByteString`, Java's byte streams, `ByteBuffer` |
| Protobuf | Keiyoushi's `parseAsProto`, `decodeProto`, `toRequestBodyProto`, `encodeProto` over the classes the source declares (`@ProtoNumber`, `@ProtoType`, packed lists, maps, enums by number) |
| Decrypted answers | Keiyoushi's `response.aesDecrypt(cipher)`, `aesDecryptWithPrependedIv`, okio's `cipherSource`; pictures decrypted this way are fetched through the source |

Titles are known by their path (`/manga/one/`), chapters by theirs; covers and pages go through
the image proxy with the source's Referer. Mihon's `memo`s (what a source keeps with a title or
a chapter, such as Madara's `mangaPath`) are kept too: a chapter's in its id after a `U+001F`
separator, as its path alone may not tell it from another title's; a title's by its path in the
extension's storage (the newest 400 KB), with the name it was listed under, which details
without a name keep, as Mihon does. Pages that are sub pages (`Page(i, url)`) get their
pictures from the source's `getImageUrl`, four at a time. A site's browser check fails the call the usual way,
so the experimental "check by hand" flow works for converted sources too. Raygun sends no view
counts.

Themes so far (ported from Keiyoushi's `lib-multisrc`, Apache-2.0):

| Theme | Variants |
|---|---|
| Madara | `Madara` (`admin-ajax.php` listings), `MadaraNoAjax` (archive pages); every chapter mode (manga page, admin-ajax, manga ajax, paginated, query) |
| MangaThemesia | `MangaThemesia`, `MangaThemesiaAlt` (slugs with a changing number, looked up in the site's list) |

## Coverage

`mihon_convert_survey` converts every extension of Keiyoushi's store (1469 as of 2026-11-06, 6
of them newer than the checkout surveyed) and loads each conversion as installing does (one that
doesn't load counts as refused):

| | Converted | Of |
|---|---|---|
| Madara | 232 | 240 |
| MangaThemesia | 104 | 113 |
| ComiciViewer, ZeistManga, GigaViewer | 77 | 77 |
| Keyoapp | 15 | 16 |
| MangaCatalog, YiiManga, PizzaReader | 35 | 35 |
| Iken, InitManga, FoolSlide, GalleryAdults | 33 | 35 |
| Masonry, UzayManga, VineTheme, WPComics, YiiMadara | 35 | 35 |
| MangaHub, Grouple, MMRCMS | 5 | 23 |
| Other themes | 71 | 118 |
| No theme | 523 | 771 |
| All | 1130 (76.9%) | 1469 |

The refusals left are a long tail of a few extensions each (none above 11): calls the runtime
doesn't have yet (`getLocalStorage()` and `poll()` need a web view; `resolve()`, `DecimalFormat()`,
`Selection()`, `compare()`…), and filters that run code when they're made. Fixing one often shows
the next an extension needs, so each step now gains well under 1%.

`mihon_popular_live` with a sample of 60 converted sources (2026-11-06) follows each listing
through a title's details, its chapters and a chapter's pages: about 40 of the 60 list their
popular titles, and of those 95% give details and chapters and over 80% pages. The sources that
don't list stop on what the runtime can't change: a browser check, a site that refuses access
from outside its country (most ComiciViewer sites outside Japan), a site down, slow or rate
limiting, or a site that changed.

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
and follows each from its first popular page to the first page's picture (drawn by the source
when it draws them).

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

**The host.** `tools/mihon-host/` is the shared host that runs one extension's own code: Mihon's
source API (adapted, Apache-2.0: `Source`, `CatalogueSource`, `HttpSource`, `ParsedHttpSource`,
`SourceFactory`, `ConfigurableSource`, the models and filters), Mihon's `NetworkHelper` and
`AndroidCookieJar` over OkHttp 5, and JVM stand-ins for the Android classes extensions touch
(`Context`/`Application` with file-backed `SharedPreferences`, `Base64`, `Log`, `Uri`,
`SystemClock`, `PreferenceScreen`). `MainKt` reads JSON-RPC lines over stdio — `load` (loads the
converted jar's `tachiyomi.extension.class` through a `URLClassLoader`, seeds Raygun's cookies,
lists the sources) then `popular`, `latest`, `search` (with Raygun's filters), `details`,
`chapters`, `pages`, `image`. It binds a public DNS so an extension can't reach this computer or
its network, and sends Raygun's User-Agent. `build.gradle.kts`'s `runtimeLibs` prints each
library pinned by SHA-256 for the desktop fetcher.

**Desktop — built.** `raygun-app/src/mihon_container/`:
- `tools.rs` fetches a pinned Temurin JRE 21 and dex2jar 2.4 on first use into `tools/mihon/`,
  each checked against its SHA-256, with progress and cancel (`mihon_runtime_setup`,
  `MihonRuntimeEvent`); the host jar ships beside them.
- `sidecar.rs` converts the checked APK's dex to a jar (dex2jar, once, cached), runs
  `java -Xmx256m -jar mihon-host.jar` per extension, and speaks the JSON-RPC above with a
  per-call timeout, restart on crash, and idle shutdown after five minutes.
- `mod.rs` (`MihonContainer`) holds the sidecars and answers the `manga.source` calls.
- `mihon_run` drives one call; the source's details (`extensions/MihonApkAction.tsx`) offers
  "Set up the runtime" then "Browse with its own code". The live path (dex2jar + the JVM running
  third-party code) is exercised by the ignored `desktop_runs_a_real_extension` test, which the
  lead runs on a desktop; the development sandbox blocks executing converted code.

**Android — not in this build.** The design is a service in a `:mihon` process
(`android:process=":mihon"`; `android:isolatedProcess` where its no-network limit can be met by
proxying requests back through the main process over AIDL) loading the dex with a
`DexClassLoader`/`InMemoryDexClassLoader` against the same stubs, bridged to Rust as the player's
reports are. The Rust side already routes Android to an honest "not in this build yet", so the UI
says so; wiring the service is the remaining work.

**What breaks either way.** Sources that need a WebView (solving checks in a hidden one, reading
pages a script draws, `lib:cronet`); Raygun never solves browser checks itself. On desktop there
is no `android.graphics`, so image unscramblers that decode bitmaps in an interceptor
(`lib:speedbinb`, `lib:publus`) don't run; the ComiciViewer and GigaViewer tile shuffles are
handled by Raygun's own converter instead (above).

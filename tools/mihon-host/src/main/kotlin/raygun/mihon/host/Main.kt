package raygun.mihon.host

import android.app.Application
import eu.kanade.tachiyomi.network.HttpException
import eu.kanade.tachiyomi.network.NetworkHelper
import eu.kanade.tachiyomi.source.CatalogueSource
import eu.kanade.tachiyomi.source.Source
import eu.kanade.tachiyomi.source.SourceFactory
import eu.kanade.tachiyomi.source.model.Filter
import eu.kanade.tachiyomi.source.model.FilterList
import eu.kanade.tachiyomi.source.model.MangasPage
import eu.kanade.tachiyomi.source.model.Page
import eu.kanade.tachiyomi.source.model.SChapter
import eu.kanade.tachiyomi.source.model.SManga
import eu.kanade.tachiyomi.source.online.HttpSource
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.add
import kotlinx.serialization.json.addJsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.int
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.putJsonObject
import okhttp3.Cookie
import okhttp3.Request
import uy.kohesive.injekt.Injekt
import uy.kohesive.injekt.api.addSingleton
import java.io.File
import java.io.FileDescriptor
import java.io.FileOutputStream
import java.io.IOException
import java.io.PrintStream
import java.net.URLClassLoader
import java.util.Base64
import java.util.concurrent.Executors
import kotlin.system.exitProcess
import kotlin.time.Duration.Companion.seconds

/**
 * Raygun's Mihon host: one extension's own code, loaded from its converted jar, answering
 * Raygun's calls as JSON lines over stdin/stdout (`{"id", "method", "params"}` in, `{"id",
 * "result"}` or `{"id", "error"}` out). Anything the extension prints goes to stderr.
 */
fun main() {
    val protocol = PrintStream(FileOutputStream(FileDescriptor.out), true, "UTF-8")
    System.setOut(System.err)
    val host = Host()
    val pool = Executors.newFixedThreadPool(4)
    val input = System.`in`.bufferedReader(Charsets.UTF_8)
    while (true) {
        val line = input.readLine() ?: break
        if (line.isBlank()) continue
        pool.execute {
            val answer = host.handle(line)
            synchronized(protocol) { protocol.println(answer) }
        }
    }
    pool.shutdown()
    exitProcess(0)
}

class Host {
    private val json = Json { ignoreUnknownKeys = true }
    private var network: NetworkHelper? = null
    private val sources = mutableMapOf<String, CatalogueSource>()

    fun handle(line: String): String {
        var id: JsonElement = JsonNull
        return try {
            val request = json.parseToJsonElement(line).jsonObject
            id = request["id"] ?: JsonNull
            val method = request["method"]?.jsonPrimitive?.content ?: error("a call without a method")
            val params = request["params"] as? JsonObject ?: JsonObject(emptyMap())
            val result = runBlocking { withTimeout(CALL_BUDGET) { call(method, params) } }
            buildJsonObject {
                put("id", id)
                put("result", result)
            }.toString()
        } catch (error: Throwable) {
            buildJsonObject {
                put("id", id)
                putJsonObject("error") {
                    put("message", describe(error))
                    put("type", error.javaClass.simpleName)
                    if (error is HttpException) put("code", error.code)
                }
            }.toString()
        }
    }

    private fun describe(error: Throwable): String {
        val message = error.message?.takeIf { it.isNotBlank() } ?: error.javaClass.simpleName
        return when (error) {
            is NoClassDefFoundError, is ClassNotFoundException ->
                "the extension needs $message, which Raygun's host doesn't have"
            is UnsatisfiedLinkError -> "the extension needs Android's native code ($message)"
            else -> message
        }
    }

    private suspend fun call(method: String, params: JsonObject): JsonElement = when (method) {
        "load" -> load(params)
        "popular" -> page(source(params).getPopularManga(params.int("page", 1)))
        "latest" -> page(source(params).getLatestUpdates(params.int("page", 1)))
        "search" -> {
            val source = source(params)
            val filters = source.getFilterList()
            (params["filters"] as? JsonObject)?.let { applyFilters(filters, it) }
            page(source.getSearchManga(params.int("page", 1), params.string("query") ?: "", filters))
        }
        "filters" -> describeFilters(source(params).getFilterList())
        "details" -> {
            val manga = SManga.create().apply { url = params.string("url") ?: error("no url") }
            manga(source(params).getMangaUpdate(manga, emptyList(), fetchDetails = true, fetchChapters = false).manga)
        }
        "chapters" -> {
            val manga = SManga.create().apply { url = params.string("url") ?: error("no url") }
            val update = source(params).getMangaUpdate(manga, emptyList(), fetchDetails = false, fetchChapters = true)
            buildJsonArray { update.chapters.forEach { add(chapter(it)) } }
        }
        "pages" -> pages(source(params), params)
        "image" -> image(source(params), params)
        "cookies" -> buildJsonArray { network?.cookieJar?.all()?.forEach { add(cookie(it)) } }
        else -> error("the host has no call $method")
    }

    private fun source(params: JsonObject): CatalogueSource =
        sources[params.string("source")] ?: error("the extension has no source ${params.string("source")}")

    // ---- Loading ----

    private fun load(params: JsonObject): JsonElement {
        val dataDir = File(params.string("dataDir") ?: error("no data folder"))
        dataDir.mkdirs()
        val network = NetworkHelper(File(dataDir, "cache"), params.string("userAgent") ?: DEFAULT_AGENT)
        this.network = network
        Injekt.addSingleton(Application(dataDir) as Application)
        Injekt.addSingleton(network)
        Injekt.addSingleton(Json { ignoreUnknownKeys = true; explicitNulls = false })
        // Raygun's cookies for the extension (a browser check passed by hand among them).
        (params["cookies"] as? JsonArray)?.forEach { item ->
            val fields = item.jsonObject
            val domain = fields.string("domain")?.trimStart('.') ?: return@forEach
            val builder = Cookie.Builder()
                .name(fields.string("name") ?: return@forEach)
                .value(fields.string("value") ?: "")
                .path(fields.string("path") ?: "/")
            if (fields["hostOnly"]?.jsonPrimitive?.booleanOrNull == true) builder.hostOnlyDomain(domain) else builder.domain(domain)
            fields["expiresAt"]?.jsonPrimitive?.longOrNull?.let { builder.expiresAt(it) }
            if (fields["secure"]?.jsonPrimitive?.booleanOrNull == true) builder.secure()
            if (fields["httpOnly"]?.jsonPrimitive?.booleanOrNull == true) builder.httpOnly()
            val url = okhttp3.HttpUrl.Builder().scheme("https").host(domain).build()
            network.cookieJar.saveFromResponse(url, listOf(builder.build()))
        }
        val loader = URLClassLoader(arrayOf(File(params.string("jar") ?: error("no jar")).toURI().toURL()), Host::class.java.classLoader)
        val packageName = params.string("packageName") ?: ""
        val className = (params.string("className") ?: error("no class")).let {
            if (it.startsWith(".")) packageName + it else it
        }
        val made = loader.loadClass(className).getDeclaredConstructor().newInstance()
        val found: List<Source> = when (made) {
            is SourceFactory -> made.createSources()
            is Source -> listOf(made)
            else -> error("$className isn't a Mihon source")
        }
        sources.clear()
        return buildJsonArray {
            for (source in found.filterIsInstance<CatalogueSource>()) {
                sources[source.id.toString()] = source
                addJsonObject {
                    put("id", source.id.toString())
                    put("name", source.name)
                    put("lang", source.lang)
                    put("supportsLatest", source.supportsLatest)
                    if (source is HttpSource) put("baseUrl", source.baseUrl)
                }
            }
        }
    }

    // ---- Answers ----

    private fun page(page: MangasPage): JsonElement = buildJsonObject {
        putJsonArray("items") { page.mangas.forEach { add(manga(it)) } }
        put("hasNextPage", page.hasNextPage)
    }

    private fun manga(manga: SManga): JsonElement = buildJsonObject {
        put("url", runCatching { manga.url }.getOrNull())
        put("title", runCatching { manga.title }.getOrNull())
        put("artist", manga.artist)
        put("author", manga.author)
        put("description", manga.description)
        put("genre", manga.genre)
        put("status", manga.status)
        put("thumbnailUrl", manga.thumbnail_url)
    }

    private fun chapter(chapter: SChapter): JsonElement = buildJsonObject {
        put("url", runCatching { chapter.url }.getOrNull())
        put("name", runCatching { chapter.name }.getOrNull())
        put("dateUpload", chapter.date_upload)
        put("chapterNumber", chapter.chapter_number)
        put("scanlator", chapter.scanlator)
    }

    private fun cookie(cookie: Cookie): JsonElement = buildJsonObject {
        put("name", cookie.name)
        put("value", cookie.value)
        put("domain", cookie.domain)
        put("path", cookie.path)
        put("expiresAt", cookie.expiresAt)
        put("secure", cookie.secure)
        put("httpOnly", cookie.httpOnly)
        put("hostOnly", cookie.hostOnly)
    }

    /** Each page's picture: its address and the headers the source's image request sends. */
    private suspend fun pages(source: CatalogueSource, params: JsonObject): JsonElement {
        val chapter = SChapter.create().apply { url = params.string("url") ?: error("no url") }
        val pages = source.getPageList(chapter)
        return buildJsonArray {
            for (page in pages) {
                if (source is HttpSource && page.imageUrl.isNullOrEmpty()) {
                    page.imageUrl = runCatching { source.getImageUrl(page) }.getOrNull()
                }
                val request = (source as? HttpSource)?.let { imageRequest(it, page) }
                addJsonObject {
                    put("index", page.index)
                    put("url", request?.url?.toString() ?: page.imageUrl)
                    putJsonObject("headers") {
                        request?.headers?.forEach { (name, value) -> put(name, value) }
                    }
                }
            }
        }
    }

    private fun imageRequest(source: HttpSource, page: Page): Request? = runCatching {
        val method = HttpSource::class.java.getDeclaredMethod("imageRequest", Page::class.java)
        method.isAccessible = true
        method.invoke(source, page) as Request
    }.getOrNull()

    /** A page's picture fetched through the source's own client (its interceptors run). */
    private suspend fun image(source: CatalogueSource, params: JsonObject): JsonElement {
        val http = source as? HttpSource ?: error("the source fetches no pictures itself")
        val page = Page(params.int("index", 0), params.string("url") ?: "", params.string("imageUrl"))
        val response = http.getImage(page)
        val bytes = response.body.bytes()
        if (!response.isSuccessful) throw HttpException(response.code)
        return buildJsonObject {
            put("contentType", response.header("Content-Type"))
            put("data", Base64.getEncoder().encodeToString(bytes))
        }
    }

    // ---- Filters, as Raygun shows them (keys `f0`, `f1`… by place) ----

    private fun describeFilters(list: FilterList): JsonElement = buildJsonArray {
        list.forEachIndexed { index, filter ->
            val key = "f$index"
            when (filter) {
                is Filter.Select<*> -> addJsonObject {
                    put("kind", "select")
                    put("key", key)
                    put("label", filter.name)
                    putJsonArray("options") {
                        filter.values.forEachIndexed { at, value ->
                            addJsonObject {
                                put("value", at.toString())
                                put("label", value.toString())
                            }
                        }
                    }
                    put("default", filter.state.toString())
                }
                is Filter.Sort -> addJsonObject {
                    put("kind", "select")
                    put("key", key)
                    put("label", filter.name)
                    putJsonArray("options") {
                        filter.values.forEachIndexed { at, value ->
                            addJsonObject {
                                put("value", "$at:d")
                                put("label", "$value ↓")
                            }
                            addJsonObject {
                                put("value", "$at:a")
                                put("label", "$value ↑")
                            }
                        }
                    }
                    filter.state?.let { put("default", "${it.index}:${if (it.ascending) "a" else "d"}") }
                }
                is Filter.CheckBox -> addJsonObject {
                    put("kind", "toggle")
                    put("key", key)
                    put("label", filter.name)
                    put("default", filter.state)
                }
                is Filter.TriState -> addJsonObject {
                    put("kind", "tags")
                    put("key", key)
                    put("label", filter.name)
                    putJsonArray("options") {
                        addJsonObject {
                            put("value", "0")
                            put("label", filter.name)
                        }
                    }
                }
                is Filter.Group<*> -> addJsonObject {
                    put("kind", "tags")
                    put("key", key)
                    put("label", filter.name)
                    putJsonArray("options") {
                        filter.state.forEachIndexed { at, child ->
                            addJsonObject {
                                put("value", at.toString())
                                put("label", (child as? Filter<*>)?.name ?: child.toString())
                            }
                        }
                    }
                }
                else -> {}
            }
        }
    }

    private fun applyFilters(list: FilterList, values: JsonObject) {
        list.forEachIndexed { index, filter ->
            val value = values["f$index"] ?: return@forEachIndexed
            when (filter) {
                is Filter.Select<*> -> (value as? JsonPrimitive)?.intOrNull?.let { filter.state = it }
                    ?: (value as? JsonPrimitive)?.contentOrNull?.toIntOrNull()?.let { filter.state = it }
                is Filter.Sort -> (value as? JsonPrimitive)?.contentOrNull?.split(':')?.let { (at, direction) ->
                    filter.state = Filter.Sort.Selection(at.toIntOrNull() ?: 0, direction == "a")
                }
                is Filter.CheckBox -> (value as? JsonPrimitive)?.booleanOrNull?.let { filter.state = it }
                is Filter.TriState -> (value as? JsonObject)?.let { chosen ->
                    filter.state = when {
                        chosen.list("include").isNotEmpty() -> Filter.TriState.STATE_INCLUDE
                        chosen.list("exclude").isNotEmpty() -> Filter.TriState.STATE_EXCLUDE
                        else -> Filter.TriState.STATE_IGNORE
                    }
                }
                is Filter.Group<*> -> (value as? JsonObject)?.let { chosen ->
                    val include = chosen.list("include")
                    val exclude = chosen.list("exclude")
                    filter.state.forEachIndexed { at, child ->
                        when (child) {
                            is Filter.TriState -> child.state = when (at.toString()) {
                                in include -> Filter.TriState.STATE_INCLUDE
                                in exclude -> Filter.TriState.STATE_EXCLUDE
                                else -> Filter.TriState.STATE_IGNORE
                            }
                            is Filter.CheckBox -> child.state = at.toString() in include
                        }
                    }
                }
                else -> {}
            }
        }
    }

    companion object {
        /** How long one call may take, its requests included. */
        val CALL_BUDGET = 90.seconds
        const val DEFAULT_AGENT =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36"
    }
}

private fun JsonObject.string(key: String): String? = (this[key] as? JsonPrimitive)?.contentOrNull

private fun JsonObject.int(key: String, fallback: Int): Int = (this[key] as? JsonPrimitive)?.intOrNull ?: fallback

private fun JsonObject.list(key: String): List<String> =
    (this[key] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull } ?: emptyList()

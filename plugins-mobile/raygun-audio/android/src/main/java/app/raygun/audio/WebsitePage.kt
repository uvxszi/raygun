package app.raygun.audio

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.text.TextUtils
import android.util.Log
import android.util.TypedValue
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.ScriptHandler
import androidx.webkit.WebViewFeature
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** The page bridge's name in pages (`raygunBridge`), as the bridge script calls it. */
internal const val BRIDGE_NAME = "raygunBridge"

/** A script file read for pages, at most (the bridge, an extension's composed bundle). */
private const val MAX_SCRIPT_BYTES = 8L * 1024 * 1024

/** Extensions with site scripts listed, at most. */
private const val MAX_SITE_SCRIPTS = 64

/** A message from a page longer than this is ignored. */
internal const val MAX_PAGE_MESSAGE = 64 * 1024

/** Frames a tab answers, at most (the newest kept). */
private const val MAX_FRAMES = 32

/** Offers a tab keeps, at most (the newest kept). */
private const val MAX_OFFERS = 20

private const val MAX_ADDRESS = 2048
private const val MAX_TEXT = 300

private val OFFER_KINDS = setOf("video", "song", "playlist")
private val MEDIA_ACTIONS = setOf("play", "pause", "previoustrack", "nexttrack", "seekto")

/**
 * An extension's site scripts for this site: one bundle Raygun composed of its scripts, registered
 * for the origins it names. The bundle checks the full match patterns itself.
 */
internal class SiteScript(
    val id: String,
    val name: String,
    val file: String,
    /** androidx.webkit origin rules: a scheme and host, `*.` before the host for its subdomains, or `*`. */
    val origins: List<String>,
    var on: Boolean,
    /** The bundle; null when its file couldn't be read (it's still listed, to switch). */
    val code: String?,
) {
    /** As the request lists it (without the code), to keep across the browser being made again. */
    fun json(): JSONObject = JSONObject()
        .put("id", id)
        .put("name", name)
        .put("file", file)
        .put("origins", JSONArray(origins))
        .put("on", on)

    /** Whether it runs on pages of `origin`. */
    fun runsOn(origin: Uri?): Boolean = origins.any { originMatches(it, origin) }
}

/** An extension with site scripts for this site, not installed: the menu offers it. */
internal class SiteSuggestion(
    val id: String,
    val name: String,
    val repository: String,
    val origins: List<String>,
)

/** Something a page offered to Raygun through an extension's script, checked. */
internal class PageOffer(
    val extension: String,
    /** The address of the page (or frame) that offered it. */
    val page: String,
    /** The origin of the frame that offered it. */
    val origin: String,
    val kind: String,
    val url: String,
    val title: String?,
    val artist: String?,
    val album: String?,
    val artworkUrl: String?,
    val durationMs: Long?,
) {
    /** As it goes to Raygun. */
    fun json(): JSONObject = JSONObject()
        .put("kind", kind)
        .put("url", url)
        .put("title", title)
        .put("artist", artist)
        .put("album", album)
        .put("artworkUrl", artworkUrl)
        .put("durationMs", durationMs)

    /** Its title, else where it is (host and path). */
    val label: String
        get() = title ?: Uri.parse(url).let { "${it.host.orEmpty()}${it.path.orEmpty()}" }.ifBlank { url }
}

/** What a frame says plays (or is paused) there, and the frame, which takes the controls. */
internal class PageMedia(
    val playing: Boolean,
    val title: String?,
    val artist: String?,
    val album: String?,
    val artwork: String?,
    /** Seconds; null when unknown (a live stream). */
    val duration: Double?,
    val position: Double?,
    val rate: Double,
    val actions: Set<String>,
    val frame: JavaScriptReplyProxy,
    /** When it was said (`SystemClock.elapsedRealtime`), for the position. */
    val at: Long = SystemClock.elapsedRealtime(),
)

/** A script registered in a tab, with the code it was registered with. */
internal class Registered(val handler: ScriptHandler, val code: String)

/** A message to a frame through the bridge (frames are only known where web message listeners are). */
internal fun JavaScriptReplyProxy.tell(message: String) {
    if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) runCatching { postMessage(message) }
}

/** What a tab's page told the browser through the bridge, and the scripts registered in it. */
internal class TabPage {
    /** Frames that said hello (or reported), oldest first: state messages go to each. */
    val frames = ArrayList<JavaScriptReplyProxy>()

    /** What plays or is paused in the page now. */
    var media: PageMedia? = null

    /** What the page offers to Raygun, newest first. */
    val offers = ArrayList<PageOffer>()

    var bridge: ScriptHandler? = null

    /** Site scripts registered in this tab, by extension id. */
    val scripts = HashMap<String, Registered>()

    /** When the main frame last said hello (`SystemClock.uptimeMillis`). */
    var mainHelloAt = 0L

    fun addFrame(frame: JavaScriptReplyProxy) {
        if (frames.any { it === frame }) return
        frames.add(frame)
        if (frames.size > MAX_FRAMES) frames.removeAt(0)
    }

    /** Keeps `offer` first; an older one for the same address goes. */
    fun addOffer(offer: PageOffer) {
        offers.removeAll { it.url == offer.url }
        offers.add(0, offer)
        while (offers.size > MAX_OFFERS) offers.removeAt(offers.size - 1)
    }

    /** A new page: what the one before said is gone. */
    fun clear() {
        frames.clear()
        offers.clear()
        media = null
    }
}

/**
 * A tab's WebView. When the browser leaves the screen Android tells its WebViews the window is
 * hidden, and pages hear they're hidden too (sites pause or slow their players then). While
 * [keepVisible] (its page plays and background play is on) it hears the window is still shown, as
 * NouTube does it; nothing is drawn meanwhile anyway.
 */
internal class PageWebView(context: Context) : WebView(context) {
    /** What its pages said through the bridge. */
    val page = TabPage()

    /** The window's visibility as Android last said it. */
    private var windowShown = View.VISIBLE

    /** Read when the window's visibility changes; [tellWindow] passes a change on at once. */
    var keepVisible = false
        set(value) {
            if (field == value) return
            field = value
            if (value && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                // WebView's default, said again: a renderer waived out of sight may be stopped by
                // the system while it plays, and without onRenderProcessGone that ends the browser.
                setRendererPriorityPolicy(WebView.RENDERER_PRIORITY_IMPORTANT, false)
            }
        }

    override fun onWindowVisibilityChanged(visibility: Int) {
        windowShown = visibility
        super.onWindowVisibilityChanged(if (keepVisible) View.VISIBLE else visibility)
    }

    /** With the window out of sight: the page hears now how it stands, as [keepVisible] has it. */
    fun tellWindow() {
        if (windowShown != View.VISIBLE) super.onWindowVisibilityChanged(if (keepVisible) View.VISIBLE else windowShown)
    }
}

/**
 * The frame of a page's full-screen view (a video), which WebView follows the window through
 * instead of the tab's own view: while [keep] it passes "shown" down when the window is hidden.
 */
internal class KeptVisibleFrame(context: Context, private val keep: () -> Boolean) : FrameLayout(context) {
    private var windowShown = View.VISIBLE

    override fun dispatchWindowVisibilityChanged(visibility: Int) {
        windowShown = visibility
        super.dispatchWindowVisibilityChanged(if (keep()) View.VISIBLE else visibility)
    }

    /** After [keep] changed while the window is hidden: the view in it hears how things stand. */
    fun recheck() {
        if (windowShown != View.VISIBLE) super.dispatchWindowVisibilityChanged(if (keep()) View.VISIBLE else windowShown)
    }
}

// ---- Reading the request ----

/** A script file Raygun wrote for the browser; null (with a log line) when it can't be read. */
internal fun readScript(path: String?): String? {
    if (path.isNullOrBlank()) return null
    return try {
        val file = File(path)
        when {
            !file.isFile -> {
                Log.w("RaygunAudio", "no site script at $path")
                null
            }
            file.length() > MAX_SCRIPT_BYTES -> {
                Log.w("RaygunAudio", "site script $path is larger than 8 MB")
                null
            }
            else -> file.readText()
        }
    } catch (error: Exception) {
        Log.w("RaygunAudio", "site script $path couldn't be read", error)
        null
    }
}

/** The request's `siteScripts`, each bundle read (so it's done off the main thread when it can be). */
internal fun parseSiteScripts(list: JSONArray?): MutableList<SiteScript> {
    val scripts = mutableListOf<SiteScript>()
    if (list == null) return scripts
    for (index in 0 until list.length()) {
        if (scripts.size >= MAX_SITE_SCRIPTS) break
        val item = list.optJSONObject(index) ?: continue
        val id = item.text("id")?.trim() ?: continue
        if (scripts.any { it.id == id }) continue
        val file = item.text("file")?.trim().orEmpty()
        scripts.add(
            SiteScript(
                id = id,
                name = item.text("name")?.trim() ?: id,
                file = file,
                origins = item.optJSONArray("origins").strings(),
                on = item.optBoolean("on", true),
                code = readScript(file),
            ),
        )
    }
    return scripts
}

internal fun parseSuggestions(list: JSONArray?): List<SiteSuggestion> {
    if (list == null) return emptyList()
    val suggestions = mutableListOf<SiteSuggestion>()
    for (index in 0 until list.length()) {
        if (suggestions.size >= MAX_SITE_SCRIPTS) break
        val item = list.optJSONObject(index) ?: continue
        val id = item.text("id")?.trim() ?: continue
        val repository = item.text("repository")?.trim() ?: continue
        if (suggestions.any { it.id == id }) continue
        suggestions.add(SiteSuggestion(id, item.text("name")?.trim() ?: id, repository, item.optJSONArray("origins").strings()))
    }
    return suggestions
}

/** The array's strings, trimmed, without blank ones. */
internal fun JSONArray?.strings(limit: Int = 256): List<String> {
    if (this == null) return emptyList()
    val strings = ArrayList<String>()
    for (index in 0 until minOf(length(), limit)) {
        val value = (opt(index) as? String)?.trim() ?: continue
        if (value.isNotEmpty()) strings.add(value)
    }
    return strings
}

// ---- Origins ----

private fun defaultPort(scheme: String): Int = when (scheme) {
    "https" -> 443
    "http" -> 80
    else -> -1
}

/** `scheme://host[:port]` (lowercase, the scheme's own port left out); null for opaque ones. */
internal fun originOf(uri: Uri?): String? {
    val scheme = uri?.scheme?.lowercase() ?: return null
    val host = uri.host?.lowercase()?.takeIf { it.isNotEmpty() } ?: return null
    val port = uri.port.takeIf { it != -1 && it != defaultPort(scheme) }
    return if (port != null) "$scheme://$host:$port" else "$scheme://$host"
}

/**
 * Whether `origin` is one androidx.webkit's origin `rule` allows: `*` allows every one, a host
 * with `*.` before it the host's subdomains (not the host itself), a plain host only that host;
 * the scheme and the port (the scheme's own when the rule names none) must be the same.
 */
internal fun originMatches(rule: String, origin: Uri?): Boolean {
    val wanted = rule.trim()
    if (wanted == "*") return true
    val scheme = origin?.scheme?.lowercase() ?: return false
    val host = origin.host?.lowercase()?.trimEnd('.')?.takeIf { it.isNotEmpty() } ?: return false
    val separator = wanted.indexOf("://")
    if (separator <= 0 || !wanted.substring(0, separator).equals(scheme, ignoreCase = true)) return false
    var rest = wanted.substring(separator + 3).lowercase().trimEnd('/')
    val portAt = rest.lastIndexOf(':')
    val rulePort = if (portAt >= 0 && portAt > rest.lastIndexOf(']')) {
        val port = rest.substring(portAt + 1).toIntOrNull() ?: return false
        rest = rest.substring(0, portAt)
        port
    } else {
        defaultPort(scheme)
    }
    val port = origin.port.takeIf { it != -1 } ?: defaultPort(scheme)
    if (port != rulePort) return false
    return if (rest.startsWith("*.")) host.endsWith(rest.substring(1)) else host == rest
}

/** An http(s) address with a host. */
internal fun isWebAddress(address: String): Boolean {
    val uri = Uri.parse(address)
    val scheme = uri.scheme?.lowercase()
    return (scheme == "https" || scheme == "http") && !uri.host.isNullOrEmpty()
}

// ---- Bridge messages ----

/** A text field of a page's message, trimmed; null when missing or blank. */
private fun JSONObject.trimmed(key: String): String? = text(key)?.trim()?.takeIf { it.isNotEmpty() }

/** A text field of a page's message to show, trimmed and cut to 300 characters. */
private fun JSONObject.pageText(key: String): String? = trimmed(key)?.take(MAX_TEXT)

/** An http(s) address field of a page's message; null when it's anything else. */
private fun JSONObject.pageAddress(key: String): String? =
    trimmed(key)?.takeIf { it.length <= MAX_ADDRESS && isWebAddress(it) }

/** Seconds, when a finite number not below 0. */
private fun JSONObject.seconds(key: String): Double? =
    if (isNull(key)) null else optDouble(key).takeIf { it.isFinite() && it >= 0 }

/**
 * An offer (`{"type":"offer","extension","page","offer"}`) from a frame of `sourceOrigin`,
 * checked: an extension of the request that's on and runs there, the page of that origin, a kind
 * Raygun takes and an http(s) address. Null when it doesn't hold up.
 */
internal fun parseOffer(message: JSONObject, sourceOrigin: Uri, scripts: List<SiteScript>): PageOffer? {
    val extension = message.trimmed("extension") ?: return null
    val script = scripts.firstOrNull { it.id == extension && it.on } ?: return null
    val origin = originOf(sourceOrigin) ?: return null
    if (!script.runsOn(sourceOrigin)) return null
    val page = message.trimmed("page") ?: return null
    if (originOf(Uri.parse(page)) != origin) return null
    val offer = message.optJSONObject("offer") ?: return null
    val kind = offer.pageText("kind")?.takeIf { it in OFFER_KINDS } ?: return null
    val url = offer.pageAddress("url") ?: return null
    val duration = if (offer.isNull("durationMs")) null else offer.optDouble("durationMs").takeIf { it.isFinite() && it > 0 }
    return PageOffer(
        extension = extension,
        page = page,
        origin = origin,
        kind = kind,
        url = url,
        title = offer.pageText("title"),
        artist = offer.pageText("artist"),
        album = offer.pageText("album"),
        artworkUrl = offer.pageAddress("artworkUrl"),
        durationMs = duration?.toLong(),
    )
}

/**
 * A withdrawal (`{"type":"withdraw","extension","url"}`): the extension's offers from that origin
 * go, the one for `url` or all of them. True when one went.
 */
internal fun withdrawOffers(page: TabPage, message: JSONObject, sourceOrigin: Uri): Boolean {
    val extension = message.trimmed("extension") ?: return false
    val origin = originOf(sourceOrigin) ?: return false
    val url = message.trimmed("url")
    return page.offers.removeAll { it.extension == extension && it.origin == origin && (url == null || it.url == url) }
}

/** A `media` report that something plays or is paused; null for `none` or a malformed one. */
internal fun parseMedia(message: JSONObject, frame: JavaScriptReplyProxy): PageMedia? {
    val state = message.optString("state")
    if (state != "playing" && state != "paused") return null
    return PageMedia(
        playing = state == "playing",
        title = message.pageText("title"),
        artist = message.pageText("artist"),
        album = message.pageText("album"),
        artwork = message.pageAddress("artwork"),
        duration = message.seconds("duration")?.takeIf { it > 0 },
        position = message.seconds("position"),
        rate = message.optDouble("rate", 1.0).takeIf { it.isFinite() && it > 0 } ?: 1.0,
        actions = message.optJSONArray("actions").strings(16).filter { it in MEDIA_ACTIONS }.toSet(),
        frame = frame,
    )
}

// ---- The offers sheet ----

private fun Context.sheetText(size: Float, bold: Boolean = false): TextView = TextView(this).apply {
    setTextColor(Color.WHITE)
    setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
    if (bold) typeface = Typeface.DEFAULT_BOLD
    maxLines = 1
    ellipsize = TextUtils.TruncateAt.END
}

/** A ScrollView no taller than `max()` pixels. */
private class ShortScroll(context: Context, private val max: () -> Int) : ScrollView(context) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(max(), MeasureSpec.AT_MOST))
    }
}

/** Pills in rows: as many to a row as fit, the rest on the next. */
private class PillRows(context: Context) : ViewGroup(context) {
    private val gap = context.dp(8)

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec) - paddingLeft - paddingRight
        var x = 0
        var y = 0
        var rowHeight = 0
        var widest = 0
        for (index in 0 until childCount) {
            val child = getChildAt(index)
            if (child.visibility == View.GONE) continue
            measureChild(child, widthMeasureSpec, heightMeasureSpec)
            if (x > 0 && x + child.measuredWidth > width) {
                x = 0
                y += rowHeight + gap
                rowHeight = 0
            }
            x += child.measuredWidth + gap
            rowHeight = maxOf(rowHeight, child.measuredHeight)
            widest = maxOf(widest, x - gap)
        }
        setMeasuredDimension(
            resolveSize(widest + paddingLeft + paddingRight, widthMeasureSpec),
            resolveSize(y + rowHeight + paddingTop + paddingBottom, heightMeasureSpec),
        )
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        val width = right - left - paddingLeft - paddingRight
        val rtl = layoutDirection == View.LAYOUT_DIRECTION_RTL
        var x = 0
        var y = 0
        var rowHeight = 0
        for (index in 0 until childCount) {
            val child = getChildAt(index)
            if (child.visibility == View.GONE) continue
            if (x > 0 && x + child.measuredWidth > width) {
                x = 0
                y += rowHeight + gap
                rowHeight = 0
            }
            val start = if (rtl) width - x - child.measuredWidth else x
            child.layout(
                paddingLeft + start,
                paddingTop + y,
                paddingLeft + start + child.measuredWidth,
                paddingTop + y + child.measuredHeight,
            )
            x += child.measuredWidth + gap
            rowHeight = maxOf(rowHeight, child.measuredHeight)
        }
    }
}

private fun kindName(kind: String): String = when (kind) {
    "song" -> "Song"
    "playlist" -> "Playlist"
    else -> "Video"
}

/**
 * The sheet's panel for what the page offers (the newest first): each one's title (else where it
 * is), what it is and who by, and its buttons: Play in Raygun, Add to queue, Save (songs) and
 * Download. `onAction` gets the offer and `play`, `queue`, `save` or `download`.
 */
internal fun Context.offersPanel(offers: List<PageOffer>, onAction: (PageOffer, String) -> Unit): LinearLayout {
    val panel = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = rounded(PANEL, dp(20).toFloat())
        setPadding(dp(20), dp(20), dp(20), dp(16))
        isClickable = true
    }
    panel.addView(sheetText(13f).apply {
        text = "In Raygun"
        alpha = 0.7f
    })
    val list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
    offers.forEachIndexed { index, offer ->
        if (index > 0) {
            list.addView(View(this).apply { setBackgroundColor(0x1FFFFFFF) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1).apply {
                topMargin = dp(14)
            })
        }
        list.addView(sheetText(16f, bold = true).apply {
            text = offer.label
            maxLines = 2
            setPadding(0, dp(12), 0, dp(2))
        })
        list.addView(sheetText(13f).apply {
            text = listOfNotNull(kindName(offer.kind), offer.artist).joinToString(" · ")
            alpha = 0.7f
        })
        val pills = PillRows(this).apply { setPadding(0, dp(10), 0, 0) }
        val choice = { label: String, action: String, filled: Boolean ->
            pills.addView(pill(label, filled) { onAction(offer, action) }.apply {
                if (!filled) background = rounded(0x22FFFFFF, dp(20).toFloat())
                contentDescription = "$label: ${offer.label}"
            })
        }
        choice("Play in Raygun", "play", true)
        choice("Add to queue", "queue", false)
        if (offer.kind == "song") choice("Save", "save", false)
        choice("Download", "download", false)
        list.addView(pills, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
    }
    val maxHeight = { (resources.displayMetrics.heightPixels * 0.6f).toInt() }
    panel.addView(ShortScroll(this, maxHeight).apply { addView(list) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
    return panel
}

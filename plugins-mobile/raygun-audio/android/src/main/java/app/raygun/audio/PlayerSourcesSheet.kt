package app.raygun.audio

import android.content.res.Configuration
import android.graphics.Color
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

/** What ranks one of a source's options: its place in the source's list and what's known of it. */
internal class OptionRank(
    val index: Int,
    val height: Long? = null,
    val range: String? = null,
    val speedKbps: Long? = null,
    val bitrateKbps: Long? = null,
)

/** Dolby Vision, then HDR, above the same height in SDR. */
private fun rangeRank(range: String?): Int = when (range) {
    "dv" -> 2
    "hdr" -> 1
    else -> 0
}

/**
 * A source's options, best picture first, then the better range, the faster, the higher bit
 * rate, then as listed; what isn't known goes last. Each key compares as one type: an unknown
 * height or speed taken as the Int -1 beside known Longs makes `compareValues` throw a
 * ClassCastException (Integer to Long) once one option has a height or a speed and another
 * doesn't.
 */
internal val OPTION_ORDER: Comparator<OptionRank> = Comparator { a, b ->
    val height = (b.height ?: -1L).compareTo(a.height ?: -1L)
    if (height != 0) return@Comparator height
    val range = rangeRank(b.range).compareTo(rangeRank(a.range))
    if (range != 0) return@Comparator range
    val speed = (b.speedKbps ?: -1L).compareTo(a.speedKbps ?: -1L)
    if (speed != 0) return@Comparator speed
    val bitrate = (b.bitrateKbps ?: -1L).compareTo(a.bitrateKbps ?: -1L)
    if (bitrate != 0) return@Comparator bitrate
    a.index.compareTo(b.index)
}

/**
 * The sources to list, by index: those in Rust's `order` first (once each, only real ones), then
 * any it left out, without those `visible` hides.
 */
internal fun orderedSources(count: Int, order: List<Int>, visible: (Int) -> Boolean): List<Int> {
    val listed = order.filter { it in 0 until count }.distinct()
    return (listed + (0 until count).filter { it !in listed }).filter(visible)
}

/** "25 Mb/s", "800 kb/s". */
internal fun speedText(kbps: Long): String = if (kbps >= 1_000) {
    String.format(Locale.ROOT, "%.1f", kbps / 1000.0).removeSuffix(".0") + " Mb/s"
} else {
    "$kbps kb/s"
}

/** "15.2 GB", "700 MB". */
internal fun sizeText(bytes: Long): String = when {
    bytes >= 1_000_000_000 -> String.format(Locale.ROOT, "%.1f GB", bytes / 1e9)
    else -> "${(bytes / 1_000_000).coerceAtLeast(1)} MB"
}

/**
 * Sources (M44, clean list after v1.6.0): two tabs. **Sources** lists the session's sources by
 * clean names (the addon or server, not the streams' long names) in the order Rust gives
 * (`order`: what works, then the best picture, then measured speed), and beside it (landscape) or
 * after picking one (portrait) that source's options: what their names say (resolution, HDR,
 * codec, sound, size, release) with network data (the bit rate a file of its size needs, the
 * connection speed measured while it played), sorted by quality then speed; a tap plays it.
 * **Raw details** is the full list as the sources name it.
 */
internal class PlayerSourcesSheet(private val host: VideoPlayerActivity) {
    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    private var raw = false
    /** The source whose options show; null follows the one playing. */
    private var picked: Int? = null
    /** Portrait: the picked source's options show instead of the list. */
    private var options = false
    private var sheet: PlayerSheet? = null
    /** Landscape's panes, whose places are kept when new sources come in. */
    private var sourcePane: ScrollView? = null
    private var optionPane: ScrollView? = null
    private var optionPaneFor: Int? = null

    fun open() {
        raw = false
        picked = null
        options = false
        sourcePane = null
        optionPane = null
        build()
    }

    /** New sources from Rust: the sheet showing them is drawn again. */
    fun refresh() {
        if (host.sheetShown(sheet)) build()
    }

    private val groups: JSONArray get() = host.sources

    /**
     * Draws the sheet. Sources answer in many shapes: one this sheet can't draw says so instead
     * of closing the player (the sheet also opens by itself when no source could play).
     */
    private fun build() {
        val before = sheet
        try {
            draw()
        } catch (error: Exception) {
            Log.e("RaygunVideo", "the Sources sheet failed", error)
            // The sheet drawn before (a refresh) and one half drawn go.
            listOfNotNull(before, sheet).forEach { if (host.sheetShown(it)) it.dismiss() }
            host.showHint(label("sourcesUnavailable", "Couldn't show the sources"), 3_000)
        }
    }

    private fun draw() {
        if (groups.length() == 0) {
            host.showHint(label("findingSource", "Finding a source…"))
            host.request("ready")
            return
        }
        val sheet = PlayerSheet(host, label("sources", "Sources"), wide = true)
        this.sheet = sheet
        sheet.chips(
            listOf(label("sourcesTab", "Sources"), label("sourcesRaw", "Raw details")),
            if (raw) 1 else 0,
        ) { tab ->
            raw = tab == 1
            build()
        }
        host.playingFrom?.let { sheet.header(label("playingFrom", "Playing from {n}", it)) }
        if (raw) rawList(sheet) else cleanList(sheet)
        sheet.show()
    }

    // ---- Raw details: the sources and their links as named ----

    private fun rawList(sheet: PlayerSheet) {
        for (group in 0 until groups.length()) {
            val server = groups.optJSONObject(group) ?: continue
            val links = server.optJSONArray("links") ?: JSONArray()
            if (links.length() == 0) {
                if (!visible(server)) continue
                sheet.row(server.optString("name"), stateText(server), onClick = null)
                continue
            }
            sheet.header(server.optString("name") + standing(server)?.let { " · $it" }.orEmpty())
            for (link in 0 until links.length()) {
                val entry = links.optJSONObject(link) ?: continue
                val current = entry.optBoolean("current")
                val note = when {
                    entry.optBoolean("failed") -> label("failed", "failed")
                    entry.optBoolean("worked") -> label("workedLastTime", "worked last time")
                    else -> null
                }
                val row = sheet.row(entry.optString("label"), note, current) { choose(group, link, current) }
                if (current) sheet.reveal(row)
            }
        }
    }

    // ---- Sources: clean names, then each one's options ----

    /** The sources in Rust's order, without the failed and empty ones the settings hide. */
    private fun ordered(): List<Int> {
        val order = host.sourceOrder
            ?.let { order -> (0 until order.length()).map { order.optInt(it, -1) } }
            .orEmpty()
        return orderedSources(groups.length(), order) { index ->
            groups.optJSONObject(index)?.let { visible(it) } == true
        }
    }

    private fun playingGroup(): Int? = (0 until groups.length()).firstOrNull { group ->
        val links = groups.optJSONObject(group)?.optJSONArray("links") ?: return@firstOrNull false
        (0 until links.length()).any { links.optJSONObject(it)?.optBoolean("current") == true }
    }

    /** Sources waiting for a key or sign-in weren't asked: one line counts and names them. */
    private fun setupLine(): String? {
        val waiting = (0 until groups.length())
            .mapNotNull { groups.optJSONObject(it) }
            .filter { it.optString("state") == "setup" && (it.optJSONArray("links")?.length() ?: 0) == 0 }
        if (waiting.isEmpty()) return null
        val count = host.countLabel(
            "sourcesNeedSetup",
            "{n} source needs a key or sign-in",
            "{n} sources need a key or sign-in",
            waiting.size.toString(),
            waiting.size,
        )
        return count + ": " + waiting.joinToString(", ") { it.optString("name") }
    }

    private fun cleanList(sheet: PlayerSheet) {
        val shown = ordered()
        val setup = setupLine()
        if (shown.isEmpty()) {
            sheet.row(setup ?: label("noSourceWorked", "No source could play this"), onClick = null)
            return
        }
        setup?.let { sheet.row(it, onClick = null) }
        val chosen = picked?.takeIf { it in shown } ?: playingGroup()?.takeIf { it in shown } ?: shown.first()
        val landscape = host.resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        if (landscape) {
            val height = (host.resources.displayMetrics.heightPixels * 0.72f).toInt() - host.dp(120)
            // New sources from Rust draw the sheet again: each pane stays where it was scrolled.
            val sourcesAt = sourcePane?.scrollY ?: 0
            val optionsAt = optionPane?.scrollY?.takeIf { optionPaneFor == chosen } ?: 0
            val left = pane { list -> sourceRows(list, shown, chosen) }
            val right = pane { list -> optionRows(list, chosen) }
            sourcePane = left
            optionPane = right
            optionPaneFor = chosen
            left.post { left.scrollTo(0, sourcesAt) }
            right.post { right.scrollTo(0, optionsAt) }
            val panes = LinearLayout(host).apply {
                orientation = LinearLayout.HORIZONTAL
                addView(left, LinearLayout.LayoutParams(0, -1, 0.42f))
                addView(View(host).apply { setBackgroundColor(0x33FFFFFF) }, LinearLayout.LayoutParams(host.dp(1), -1))
                addView(right, LinearLayout.LayoutParams(0, -1, 0.58f))
            }
            sheet.fixed(panes, height.coerceAtLeast(host.dp(160)))
            return
        }
        if (options) {
            sheet.row("‹  " + label("allSources", "All sources")) {
                options = false
                build()
            }
            val server = groups.optJSONObject(chosen) ?: return
            sheet.header(server.optString("name"))
            val list = LinearLayout(host).apply { orientation = LinearLayout.VERTICAL }
            optionRows(list, chosen)
            sheet.custom(list)
            return
        }
        val list = LinearLayout(host).apply { orientation = LinearLayout.VERTICAL }
        sourceRows(list, shown, null)
        sheet.custom(list)
    }

    /** A pane that scrolls on its own. */
    private fun pane(fill: (LinearLayout) -> Unit): ScrollView = ScrollView(host).apply {
        isVerticalScrollBarEnabled = true
        addView(LinearLayout(host).apply {
            orientation = LinearLayout.VERTICAL
            fill(this)
        })
    }

    /** The sources, the `chosen` one marked (landscape: its options show beside them). */
    private fun sourceRows(list: LinearLayout, shown: List<Int>, chosen: Int?) {
        val playing = playingGroup()
        shown.forEach { group ->
            val server = groups.optJSONObject(group) ?: return@forEach
            val links = server.optJSONArray("links")?.length() ?: 0
            val detail = if (links == 0) stateText(server) else sourceDetail(server, group == playing)
            list.addView(paneRow(server.optString("name"), detail, selected = group == chosen) {
                picked = group
                options = true
                build()
            })
        }
    }

    /** "Up to 2160p · 12 options · 25 Mb/s · worked last time". */
    private fun sourceDetail(server: JSONObject, playing: Boolean): String {
        val options = server.optInt("options")
        return listOfNotNull(
            label("sourcePlaying", "Playing").takeIf { playing },
            server.number("best")?.let { label("sourceBest", "Up to {n}", "${it}p") },
            host.countLabel("sourceOptions", "{n} option", "{n} options", options.toString(), options),
            server.number("speedKbps")?.let { speed(it) },
            standing(server),
        ).joinToString(" · ")
    }

    /** A source's options, best picture first (then faster, then higher bit rate). */
    private fun optionRows(list: LinearLayout, group: Int) {
        val server = groups.optJSONObject(group) ?: return
        val links = server.optJSONArray("links") ?: JSONArray()
        if (links.length() == 0) {
            list.addView(paneRow(server.optString("name"), stateText(server), selected = false, onClick = null))
            return
        }
        val ranks = (0 until links.length()).mapNotNull { index ->
            val link = links.optJSONObject(index) ?: return@mapNotNull null
            val facts = link.optJSONObject("facts")
            OptionRank(
                index = index,
                height = facts?.number("height"),
                range = facts?.text("range"),
                speedKbps = link.number("speedKbps"),
                bitrateKbps = link.number("bitrateKbps"),
            )
        }
        ranks.sortedWith(OPTION_ORDER).forEach { rank ->
            val link = links.optJSONObject(rank.index) ?: return@forEach
            val current = link.optBoolean("current")
            list.addView(paneRow(optionTitle(link), optionDetail(link, server), selected = current) {
                choose(group, rank.index, current)
            })
        }
    }

    /** "2160p (4K) · HDR", or the link's own short name when its height isn't known. */
    private fun optionTitle(link: JSONObject): String {
        val facts = link.optJSONObject("facts")
        val height = facts?.number("height")?.toInt()
        val name = when {
            height == null -> link.optString("label").substringBefore(" · ").trim().take(60).ifBlank { label("video", "Video") }
            height >= 4000 -> label("quality8k", "{n}p (8K)", height.toString())
            height >= 2000 -> label("quality4k", "{n}p (4K)", height.toString())
            else -> "${height}p"
        }
        return when (facts?.optString("range")) {
            "hdr" -> "$name · HDR"
            "dv" -> "$name · Dolby Vision"
            else -> name
        }
    }

    /** "HEVC 10-bit · DD+ 5.1 · 15.2 GB · ~20 Mb/s · WEB-DL · playing". */
    private fun optionDetail(link: JSONObject, server: JSONObject): String? {
        val facts = link.optJSONObject("facts") ?: JSONObject()
        val codec = listOfNotNull(
            facts.text("codec"),
            facts.number("bitDepth")?.takeIf { it > 8 }?.let { "$it-bit" },
        ).joinToString(" ").ifBlank { null }
        val measured = link.number("speedKbps") ?: server.number("speedKbps")
        val status = when {
            link.optBoolean("current") -> label("sourcePlaying", "Playing")
            link.optBoolean("failed") -> label("failed", "failed")
            link.optBoolean("worked") -> label("workedLastTime", "worked last time")
            else -> null
        }
        return listOfNotNull(
            codec,
            link.optJSONObject("torrent")?.let { torrent ->
                torrent.number("seeds")?.toInt()?.let { seeds ->
                    host.countLabel("torrentSeeds", "torrent · {n} seed", "torrent · {n} seeds", seeds.toString(), seeds)
                } ?: label("torrent", "torrent")
            },
            facts.text("audio"),
            facts.number("sizeBytes")?.let { size(it) },
            link.number("bitrateKbps")?.let { label("bitrateNeeds", "needs ~{n}", speed(it)) },
            measured?.let { label("speedMeasured", "{n} measured", speed(it)) },
            facts.text("release"),
            status,
            // A torrent playing: its peers and speed.
            host.transferNote?.takeIf { link.optBoolean("current") },
        ).joinToString(" · ").ifBlank { null }
    }

    // ---- Shared ----

    private fun choose(group: Int, link: Int, current: Boolean) {
        sheet?.dismiss()
        if (!current) host.chooseSource(group, link)
    }

    /** Failed and empty sources show only when the settings say so. */
    private fun visible(server: JSONObject): Boolean {
        val links = server.optJSONArray("links")?.length() ?: 0
        if (links > 0) return true
        return when (server.optString("state")) {
            "failed" -> host.prefs.showFailedSources
            "empty" -> host.prefs.showEmptySources
            // Counted in one line instead (`setupLine`).
            "setup" -> false
            else -> true
        }
    }

    /** Why a source has no links: finding them, failed (in its own words, short), none. */
    private fun stateText(server: JSONObject): String = when (server.optString("state")) {
        "resolving" -> label("resolving", "finding links…")
        "failed" -> label("failed", "failed") + (server.text("error")?.let { ": ${it.take(120)}" } ?: "")
        else -> label("empty", "no videos")
    }

    /** It worked last time, or failed lately (M46.17). */
    private fun standing(server: JSONObject): String? = when {
        server.optBoolean("unreliable") -> label("unreliable", "didn't work lately")
        server.optBoolean("worked") -> label("workedLastTime", "worked last time")
        else -> null
    }

    private fun speed(kbps: Long): String = speedText(kbps)

    private fun size(bytes: Long): String = sizeText(bytes)

    /** A row of a pane, in the sheets' style: words, a tick when `selected`. */
    private fun paneRow(text: String, detail: String?, selected: Boolean, onClick: (() -> Unit)?): TextView =
        TextView(host).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(host.dp(20), host.dp(12), host.dp(20), host.dp(12))
            minHeight = host.dp(48)
            gravity = Gravity.CENTER_VERTICAL
            val check = if (selected) "✓  " else ""
            this.text = if (detail.isNullOrBlank()) "$check$text" else "$check$text\n$detail"
            setTextColor(
                when {
                    onClick == null -> 0x80FFFFFF.toInt()
                    selected -> playerAccent
                    else -> Color.WHITE
                },
            )
            if (onClick != null) {
                val ripple = TypedValue()
                host.theme.resolveAttribute(android.R.attr.selectableItemBackground, ripple, true)
                setBackgroundResource(ripple.resourceId)
                isClickable = true
                isFocusable = true
                setOnClickListener { onClick() }
            }
            layoutParams = LinearLayout.LayoutParams(-1, -2)
        }
}

package app.raygun.audio

import android.content.res.Configuration
import android.graphics.Color
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

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

    fun open() {
        raw = false
        picked = null
        options = false
        build()
    }

    /** New sources from Rust: the sheet showing them is drawn again. */
    fun refresh() {
        if (host.sheetShown(sheet)) build()
    }

    private val groups: JSONArray get() = host.sources

    private fun build() {
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
        val all = (0 until groups.length()).toList()
        val listed = host.sourceOrder
            ?.let { order -> (0 until order.length()).map { order.optInt(it, -1) } }
            .orEmpty()
            .filter { it in all }
            .distinct()
        return (listed + all.filter { it !in listed }).filter { index ->
            groups.optJSONObject(index)?.let { visible(it) } == true
        }
    }

    private fun playingGroup(): Int? = (0 until groups.length()).firstOrNull { group ->
        val links = groups.optJSONObject(group)?.optJSONArray("links") ?: return@firstOrNull false
        (0 until links.length()).any { links.optJSONObject(it)?.optBoolean("current") == true }
    }

    private fun cleanList(sheet: PlayerSheet) {
        val shown = ordered()
        if (shown.isEmpty()) {
            sheet.row(label("noSourceWorked", "No source could play this"), onClick = null)
            return
        }
        val chosen = picked?.takeIf { it in shown } ?: playingGroup()?.takeIf { it in shown } ?: shown.first()
        val landscape = host.resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        if (landscape) {
            val height = (host.resources.displayMetrics.heightPixels * 0.72f).toInt() - host.dp(120)
            val panes = LinearLayout(host).apply {
                orientation = LinearLayout.HORIZONTAL
                addView(pane { list -> sourceRows(list, shown, chosen) }, LinearLayout.LayoutParams(0, -1, 0.42f))
                addView(View(host).apply { setBackgroundColor(0x33FFFFFF) }, LinearLayout.LayoutParams(host.dp(1), -1))
                addView(pane { list -> optionRows(list, chosen) }, LinearLayout.LayoutParams(0, -1, 0.58f))
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
        val rangeRank = mapOf("dv" to 2, "hdr" to 1)
        val sorted = (0 until links.length()).mapNotNull { index -> links.optJSONObject(index)?.let { index to it } }
            .sortedWith(
                compareByDescending<Pair<Int, JSONObject>> { (_, link) -> link.optJSONObject("facts")?.number("height") ?: -1 }
                    .thenByDescending { (_, link) -> rangeRank[link.optJSONObject("facts")?.optString("range")] ?: 0 }
                    .thenByDescending { (_, link) -> link.number("speedKbps") ?: -1 }
                    .thenByDescending { (_, link) -> link.number("bitrateKbps") ?: -1 }
                    .thenBy { (index, _) -> index },
            )
        sorted.forEach { (index, link) ->
            val current = link.optBoolean("current")
            list.addView(paneRow(optionTitle(link), optionDetail(link, server), selected = current) {
                choose(group, index, current)
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
            facts.text("audio"),
            facts.number("sizeBytes")?.let { size(it) },
            link.number("bitrateKbps")?.let { label("bitrateNeeds", "needs ~{n}", speed(it)) },
            measured?.let { label("speedMeasured", "{n} measured", speed(it)) },
            facts.text("release"),
            status,
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

    /** "25 Mb/s", "800 kb/s". */
    private fun speed(kbps: Long): String = if (kbps >= 1_000) {
        String.format(Locale.ROOT, "%.1f", kbps / 1000.0).removeSuffix(".0") + " Mb/s"
    } else {
        "$kbps kb/s"
    }

    /** "15.2 GB", "700 MB". */
    private fun size(bytes: Long): String = when {
        bytes >= 1_000_000_000 -> String.format(Locale.ROOT, "%.1f GB", bytes / 1e9)
        else -> "${(bytes / 1_000_000).coerceAtLeast(1)} MB"
    }

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

package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject

/**
 * Whether the quality is automatic: the stream picks its variant by itself (`variantsAuto`), no
 * step was asked for in the menu (Rust's `wanted`) and no link was picked in Sources (`handPicked`).
 */
internal fun automatic(variantsAuto: Boolean, wanted: String?, handPicked: Boolean = false): Boolean =
    variantsAuto && wanted == null && !handPicked

/** A row's name: its tier, the picture's own size beside it when Rust names one ("1080p (1920×816)"). */
internal fun qualityName(height: Int, exact: String?, nickname: String?): String = when {
    exact != null -> "${height}p ($exact)"
    nickname != null -> nickname
    else -> "${height}p"
}

/**
 * The variant tracks a row holds: those whose own sides are among the row's `sizes` (as they
 * were reported), in its range; the best of them plays.
 */
internal fun <T> heldVariants(
    tracks: List<T>,
    sizes: List<Pair<Int, Int>>,
    range: String,
    sides: (T) -> Pair<Int, Int>,
    rangeOf: (T) -> String,
): List<T> = tracks.filter { track ->
    val (width, height) = sides(track)
    rangeOf(track) == range && sizes.any { (w, h) -> w == width.coerceAtLeast(0) && h == height.coerceAtLeast(0) }
}

/** "HEVC", "H.264", "AV1", "VP9" from a codecs string ("hvc1.2.4.L153"). */
internal fun codecName(codecs: String?): String? {
    val codec = codecs?.split(',')?.map { it.trim().lowercase() }?.firstOrNull { !it.startsWith("mp4a") } ?: return null
    return when {
        codec.startsWith("hvc1") || codec.startsWith("hev1") || codec.startsWith("dvh") -> "HEVC"
        codec.startsWith("avc") || codec.startsWith("dva") -> "H.264"
        codec.startsWith("av01") || codec.startsWith("dav1") -> "AV1"
        codec.startsWith("vp09") || codec.startsWith("vp9") -> "VP9"
        else -> null
    }
}

/** `sdr`, `hdr` or `dv` for a video format. */
@OptIn(UnstableApi::class)
internal fun rangeOf(format: Format): String {
    val codecs = format.codecs?.lowercase().orEmpty()
    if (codecs.contains("dvh") || codecs.contains("dva") || codecs.contains("dav1") ||
        format.sampleMimeType == "video/dolby-vision"
    ) {
        return "dv"
    }
    return when (format.colorInfo?.colorTransfer) {
        C.COLOR_TRANSFER_ST2084, C.COLOR_TRANSFER_HLG -> "hdr"
        else -> "sdr"
    }
}

/**
 * The Quality button's sheet: the tiers the stream and the sources have, largest first, as Rust
 * lists them (`qualities` updates, `video_quality::rows` in raygun-app), each with the picture's
 * own size when it isn't the tier's plain one ("1080p (1920×816)"). A row the stream playing has
 * (an HLS or DASH variant) switches here; one the source playing has as another link, or only
 * another source has, is asked of Rust (`quality`), which plays it from where the video is.
 * The row picked, tapped again, lists every link with it to choose one. The stream's variants go
 * to Rust with their own sides as they become known (`variants`). While automatic, stalls step
 * the quality down ([lookAtStalls]).
 */
internal class PlayerQuality(private val host: VideoPlayerActivity) {
    private var rows = JSONArray()
    /** The step asked for in this menu (Rust's `wanted`); null while it picks by itself. */
    private var wanted: String? = null
    private var sheet: PlayerSheet? = null
    private var reported = ""
    /** A link was picked by hand (Rust's `handPicked`): automatic quality leaves it be. */
    private var handPicked = false
    /** The row whose links show in the sheet (its key): the row picked, tapped again. */
    private var listed: String? = null
    private val stalls = StallWatch()
    /** The cap on the stream's variants while stepped down (a tier); null at the preferred one. */
    private var loweredCap: Int? = null
    /**
     * A step down to another link says so once that link shows its first frame: the load it
     * brings clears hints and covers the picture with its ring, which would hide it.
     */
    private var loweredNotice: String? = null

    /** Whether the button has anything to offer: a session's sources, or variants to pick. */
    val offered: Boolean get() = host.session || host.tracks.qualities().size > 1

    /** Rust's `qualities` update. */
    fun onUpdate(update: JSONObject) {
        rows = update.optJSONArray("items") ?: JSONArray()
        wanted = update.text("wanted")
        handPicked = update.optBoolean("handPicked")
        update.text("notice")?.let {
            // Nothing loads (no other link has that step): the notice says so instead.
            loweredNotice = null
            host.pickedNothing()
            host.showHint(it, 3_000)
        }
        if (host.sheetShown(sheet)) open()
    }

    /** A new video: its variants are reported once known. */
    fun onLoad() {
        host.qualityHold.held = false
        reported = ""
        rows = JSONArray()
        listed = null
        // Each video starts afresh: its stalls are its own; the load applies the preferred cap.
        stalls.reset(android.os.SystemClock.elapsedRealtime())
        loweredCap = null
    }

    /** The first frame of a load: a step down that brought it is said now. */
    fun onFirstFrame() {
        val notice = loweredNotice ?: return
        loweredNotice = null
        host.showHint(notice, 2_500)
    }

    /** Rust's rows as the steps see them. */
    private fun steps(): List<QualityStep> = (0 until rows.length()).mapNotNull { index ->
        val row = rows.optJSONObject(index) ?: return@mapNotNull null
        val sizes = row.optJSONArray("sizes") ?: JSONArray()
        QualityStep(
            key = row.optString("key"),
            height = row.optInt("height"),
            range = row.optString("range", "sdr"),
            state = row.optString("state"),
            sizes = (0 until sizes.length()).mapNotNull { at ->
                sizes.optJSONObject(at)?.let { it.optInt("width") to it.optInt("height") }
            },
        )
    }

    /**
     * Automatic quality that keeps the video playing ([StallWatch]), looked at every tick: while
     * nothing was picked by hand (a step here, a variant held, a link chosen in Sources), stalls
     * cap an HLS or DASH stream a variant lower, or ask Rust for a lower step's link from where
     * the video is (`quality` with `auto`); a long smooth stretch lifts the cap a step at a time.
     * Never below the lowest.
     */
    fun lookAtStalls(now: Long, playing: Boolean, stalled: Boolean) {
        val locked = wanted != null || handPicked || !host.tracks.qualityAuto()
        if (locked) return
        val adaptive = host.tracks.qualities().size > 1
        val preferred = host.prefs.maxHeight ?: Int.MAX_VALUE
        when (stalls.tick(now, playing, stalled, adaptive && loweredCap != null)) {
            StallAction.LOWER -> {
                val step = lowerStep(steps(), adaptive) ?: return
                if (adaptive) {
                    val cap = capOf(step)
                    if (cap >= preferred) return
                    host.tracks.limit(cap)
                    loweredCap = cap
                }
                val notice = host.label("qualityLowered", "Lowered to {n} to keep playing", stepName(step))
                if (!adaptive) {
                    if (!host.session) return
                    loweredNotice = notice
                    host.request("quality", JSONObject().put("key", step.key).put("auto", true))
                }
                stalls.stepped(now)
                host.showHint(notice, 2_500)
            }
            StallAction.RAISE -> {
                val cap = loweredCap ?: return
                val next = raiseStep(steps(), cap)?.let { capOf(it) }?.takeIf { it < preferred }
                host.tracks.limit(next ?: host.prefs.maxHeight)
                loweredCap = next
                stalls.stepped(now)
            }
            null -> Unit
        }
    }

    /**
     * Tells Rust the stream's variants and the one playing, when they changed: their own sides
     * (0 when unknown), which Rust groups into tiers.
     */
    fun reportVariants() {
        val items = JSONArray()
        host.tracks.qualities().forEach { track ->
            val format = track.format
            if (format.height <= 0 && format.width <= 0) return@forEach
            items.put(
                JSONObject()
                    .put("width", format.width.coerceAtLeast(0))
                    .put("height", format.height.coerceAtLeast(0))
                    .put("range", rangeOf(format))
                    .put("codec", codecName(format.codecs) ?: JSONObject.NULL),
            )
        }
        val playing = host.player.videoFormat?.takeIf { it.height > 0 || it.width > 0 }?.let { format ->
            JSONObject()
                .put("width", format.width.coerceAtLeast(0))
                .put("height", format.height.coerceAtLeast(0))
                .put("range", rangeOf(format))
        }
        val json = JSONObject().put("items", items).put("playing", playing ?: JSONObject.NULL)
        val text = json.toString()
        if (text == reported) return
        reported = text
        host.request("variants", json)
    }

    private fun stepName(row: JSONObject): String =
        stepName(row.optInt("height"), row.optString("range"), row.text("exact"))

    private fun stepName(step: QualityStep): String = stepName(step.height, step.range, null)

    private fun stepName(height: Int, range: String, exact: String?): String {
        val nickname = when (height) {
            4320 -> host.label("quality8k", "{n}p (8K)", height.toString())
            2160 -> host.label("quality4k", "{n}p (4K)", height.toString())
            else -> null
        }
        val name = qualityName(height, exact, nickname)
        return when (range) {
            "hdr" -> "$name · HDR"
            "dv" -> "$name · Dolby Vision"
            else -> name
        }
    }

    /**
     * The sheet. Automatic is ticked while nothing was picked here; the row playing is always
     * set apart (on a tint of the accent, "Playing" under it), and ticked too when it was picked.
     * The preferred resolution's row comes first with a small "Preferred" tag (Rust's order); the
     * row picked, tapped again, lists every link with that tier to pick one (`alternatives`).
     */
    fun open() {
        // Drawn again over itself (a row's links shown, an update): it stays where it was.
        val before = sheet?.takeIf { host.sheetShown(it) }?.scrolled ?: 0
        val sheet = PlayerSheet(host, host.label("quality", "Quality"))
        this.sheet = sheet
        val auto = automatic(host.tracks.qualityAuto(), wanted, handPicked)
        sheet.row(host.label("qualityAuto", "Automatic"), selected = auto, highlighted = false) {
            host.tracks.selectQuality(null)
            if (host.session) host.request("quality", JSONObject().put("key", JSONObject.NULL))
            sheet.dismiss()
        }
        if (rows.length() == 0) {
            sheet.row(host.label("qualityWaiting", "Finding qualities…"), onClick = null)
        }
        for (index in 0 until rows.length()) {
            val row = rows.optJSONObject(index) ?: continue
            val codec = row.text("codec")
            val state = row.optString("state")
            val key = row.optString("key")
            val alternatives = row.optJSONArray("alternatives") ?: JSONArray()
            val several = state == "playing" && !auto && alternatives.length() > 1
            val more = alternatives.length() - 1
            val detail = when (state) {
                "playing" -> listOfNotNull(
                    host.label("qualityPlaying", "Playing"),
                    codec,
                    if (several) {
                        host.countLabel("otherCopiesPick", "{n} more link: tap to pick", "{n} more links: tap to pick", more.toString(), more)
                    } else {
                        null
                    },
                ).joinToString(" · ")
                "otherSource" -> host.label(
                    "qualityElsewhere",
                    "Not in this source · plays from {n}",
                    row.optString("source"),
                )
                else -> codec
            }
            val onClick: (() -> Unit)? = when (state) {
                "variant" -> { { pickVariant(row); sheet.dismiss() } }
                // While automatic, the row playing is kept as it is (nothing loads again).
                "playing" -> when {
                    auto -> { { keepPlaying(row); sheet.dismiss() } }
                    // Picked, and tapped again: its links, to choose one.
                    several -> { { listed = if (listed == key) null else key; open() } }
                    host.session -> { { sheet.dismiss() } }
                    else -> null
                }
                else -> { { ask(row); sheet.dismiss() } }
            }
            val playing = state == "playing"
            val shown = sheet.row(
                stepName(row),
                detail,
                selected = playing && !auto,
                highlighted = playing,
                badge = if (row.optBoolean("preferred")) host.label("preferredOptions", "Preferred") else null,
                onClick = onClick,
            )
            if (playing && before == 0) sheet.reveal(shown)
            if (several && listed == key) linkRows(sheet, row, alternatives)
        }
        sheet.scrolled = before
        sheet.show()
    }

    /** A row's links, each with its source and what's known of it; a tap plays it from here. */
    private fun linkRows(sheet: PlayerSheet, row: JSONObject, alternatives: JSONArray) {
        sheet.header(host.label("linksWith", "Links with {n}", stepName(row.optInt("height"), row.optString("range"), null)))
        for (index in 0 until alternatives.length()) {
            val place = alternatives.optJSONObject(index) ?: continue
            val group = place.optInt("group", -1)
            val link = place.optInt("link", -1)
            val server = host.sources.optJSONObject(group) ?: continue
            val entry = server.optJSONArray("links")?.optJSONObject(link) ?: continue
            val current = entry.optBoolean("current")
            sheet.row(
                "   " + host.sourcesSheet.optionTitle(entry) + " · " + server.optString("name"),
                host.sourcesSheet.alternativeDetail(entry, server),
                selected = current,
            ) {
                sheet.dismiss()
                if (!current) host.chooseSource(group, link)
            }
        }
    }

    /** The stream's own variant of this row: the best of those it holds ([heldVariants]). */
    private fun pickVariant(row: JSONObject) {
        val list = row.optJSONArray("sizes") ?: JSONArray()
        val sizes = (0 until list.length()).mapNotNull { index ->
            list.optJSONObject(index)?.let { it.optInt("width") to it.optInt("height") }
        }
        val track = heldVariants(
            host.tracks.qualities(),
            sizes,
            row.optString("range"),
            { it.format.width to it.format.height },
            { rangeOf(it.format) },
        ).maxByOrNull { it.format.bitrate } ?: return
        host.tracks.selectQuality(track)
    }

    /**
     * The row playing kept: the variant on screen is held ([QualityHold]: the adaptive selection
     * stops switching; a track override would make an HLS stream load again), and a session
     * keeps the step without loading anything (`keep`).
     */
    private fun keepPlaying(row: JSONObject) {
        if (host.tracks.qualities().size > 1) host.tracks.holdQuality()
        if (host.session) {
            host.request("quality", JSONObject().put("key", row.optString("key")).put("keep", true))
        }
    }

    /** Asks Rust for another link with this step; the ring names the source while it loads. */
    private fun ask(row: JSONObject) {
        val source = row.text("source") ?: host.playingSourceName()
        host.pickedQuality(source, stepName(row), again = false)
        host.request("quality", JSONObject().put("key", row.optString("key")))
    }
}

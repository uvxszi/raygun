package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject

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
 * Tapping the row that plays again tries the next source with it. The stream's variants go to
 * Rust with their own sides as they become known (`variants`).
 */
internal class PlayerQuality(private val host: VideoPlayerActivity) {
    private var rows = JSONArray()
    private var sheet: PlayerSheet? = null
    private var reported = ""

    /** Whether the button has anything to offer: a session's sources, or variants to pick. */
    val offered: Boolean get() = host.session || host.tracks.qualities().size > 1

    /** Rust's `qualities` update. */
    fun onUpdate(update: JSONObject) {
        rows = update.optJSONArray("items") ?: JSONArray()
        update.text("notice")?.let {
            host.pickedNothing()
            host.showHint(it, 3_000)
        }
        if (host.sheetShown(sheet)) open()
    }

    /** A new video: its variants are reported once known. */
    fun onLoad() {
        reported = ""
        rows = JSONArray()
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

    private fun stepName(row: JSONObject): String {
        val height = row.optInt("height")
        val nickname = when (height) {
            4320 -> host.label("quality8k", "{n}p (8K)", height.toString())
            2160 -> host.label("quality4k", "{n}p (4K)", height.toString())
            else -> null
        }
        val name = qualityName(height, row.text("exact"), nickname)
        return when (row.optString("range")) {
            "hdr" -> "$name · HDR"
            "dv" -> "$name · Dolby Vision"
            else -> name
        }
    }

    fun open() {
        val sheet = PlayerSheet(host, host.label("quality", "Quality"))
        this.sheet = sheet
        val auto = host.tracks.qualityAuto()
        sheet.row(host.label("qualityAuto", "Automatic"), selected = auto && host.tracks.qualities().size > 1) {
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
            val detail = when (state) {
                "playing" -> listOfNotNull(host.label("qualityPlaying", "Playing"), codec).joinToString(" · ")
                "otherSource" -> host.label(
                    "qualityElsewhere",
                    "Not in this source · plays from {n}",
                    row.optString("source"),
                )
                else -> codec
            }
            val onClick: (() -> Unit)? = when (state) {
                "variant" -> { { pickVariant(row); sheet.dismiss() } }
                "playing" -> if (host.session) { { ask(row, again = true); sheet.dismiss() } } else null
                else -> { { ask(row, again = false); sheet.dismiss() } }
            }
            val shown = sheet.row(stepName(row), detail, selected = state == "playing" && !auto, onClick = onClick)
            if (state == "playing") sheet.reveal(shown)
        }
        sheet.show()
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

    /** Asks Rust for another link with this step; the ring names the source while it loads. */
    private fun ask(row: JSONObject, again: Boolean) {
        val source = row.text("source") ?: host.playingSourceName()
        host.pickedQuality(source, stepName(row), again)
        host.request("quality", JSONObject().put("key", row.optString("key")))
    }
}

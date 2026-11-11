package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject

/** The resolution ladder, as `video_quality::LADDER` in raygun-app. */
private val LADDER = listOf(4320, 2160, 1440, 1080, 720, 480, 360, 240, 144)

/** The ladder step a height counts as: the nearest, ties going up. */
internal fun ladderStep(height: Int): Int =
    LADDER.minWith(compareBy<Int> { kotlin.math.abs(it - height) }.thenByDescending { it })

/** The height a picture counts as: what a 16:9 one with the same long side has (1920×800 is 1080p). */
internal fun pictureHeight(width: Int, height: Int): Int =
    if (width > 0 && height > 0) ((maxOf(width, height).toLong() * 9 + 8) / 16).toInt() else height

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
 * The Quality button's sheet: resolutions from 4K down, as Rust lists them (`qualities` updates,
 * `video_quality::rows` in raygun-app). A step the stream playing has (an HLS or DASH variant)
 * switches here; one the source playing has as another link, or only another source has, is
 * asked of Rust (`quality`), which plays it from where the video is. Tapping the step that plays
 * again tries the next source with it. The stream's variants go to Rust as they become known
 * (`variants`).
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

    /** Tells Rust the stream's variants and the one playing, when they changed. */
    fun reportVariants() {
        val items = JSONArray()
        host.tracks.qualities().forEach { track ->
            val format = track.format
            if (format.height <= 0) return@forEach
            items.put(
                JSONObject()
                    .put("height", pictureHeight(format.width, format.height))
                    .put("range", rangeOf(format))
                    .put("codec", codecName(format.codecs) ?: JSONObject.NULL),
            )
        }
        val playing = host.player.videoFormat?.takeIf { it.height > 0 }?.let { format ->
            JSONObject()
                .put("height", pictureHeight(format.width, format.height))
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
        val name = when (height) {
            4320 -> host.label("quality8k", "{n}p (8K)", height.toString())
            2160 -> host.label("quality4k", "{n}p (4K)", height.toString())
            else -> "${height}p"
        }
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
                "missing" -> host.label("qualityMissing", "No source has it")
                else -> codec
            }
            val onClick: (() -> Unit)? = when (state) {
                "missing" -> null
                "variant" -> { { pickVariant(row); sheet.dismiss() } }
                "playing" -> if (host.session) { { ask(row, again = true); sheet.dismiss() } } else null
                else -> { { ask(row, again = false); sheet.dismiss() } }
            }
            val shown = sheet.row(stepName(row), detail, selected = state == "playing" && !auto, onClick = onClick)
            if (state == "playing") sheet.reveal(shown)
        }
        sheet.show()
    }

    /** The stream's own variant of this step: the best one of its height and range. */
    private fun pickVariant(row: JSONObject) {
        val height = row.optInt("height")
        val range = row.optString("range")
        val track = host.tracks.qualities()
            .filter { ladderStep(pictureHeight(it.format.width, it.format.height)) == height && rangeOf(it.format) == range }
            .maxByOrNull { it.format.bitrate }
            ?: return
        host.tracks.selectQuality(track)
    }

    /** Asks Rust for another link with this step; the ring names the source while it loads. */
    private fun ask(row: JSONObject, again: Boolean) {
        val source = row.text("source") ?: host.playingSourceName()
        host.pickedQuality(source, stepName(row), again)
        host.request("quality", JSONObject().put("key", row.optString("key")))
    }
}

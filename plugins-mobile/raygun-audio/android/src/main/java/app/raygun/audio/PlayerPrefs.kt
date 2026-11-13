package app.raygun.audio

import android.content.pm.ActivityInfo
import org.json.JSONArray
import org.json.JSONObject

/** The text colour subtitles have unless one is picked. */
internal const val DEFAULT_TEXT_COLOR = 0xFFFFFFFF.toInt()

/** How wide subtitle lines get unless changed, in percent of the picture's width. */
internal const val DEFAULT_MAX_WIDTH = 80

/** How subtitles look (`SubtitleStyle` in raygun-model). Colours are 0xAARRGGBB. */
internal data class SubtitleLook(
    val size: Int = 100,
    val font: String = "default",
    val bold: Boolean = false,
    val italic: Boolean = false,
    val textColor: Int = DEFAULT_TEXT_COLOR,
    /** The text colour was picked, white included. */
    val textColorChosen: Boolean = false,
    val edge: String = "outline",
    val edgeColor: Int = 0xFF000000.toInt(),
    val backgroundColor: Int = 0,
    /** The background was picked, "None" included. */
    val backgroundColorChosen: Boolean = false,
    val position: Int = 8,
    /** The widest a line gets, in percent of the picture's width; longer ones wrap. */
    val maxWidth: Int = DEFAULT_MAX_WIDTH,
    val ignoreEmbedded: Boolean = false,
) {
    /**
     * The text colour wins over the colours a file gives its lines (an ASS style's, a TTML
     * region's): once picked, white too. Only an unpicked white leaves them, as the default.
     */
    val ownText: Boolean get() = textColorChosen || textColor != DEFAULT_TEXT_COLOR

    /** The background replaces the ones a file sets: once picked, "None" too. */
    val ownBackground: Boolean get() = backgroundColorChosen || backgroundColor != 0

    /** `color` picked for the text. */
    fun withText(color: Int): SubtitleLook = copy(textColor = color, textColorChosen = true)

    /** `color` picked for the background. */
    fun withBackground(color: Int): SubtitleLook = copy(backgroundColor = color, backgroundColorChosen = true)

    fun toJson(): JSONObject = JSONObject()
        .put("size", size)
        .put("font", font)
        .put("bold", bold)
        .put("italic", italic)
        .put("textColor", textColor.toLong() and 0xFFFFFFFFL)
        .put("textColorChosen", textColorChosen)
        .put("edge", edge)
        .put("edgeColor", edgeColor.toLong() and 0xFFFFFFFFL)
        .put("backgroundColor", backgroundColor.toLong() and 0xFFFFFFFFL)
        .put("backgroundColorChosen", backgroundColorChosen)
        .put("position", position)
        .put("maxWidth", maxWidth)
        .put("ignoreEmbedded", ignoreEmbedded)

    companion object {
        fun from(json: JSONObject?): SubtitleLook {
            val defaults = SubtitleLook()
            json ?: return defaults
            return SubtitleLook(
                size = json.optInt("size", defaults.size),
                font = json.optString("font", defaults.font),
                bold = json.optBoolean("bold", defaults.bold),
                italic = json.optBoolean("italic", defaults.italic),
                textColor = json.optLong("textColor", 0xFFFFFFFFL).toInt(),
                textColorChosen = json.optBoolean("textColorChosen", false),
                edge = json.optString("edge", defaults.edge),
                edgeColor = json.optLong("edgeColor", 0xFF000000L).toInt(),
                backgroundColor = json.optLong("backgroundColor", 0L).toInt(),
                backgroundColorChosen = json.optBoolean("backgroundColorChosen", false),
                position = json.optInt("position", defaults.position),
                maxWidth = json.optInt("maxWidth", defaults.maxWidth).coerceIn(40, 100),
                ignoreEmbedded = json.optBoolean("ignoreEmbedded", defaults.ignoreEmbedded),
            )
        }
    }
}

/**
 * Settings → Player as the player reads it (`VideoPlayerPrefs` in raygun-model, M45). The fields
 * the player itself can change are `var`; the change is also sent to Rust to be saved.
 */
internal class PlayerPrefs(json: JSONObject) {
    val orientation: String = json.optString("orientation", "free")
    val aspect: String = json.optString("aspect", "fit")
    val controlsMs: Long = json.optInt("controlsSeconds", 4).coerceIn(1, 30) * 1000L
    var showRemaining: Boolean = json.optBoolean("showRemaining", false)
    var defaultSpeed: Float = json.optInt("defaultSpeed", 100) / 100f
    val speedPresets: List<Float> = json.optJSONArray("speedPresets").numbers()
        .map { it / 100f }
        .ifEmpty { listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 1.75f, 2f) }
    var pitchCorrection: Boolean = json.optBoolean("pitchCorrection", true)
    /** 0 turns holding for speed off. */
    val holdSpeed: Float = json.optInt("holdSpeed", 200) / 100f
    val rememberBrightness: Boolean = json.optBoolean("rememberBrightness", false)
    /** The brightness to start at, when remembered. */
    val brightness: Float? = if (json.has("brightness")) json.optDouble("brightness").toFloat() else null

    var brightnessGesture: Boolean = json.optBoolean("brightnessGesture", true)
    var volumeGesture: Boolean = json.optBoolean("volumeGesture", true)
    val swapSides: Boolean = json.optBoolean("swapSides", false)
    var seekGesture: Boolean = json.optBoolean("seekGesture", true)
    val doubleTapLeft: String = json.optString("doubleTapLeft", "seek")
    val doubleTapCenter: String = json.optString("doubleTapCenter", "playPause")
    val doubleTapRight: String = json.optString("doubleTapRight", "seek")
    val mediaKeys: String = json.optString("mediaKeys", "switchEpisode")
    /** How far past 100 % the volume can go, as a fraction (0.3 = +30 %). */
    val volumeBoost: Float = json.optInt("volumeBoost", 30).coerceIn(0, 100) / 100f

    val skipButton: Boolean = json.optBoolean("skipButton", true)
    val introMs: Long = json.optInt("introSeconds", 85) * 1000L
    val autoSkip: Boolean = json.optBoolean("autoSkip", false)
    val skipCountdown: Boolean = json.optBoolean("skipCountdown", false)
    val skipWaitMs: Long = json.optInt("skipWaitSeconds", 5) * 1000L

    val subtitleFallback: String = json.optString("subtitleFallback")
    val preferWords: List<String> = words(json.optString("subtitlePreferWords"))
    val skipWords: List<String> = words(json.optString("subtitleSkipWords"))
    var style: SubtitleLook = SubtitleLook.from(json.optJSONObject("subtitleStyle"))
    var audioLanguage: String = json.optString("audioLanguage")
    val audioFallback: String = json.optString("audioFallback")

    val pipOnLeave: Boolean = json.optBoolean("pipOnLeave", true)
    val pipOnBack: Boolean = json.optBoolean("pipOnBack", false)

    var decoder: String = json.optString("decoder", "auto")
    /** `VideoCodec` in raygun-model: H.264 over VP9 and AV1 where a stream offers it. */
    val preferH264: Boolean = json.optString("videoCodec", "auto") == "h264"
    val externalPlayer: Boolean = json.optBoolean("externalPlayer", false)
    val externalPackage: String = json.optString("externalPackage")
    val showFailedSources: Boolean = json.optBoolean("showFailedSources", true)
    val showEmptySources: Boolean = json.optBoolean("showEmptySources", true)

    /** The largest picture height automatic quality may use ("1080p" → 1080); null for the highest. */
    val maxHeight: Int? get() = preferredResolution.let { value ->
        if (value == "highest") null else value.removeSuffix("p").toIntOrNull() ?: 1080
    }
    /** `VideoResolution` in raygun-model: "highest" or a height such as "1080p". */
    var preferredResolution: String = json.optString("preferredResolution", "1080p")

    private companion object {
        fun words(text: String): List<String> =
            text.split(',').map { it.trim().lowercase() }.filter { it.isNotEmpty() }

        fun JSONArray?.numbers(): List<Int> {
            this ?: return emptyList()
            return (0 until length()).mapNotNull { index -> optInt(index, -1).takeIf { it > 0 } }
        }
    }
}

/**
 * A track picked by language and name, so the same one is found in the next episode; a subtitle
 * track's `kind` ([SubtitleKind]) finds its like in another title.
 */
internal data class TrackChoice(val language: String?, val label: String?, val kind: String? = null) {
    fun toJson(): JSONObject = JSONObject()
        .put("language", language ?: JSONObject.NULL)
        .put("label", label ?: JSONObject.NULL)
        .apply { if (kind != null) put("kind", kind) }

    companion object {
        fun from(json: JSONObject?): TrackChoice? {
            json ?: return null
            val language = json.optString("language").takeIf { it.isNotBlank() && it != "null" }
            val label = json.optString("label").takeIf { it.isNotBlank() && it != "null" }
            val kind = json.optString("kind").takeIf { it.isNotBlank() && it != "null" }
            return if (language == null && label == null) null else TrackChoice(language, label, kind)
        }
    }
}

/**
 * What a title remembers (`video.playerState.<group>` in Rust): speed (hundredths), aspect, the
 * subtitle choice (`{"off": true}` or a track), the second subtitle track, the subtitle delay,
 * the audio track, the audio delay, another source's sound and the intro length. `last` is the
 * last subtitle choice made in any title, for a title without its own ([carried]).
 */
internal class TitleState(json: JSONObject?, last: JSONObject? = null) {
    /** The last choice in any title, where this title has made none of its own. */
    val carried = CarriedSubtitles(json, last)
    var speed: Float? = json?.optInt("speed", 0)?.takeIf { it > 0 }?.let { it / 100f }
    var aspect: String? = json?.optString("aspect")?.takeIf { it.isNotBlank() }
    var subtitleOff: Boolean = json?.optJSONObject("subtitle")?.optBoolean("off") == true
    var subtitle: TrackChoice? = TrackChoice.from(json?.optJSONObject("subtitle"))
    var secondary: TrackChoice? = TrackChoice.from(json?.optJSONObject("secondarySubtitle"))
    var subtitleDelayMs: Long = json?.optLong("subtitleDelayMs", 0L) ?: 0L
    var audio: TrackChoice? = TrackChoice.from(json?.optJSONObject("audio"))
    /** Positive plays the sound later than the picture ([PlayerAudio]). */
    var audioDelayMs: Long = json?.optLong("audioDelayMs", 0L) ?: 0L
    /** Sound from another source, taken again when the same source offers the same language. */
    var externalAudio: RememberedAudio? = RememberedAudio.from(json?.optJSONObject("externalAudio"))
    /** 0 hides the skip button's jump for this title. */
    var introMs: Long? = json?.let { if (it.has("introMs")) it.optLong("introMs") else null }
}

/** A chapter (`VideoChapter` in raygun-model): `kind` is opening, ending, recap or other. */
internal data class Chapter(val startMs: Long, val endMs: Long?, val name: String?, val kind: String) {
    /** Openings, endings and recaps can be skipped. */
    val skippable: Boolean get() = kind == "opening" || kind == "ending" || kind == "recap"

    companion object {
        fun list(array: JSONArray?): List<Chapter> {
            array ?: return emptyList()
            return (0 until array.length()).mapNotNull { index ->
                val item = array.optJSONObject(index) ?: return@mapNotNull null
                Chapter(
                    startMs = item.optLong("startMs", 0L),
                    endMs = if (item.isNull("endMs")) null else item.optLong("endMs"),
                    name = item.optString("name").takeIf { it.isNotBlank() && it != "null" },
                    kind = item.optString("kind", "other"),
                )
            }.sortedBy { it.startMs }
        }
    }
}

/**
 * The screen orientation the player asks for under Settings → Player → Orientation
 * (`PlayerOrientation`). Following the device (`free`, the default) turns as the system does:
 * every side while auto-rotate is on, the way it's held while it's off. `video` starts in
 * landscape until the video's size is known; anything unknown follows the device.
 */
internal fun screenOrientationFor(name: String): Int = when (name) {
    "portrait" -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    "reversePortrait" -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
    "sensorPortrait" -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
    "landscape" -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
    "reverseLandscape" -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
    "sensorLandscape", "video" -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    else -> ActivityInfo.SCREEN_ORIENTATION_FULL_USER
}

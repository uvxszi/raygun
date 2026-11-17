package app.raygun.audio

import android.content.res.ColorStateList
import android.graphics.Color
import android.util.TypedValue
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import androidx.media3.common.util.UnstableApi
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale
import kotlin.math.abs
import kotlin.math.pow
import kotlin.math.roundToInt

/** Pitch in tenths of a semitone as a frequency ratio (Media3's `PlaybackParameters.pitch`). */
fun pitchRatio(tenths: Int): Float = 2.0.pow(tenths.coerceIn(-120, 120) / 120.0).toFloat()

/**
 * A title's sound in the video player (`VideoSound` in raygun-model, `videoSound` in its memory):
 * pitch in tenths of a semitone (the tempo is the title's speed), and an equalizer of its own or
 * the music player's (`musicEq` on every request).
 */
internal data class VideoSound(
    val pitch: Int = 0,
    val ownEq: Boolean = false,
    val eqEnabled: Boolean = false,
    val eqPreset: String = "flat",
    val eqBands: List<Int> = List(EQ_FREQUENCIES.size) { 0 },
    val preamp: Int = 0,
) {
    val neutral: Boolean get() = pitch == 0 && !ownEq

    fun toJson(): JSONObject = JSONObject()
        .put("pitch", pitch)
        .put("ownEq", ownEq)
        .put("eqEnabled", eqEnabled)
        .put("eqPreset", eqPreset)
        .put("eqBands", JSONArray(eqBands))
        .put("preamp", preamp)

    companion object {
        fun from(json: JSONObject?): VideoSound? {
            json ?: return null
            val bands = json.optJSONArray("eqBands")
            return VideoSound(
                pitch = json.optInt("pitch", 0).coerceIn(-120, 120),
                ownEq = json.optBoolean("ownEq", false),
                eqEnabled = json.optBoolean("eqEnabled", false),
                eqPreset = json.optString("eqPreset", "flat"),
                eqBands = List(EQ_FREQUENCIES.size) { (bands?.optInt(it, 0) ?: 0).coerceIn(-120, 120) },
                preamp = json.optInt("preamp", 0).coerceIn(-120, 120),
            )
        }
    }
}

/** How a title's sound maps onto the player, the same as the desktop's (`videoSound.ts`). */
internal object VideoSoundMath {
    /** The equalizer that applies: the title's own, else the music player's (`musicEq`). */
    fun equalizer(sound: VideoSound, musicEq: JSONObject?): SoundConfig {
        val source = if (sound.ownEq) sound.toJson() else musicEq ?: JSONObject()
        val bands = source.optJSONArray("eqBands")
        return SoundConfig(
            eqEnabled = source.optBoolean("eqEnabled", false),
            bands = DoubleArray(EQ_FREQUENCIES.size) { (bands?.optDouble(it, 0.0) ?: 0.0) / 10 },
            preampDb = source.optDouble("preamp", 0.0) / 10,
        )
    }

    /** The pitch as the sheet says it: "+1.5", "−3", "0". */
    fun pitchText(tenths: Int): String {
        val semitones = abs(tenths) / 10.0
        val number = if (tenths % 10 == 0) (abs(tenths) / 10).toString() else String.format(Locale.ROOT, "%.1f", semitones)
        return when {
            tenths > 0 -> "+$number"
            tenths < 0 -> "−$number"
            else -> "0"
        }
    }

    /** Presets offered for a title's own equalizer (`sound/presets.ts`' values), tenths of a dB. */
    val PRESETS: List<Triple<String, String, List<Int>>> = listOf(
        Triple("flat", "Flat", List(10) { 0 }),
        Triple("bassBoost", "Bass boost", listOf(80, 64, 48, 32, 16, 0, 0, 0, 0, 0)),
        Triple("trebleBoost", "Treble boost", listOf(0, 0, 0, 0, 0, 10, 30, 50, 70, 80)),
        Triple("vocal", "Vocal", listOf(-16, 0, 16, 48, 64, 64, 48, 32, 16, 0)),
        Triple("spokenWord", "Spoken word", listOf(-40, -30, -10, 10, 30, 40, 40, 30, 10, -20)),
        Triple("loudness", "Loudness", listOf(50, 40, 10, 0, -10, 0, 0, 10, 40, 50)),
        Triple("smallSpeakers", "Small speakers", listOf(-60, -40, 10, 40, 30, 10, 0, 10, 20, 20)),
    )

    /** Tempo steps the sheet's slider moves in, as speeds. */
    const val TEMPO_MIN = 0.5f
    const val TEMPO_MAX = 2f
    const val TEMPO_STEP = 0.05f
}

/**
 * The video player's sound: pitch (apart from the tempo, through Media3's Sonic with the speed)
 * and an equalizer ([SoundProcessor] in the player's sink, passing sound untouched while flat).
 * A title keeps its own (`videoSound`); the equalizer otherwise follows the music player's.
 */
@UnstableApi
internal class PlayerVideoSound(private val host: VideoPlayerActivity) {
    /** In the player's audio sink, after smart audio. */
    val processor = SoundProcessor(bypassWhenFlat = true)
    private var own: VideoSound? = null
    private var musicEq: JSONObject? = null

    val sound: VideoSound get() = own ?: VideoSound()

    /** What the pitch multiplies the speed's own by. */
    val pitch: Float get() = pitchRatio(sound.pitch)

    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    /** A video loads: its title's sound, the music player's equalizer. */
    fun onLoad(request: JSONObject) {
        request.optJSONObject("musicEq")?.let { musicEq = it }
        own = VideoSound.from(request.optJSONObject("titleState")?.optJSONObject("videoSound"))
        processor.setConfig(VideoSoundMath.equalizer(sound, musicEq))
    }

    /** Settings changed while a video plays: the music player's equalizer, at once. */
    fun onPrefs(update: JSONObject) {
        update.optJSONObject("musicEq")?.let { musicEq = it } ?: return
        processor.setConfig(VideoSoundMath.equalizer(sound, musicEq))
    }

    private fun set(next: VideoSound, keep: Boolean = true) {
        val pitchChanged = next.pitch != sound.pitch
        own = next
        processor.setConfig(VideoSoundMath.equalizer(next, musicEq))
        if (pitchChanged) host.applySpeed(host.player.playbackParameters.speed)
        if (keep) host.remember(JSONObject().put("videoSound", next.toJson()))
    }

    private fun reset() {
        own = null
        processor.setConfig(VideoSoundMath.equalizer(sound, musicEq))
        host.remember(JSONObject().put("videoSound", JSONObject.NULL))
        host.setSpeed(host.prefs.defaultSpeed)
    }

    /** The Effects sheet's Sound: tempo, pitch, the equalizer and Reset. */
    fun rows(sheet: PlayerSheet, redraw: () -> Unit) {
        sheet.header(label("effectsSound", "Sound"))
        val speed = host.player.playbackParameters.speed
        sheet.custom(
            sheetSlider(
                host,
                name = label("soundTempo", "Tempo"),
                steps = ((VideoSoundMath.TEMPO_MAX - VideoSoundMath.TEMPO_MIN) / VideoSoundMath.TEMPO_STEP).roundToInt(),
                position = ((speed.coerceIn(VideoSoundMath.TEMPO_MIN, VideoSoundMath.TEMPO_MAX) - VideoSoundMath.TEMPO_MIN) /
                    VideoSoundMath.TEMPO_STEP).roundToInt(),
                text = { step -> formatSpeed(VideoSoundMath.TEMPO_MIN + step * VideoSoundMath.TEMPO_STEP) },
                moved = { step -> host.applySpeed(VideoSoundMath.TEMPO_MIN + step * VideoSoundMath.TEMPO_STEP) },
                released = { step -> host.setSpeed(VideoSoundMath.TEMPO_MIN + step * VideoSoundMath.TEMPO_STEP) },
            ),
        )
        // Half semitones from −12 to +12.
        sheet.custom(
            sheetSlider(
                host,
                name = label("soundPitch", "Pitch"),
                steps = 48,
                position = (sound.pitch + 120) / 5,
                text = { step -> VideoSoundMath.pitchText(step * 5 - 120) },
                moved = { step -> set(sound.copy(pitch = step * 5 - 120), keep = false) },
                released = { step -> set(sound.copy(pitch = step * 5 - 120)) },
            ),
        )
        val current = sound
        sheet.row(
            label("soundEqualizerOwn", "Own equalizer for this title"),
            if (current.ownEq) null else label("soundEqualizerFollows", "Follows the music player's"),
            current.ownEq,
        ) {
            set(sound.copy(ownEq = !sound.ownEq, eqEnabled = !sound.ownEq || sound.eqEnabled))
            redraw()
        }
        if (current.ownEq) {
            val presets = VideoSoundMath.PRESETS
            sheet.chips(
                presets.map { (id, fallback, _) -> label("eqPreset" + id.replaceFirstChar { it.uppercase() }, fallback) },
                presets.indexOfFirst { it.third == current.eqBands && current.eqEnabled },
            ) { picked ->
                val (id, _, bands) = presets[picked]
                set(sound.copy(eqEnabled = id != "flat", eqPreset = id, eqBands = bands))
                redraw()
            }
        }
        sheet.custom(LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(host.dp(12), host.dp(4), host.dp(20), host.dp(4))
            addView(host.pill(label("soundReset", "Reset sound")) {
                reset()
                redraw()
            })
        })
    }
}

/** A named slider with its value beside the name, live while moved, kept when let go. */
internal fun sheetSlider(
    host: VideoPlayerActivity,
    name: String,
    steps: Int,
    position: Int,
    text: (Int) -> String,
    moved: (Int) -> Unit,
    released: (Int) -> Unit,
): LinearLayout {
    val title = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
        setPadding(host.dp(20), host.dp(6), host.dp(20), 0)
        this.text = "$name  ${text(position)}"
    }
    val bar = SeekBar(host).apply {
        max = steps
        progress = position.coerceIn(0, steps)
        progressTintList = ColorStateList.valueOf(playerAccent)
        thumbTintList = ColorStateList.valueOf(playerAccent)
        setPadding(host.dp(28), host.dp(8), host.dp(28), host.dp(8))
        contentDescription = name
        setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                if (!fromUser) return
                title.text = "$name  ${text(progress)}"
                moved(progress)
            }

            override fun onStartTrackingTouch(bar: SeekBar) {}

            override fun onStopTrackingTouch(bar: SeekBar) = released(bar.progress)
        })
    }
    return LinearLayout(host).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_VERTICAL
        addView(title)
        addView(bar)
    }
}

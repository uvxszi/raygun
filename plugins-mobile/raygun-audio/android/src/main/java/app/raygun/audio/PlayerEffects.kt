package app.raygun.audio

import android.content.res.ColorStateList
import android.graphics.Color
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.VideoFrameProcessor
import androidx.media3.common.util.UnstableApi
import org.json.JSONObject
import java.util.Locale

/** How often frames are checked while heavy effects play. */
private const val WATCH_MS = 1_000L

/** A change while paused: the frame is drawn again with it after this long (changes in between join it). */
private const val PREVIEW_MS = 40L

/** Without Media3's replay cache, the paused frame comes again through a seek, after this long. */
private const val SEEK_PREVIEW_MS = 150L

/** One adjustment of the Effects sheet: its field, its words and its range. */
private class Adjustment(val key: String, val label: String, val fallback: String, val min: Int, val max: Int)

private val ADJUSTMENTS = listOf(
    Adjustment("brightness", "effectBrightness", "Brightness", -100, 100),
    Adjustment("contrast", "effectContrast", "Contrast", -100, 100),
    Adjustment("saturation", "effectSaturation", "Saturation", -100, 100),
    Adjustment("gamma", "effectGamma", "Gamma", -100, 100),
    Adjustment("hue", "effectHue", "Hue", -180, 180),
    Adjustment("sharpen", "effectSharpen", "Sharpen", 0, 100),
    Adjustment("deflicker", "effectDeflicker", "Deflicker", 0, 100),
    Adjustment("denoise", "effectDenoise", "Denoise", 0, 100),
)

private fun PictureEffects.value(key: String): Int = when (key) {
    "brightness" -> brightness
    "contrast" -> contrast
    "saturation" -> saturation
    "gamma" -> gamma
    "hue" -> hue
    "sharpen" -> sharpen
    "deflicker" -> deflicker
    "denoise" -> denoise
    else -> 0
}

private fun PictureEffects.with(key: String, value: Int): PictureEffects = when (key) {
    "brightness" -> copy(brightness = value)
    "contrast" -> copy(contrast = value)
    "saturation" -> copy(saturation = value)
    "gamma" -> copy(gamma = value)
    "hue" -> copy(hue = value)
    "sharpen" -> copy(sharpen = value)
    "deflicker" -> copy(deflicker = value)
    "denoise" -> copy(denoise = value)
    else -> this
}

/** Smart audio's switches: field, words, the line under them. */
private class SmartOption(
    val label: String,
    val fallback: String,
    val hint: String,
    val hintFallback: String,
    val get: (SmartAudioPrefs) -> Boolean,
    val toggled: (SmartAudioPrefs) -> SmartAudioPrefs,
)

private val SMART_OPTIONS = listOf(
    SmartOption("smartDialogue", "Clearer dialogue", "smartDialogueHint", "Voices brought forward", { it.dialogue }) {
        it.copy(dialogue = !it.dialogue)
    },
    SmartOption("smartNight", "Night mode", "smartNightHint", "Loud parts quieter, quiet parts louder", { it.night }) {
        it.copy(night = !it.night)
    },
    SmartOption("smartNormalize", "Even loudness", "smartNormalizeHint", "Every video about as loud", { it.normalize }) {
        it.copy(normalize = !it.normalize)
    },
    SmartOption("smartLeveling", "Level scenes", "smartLevelingHint", "Quiet and loud scenes evened out", { it.leveling }) {
        it.copy(leveling = !it.leveling)
    },
)

/**
 * The video player's effects: the picture's ([PictureEffectsGl]) and smart audio
 * ([SmartAudioProcessor]). What applies is the title's own (`pictureEffects` and `smartAudio` in
 * its memory), else Settings → Video player → Effects. Changes made here are kept for the title;
 * Make default saves them as the settings' instead.
 *
 * The picture goes through Media3's effect pipeline only once an effect is on, since it costs a
 * copy of every frame on the GPU: `setVideoEffects` must come before the player prepares, so the
 * first effect turned on during a video prepares it again where it is. From then on values change
 * live (the passes read them each frame). While frames drop with heavy effects on (denoise, then
 * deflicker, then sharpen), the heaviest is turned off for this player, with a note; the title
 * keeps it. A pipeline that fails turns the picture's effects off for the rest of the run.
 *
 * Paused, a change draws the frame shown again at once: Media3's replay cache ([VideoGraphChoice])
 * sends the last frames through the passes again with the new values. Smooth motion
 * ([SmoothMotionGl]) comes after the picture's passes and goes first when frames drop, or by
 * itself when it can't keep up.
 */
@UnstableApi
internal class PlayerEffects(private val host: VideoPlayerActivity) {
    /** In the player's audio sink (`PlayerRenderers`), from the start. */
    val audio = SmartAudioProcessor()
    private val params = EffectParams()
    private val status = EffectStatus()
    private val effect = PictureEffectsGl(params, status)
    private val motionStatus = MotionStatus()
    private val motion = SmoothMotionGl(params, motionStatus)

    /** How the video renderer builds its effect graph ([PlayerRenderers]). */
    val graph = VideoGraphChoice()

    private var defaults = PictureEffects.NONE
    private var defaultAudio = SmartAudioPrefs()
    private var own: PictureEffects? = null
    private var ownAudio: SmartAudioPrefs? = null

    /** Media3's pipeline is in this player (it stays until the player closes). */
    private var pipeline = false
    /** The next prepare needs the pipeline first. */
    private var startOnPrepare = false
    /** Heavy effects turned off for this player because frames dropped. */
    private val dropped = linkedSetOf<String>()
    /** The adjustment the sheet's slider changes. */
    private var editing = ADJUSTMENTS.first().key

    val picture: PictureEffects get() = own ?: defaults
    val sound: SmartAudioPrefs get() = ownAudio ?: defaultAudio

    /** What plays: the picture's effects without the ones dropped for speed. */
    private val applied: PictureEffects get() = dropped.fold(picture) { effects, kind -> effects.without(kind) }

    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    // ---- Following what plays ----

    /** A video loads (`load` in the player): its title's effects, else the settings'. */
    fun onLoad(request: JSONObject) {
        request.optJSONObject("prefs")?.let(::readDefaults)
        val memory = request.optJSONObject("titleState")
        own = PictureEffects.from(memory?.optJSONObject("pictureEffects"))
        ownAudio = SmartAudioPrefs.from(memory?.optJSONObject("smartAudio"))
        audio.setPrefs(sound)
        audio.newProgram()
        params.shader = EffectMath.shader(applied)
        followMotion()
        status.reset()
        startOnPrepare = wantsPipeline()
    }

    /** Right before the player prepares a video: the pipeline goes in first when it's wanted. */
    fun beforePrepare() {
        if (!startOnPrepare) return
        startOnPrepare = false
        startPipeline(prepare = false)
        watchFrames()
    }

    /** Settings changed in the app while a video plays: new defaults apply at once. */
    fun onPrefs(update: JSONObject) {
        update.optJSONObject("prefs")?.let(::readDefaults) ?: return
        audio.setPrefs(sound)
        apply()
    }

    private fun readDefaults(prefs: JSONObject) {
        defaults = PictureEffects.from(prefs.optJSONObject("pictureEffects")) ?: PictureEffects.NONE
        defaultAudio = SmartAudioPrefs.from(prefs.optJSONObject("smartAudio")) ?: SmartAudioPrefs()
    }

    private fun wantsPipeline(): Boolean = !applied.neutral && !pipeline && !failedThisRun

    /**
     * Puts Media3's pipeline in. Before a prepare that's all; during a video the player stops and
     * prepares again where it was (stopping resets the renderers, which only build the pipeline
     * when enabled afresh).
     */
    private fun startPipeline(prepare: Boolean) {
        val player = host.player
        graph.wantReplay = !applied.smoothMotion
        player.setVideoEffects(listOf(effect, motion))
        pipeline = true
        status.reset()
        if (player.playbackState == Player.STATE_IDLE) return
        player.stop()
        if (prepare) {
            player.prepare()
            host.showHint(label("effectsStarting", "Starting effects…"))
        }
    }

    /** The values in effect now, live; paused, the frame is drawn again with them. */
    private fun apply() {
        val effects = applied
        params.shader = EffectMath.shader(effects)
        followMotion()
        if (!effects.neutral && !pipeline && !failedThisRun && host.player.currentMediaItem != null) {
            startPipeline(prepare = true)
        }
        if (pipeline && !host.player.isPlaying) {
            host.main.removeCallbacks(preview)
            host.main.postDelayed(preview, if (graph.replayable) PREVIEW_MS else SEEK_PREVIEW_MS)
        }
        watchFrames()
    }

    /**
     * Paused: the frame shown goes through the passes again with the new values. Media3 replays
     * its last frames when its graph keeps them, else the same place is sought again.
     */
    private val preview = Runnable {
        val player = host.player
        if (player.isPlaying || player.playbackState == Player.STATE_IDLE || !pipeline || failedThisRun) return@Runnable
        if (graph.replayable) {
            player.setVideoEffects(VideoFrameProcessor.REDRAW)
        } else {
            player.seekTo(player.currentPosition)
        }
    }

    private var listening = false

    /** Smooth motion's switch and what it follows: the screen's refresh rate and the speed. */
    private fun followMotion() {
        val player = host.player
        val hz = host.window?.decorView?.display?.refreshRate ?: 60f
        params.motion = MotionParams(on = applied.smoothMotion, displayHz = hz, speed = player.playbackParameters.speed)
        if (!listening) {
            listening = true
            player.addListener(object : Player.Listener {
                override fun onPlaybackParametersChanged(playbackParameters: PlaybackParameters) = followMotion()
            })
        }
    }

    /** A failed pipeline: the video goes on without picture effects. True when it was the effects'. */
    fun onPlayerError(error: PlaybackException): Boolean {
        if (!pipeline || failedThisRun) return false
        val code = error.errorCode
        if (code != PlaybackException.ERROR_CODE_VIDEO_FRAME_PROCESSOR_INIT_FAILED &&
            code != PlaybackException.ERROR_CODE_VIDEO_FRAME_PROCESSING_FAILED
        ) {
            return false
        }
        Log.w(EFFECTS_TAG, "the effects pipeline failed; picture effects off for this run", error)
        failedThisRun = true
        params.shader = EffectMath.shader(PictureEffects.NONE)
        // Media3 keeps the pipeline in the renderer; with nothing in it, the video goes on.
        host.player.setVideoEffects(emptyList())
        host.player.prepare()
        host.showHint(label("effectsFailed", "Picture effects can't play here, so they're off"), 4_000)
        return true
    }

    // ---- Keeping playback smooth ----

    private var watching = false
    private var lastDropped = -1
    private var lastRendered = -1
    private val window = ArrayDeque<Pair<Int, Int>>()

    private val watch = object : Runnable {
        override fun run() {
            if (!watching) return
            runCatching { checkFrames() }.onFailure { error -> Log.w(EFFECTS_TAG, "frame check failed", error) }
            host.main.postDelayed(this, WATCH_MS)
        }
    }

    private fun watchFrames() {
        val wanted = pipeline && applied.heavy.isNotEmpty()
        if (wanted == watching) return
        watching = wanted
        host.main.removeCallbacks(watch)
        window.clear()
        lastDropped = -1
        if (wanted) host.main.postDelayed(watch, WATCH_MS)
    }

    /**
     * Frames dropped over the last seconds (Media3's counters), or passes slower than the frames
     * come: the heaviest effect goes, with a note.
     */
    private fun checkFrames() {
        val player = host.player
        // Smooth motion found it can't keep up (or its passes failed here): it goes, with a note.
        if (motionStatus.gaveUp && applied.smoothMotion) {
            dropped += PictureEffects.SMOOTH_MOTION
            followMotion()
            watchFrames()
            host.showHint(label("effectsSmoothSlow", "Smooth motion turned off: this phone can't keep up"), 3_000)
            return
        }
        val counters = player.videoDecoderCounters
        if (!player.isPlaying || counters == null) {
            window.clear()
            lastDropped = -1
            return
        }
        counters.ensureUpdated()
        val droppedNow = counters.droppedBufferCount
        val renderedNow = counters.renderedOutputBufferCount
        if (lastDropped in 0..droppedNow && lastRendered in 0..renderedNow) {
            window.addLast((droppedNow - lastDropped) to (renderedNow - lastRendered))
            if (window.size > 4) window.removeFirst()
        }
        lastDropped = droppedNow
        lastRendered = renderedNow
        val drops = window.sumOf { it.first }
        val shown = window.sumOf { it.second }
        val frameMs = 1000.0 / (player.videoFormat?.frameRate?.takeIf { it > 0f } ?: 30f)
        val slow = status.samples >= 3 && status.averageMs > frameMs * 0.7
        val dropping = window.size >= 3 && drops >= 8 && drops > (drops + shown) * 0.12
        if ((dropping || slow) && player.playbackParameters.speed <= 1.5f) {
            val heaviest = applied.heavy.firstOrNull() ?: return
            dropped += heaviest
            Log.i(
                EFFECTS_TAG,
                String.format(Locale.ROOT, "%s off: %d of %d frames dropped, %.2f ms a frame", heaviest, drops, drops + shown, status.averageMs),
            )
            params.shader = EffectMath.shader(applied)
            followMotion()
            status.reset()
            window.clear()
            watchFrames()
            host.showHint(label("effectsDropped", "{n} turned off: frames were dropping", adjustmentName(heaviest)), 3_000)
        }
    }

    // ---- Changes ----

    /** The title's own effects from now on. */
    private fun setPicture(next: PictureEffects, keep: Boolean = true) {
        own = next
        if (keep) host.remember(JSONObject().put("pictureEffects", next.toJson()))
        apply()
    }

    private fun setSound(next: SmartAudioPrefs) {
        ownAudio = next
        audio.setPrefs(sound)
        host.remember(JSONObject().put("smartAudio", next.toJson()))
    }

    /** These become Settings → Video player → Effects; the title follows them from now on. */
    private fun makePictureDefault() {
        defaults = picture
        own = null
        host.savePrefs(JSONObject().put("pictureEffects", defaults.toJson()))
        host.remember(JSONObject().put("pictureEffects", JSONObject.NULL))
        host.showHint(label("effectsMadeDefault", "Saved as the default"))
    }

    private fun makeSoundDefault() {
        defaultAudio = sound
        ownAudio = null
        host.savePrefs(JSONObject().put("smartAudio", defaultAudio.toJson()))
        host.remember(JSONObject().put("smartAudio", JSONObject.NULL))
        host.showHint(label("effectsMadeDefault", "Saved as the default"))
    }

    /** No effects for this title (the defaults again, when they're none). */
    private fun resetPicture() {
        if (defaults.neutral) {
            own = null
            host.remember(JSONObject().put("pictureEffects", JSONObject.NULL))
            apply()
        } else {
            setPicture(PictureEffects.NONE)
        }
    }

    private fun useDefaultPicture() {
        own = null
        host.remember(JSONObject().put("pictureEffects", JSONObject.NULL))
        apply()
    }

    // ---- The sheets ----

    /** What More → Effects says beside its name. */
    fun summary(): String = if (picture.neutral) label("off", "Off") else label("effectsOn", "On")

    private fun adjustmentName(key: String): String =
        if (key == PictureEffects.SMOOTH_MOTION) {
            label("effectSmoothMotion", "Smooth motion")
        } else {
            ADJUSTMENTS.firstOrNull { it.key == key }?.let { label(it.label, it.fallback) } ?: key
        }

    private fun lookName(look: String): String = when (look) {
        "warm" -> label("effectsLookWarm", "Warm")
        "cool" -> label("effectsLookCool", "Cool")
        "vivid" -> label("effectsLookVivid", "Vivid")
        "grayscale" -> label("effectsLookGrayscale", "Black and white")
        "sepia" -> label("effectsLookSepia", "Sepia")
        else -> label("effectsLookNone", "None")
    }

    private fun shown(adjustment: Adjustment, value: Int): String {
        val signed = when {
            adjustment.min == 0 -> value.toString()
            value > 0 -> "+$value"
            value < 0 -> "−${-value}"
            else -> "0"
        }
        return if (adjustment.key == "hue") "$signed°" else signed
    }

    /** What the sheet says under its controls: why something is off, else what the effects cost. */
    private fun note(): String? = when {
        failedThisRun -> label("effectsFailed", "Picture effects can't play here, so they're off")
        status.hdr -> label("effectsHdr", "HDR videos play without picture effects")
        picture.smoothMotion && motionStatus.gaveUp ->
            label("effectsSmoothSlow", "Smooth motion turned off: this phone can't keep up")
        dropped.isNotEmpty() ->
            label("effectsDropped", "{n} turned off: frames were dropping", dropped.joinToString(", ") { adjustmentName(it) })
        status.limited -> label("effectsLimited", "Only the colours apply on this phone")
        status.samples > 0 && pipeline ->
            label("effectsCost", "About {n} ms a frame", String.format(Locale.ROOT, "%.1f", status.averageMs))
        else -> null
    }

    /** More → Effects: presets, looks, then one slider at a time over a clear view of the picture. */
    fun open() {
        val sheet = PlayerSheet(host, label("effects", "Effects"))
        // The picture shows undimmed while it changes.
        sheet.root.setBackgroundColor(Color.TRANSPARENT)
        val effects = picture
        val presets = EffectMath.PRESETS
        sheet.header(label("effectsPresets", "Presets"))
        sheet.chips(
            presets.map { (key, _) -> label(key, presetFallback(key)) },
            presets.indexOfFirst { it.second == effects.adjustments },
        ) { picked ->
            // Picked again: what dropped for speed gets another try.
            dropped.clear()
            motionStatus.again()
            setPicture(presets[picked].second.copy(smoothMotion = picture.smoothMotion))
            open()
        }
        sheet.header(label("effectsLook", "Look"))
        sheet.chips(PictureEffects.LOOKS.map(::lookName), PictureEffects.LOOKS.indexOf(effects.look)) { picked ->
            setPicture(picture.copy(look = PictureEffects.LOOKS[picked]))
            open()
        }
        sheet.header(label("effectsAdjust", "Adjust"))
        adjuster(sheet)
        sheet.header(label("effectsMotion", "Motion"))
        sheet.row(
            label("effectSmoothMotion", "Smooth motion"),
            label("effectSmoothMotionHint", "In-between frames for smoother movement"),
            effects.smoothMotion,
        ) {
            val on = !picture.smoothMotion
            if (on) {
                // Asked for again: it gets another try.
                dropped.remove(PictureEffects.SMOOTH_MOTION)
                motionStatus.again()
            }
            setPicture(picture.copy(smoothMotion = on))
            open()
        }
        sheet.custom(LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(host.dp(12), host.dp(8), host.dp(20), host.dp(4))
            addView(host.pill(label("effectsResetAll", "Reset all")) {
                resetPicture()
                open()
            })
            addView(host.pill(label("effectsMakeDefault", "Make default")) {
                makePictureDefault()
                open()
            })
            if (own != null && own != defaults) {
                addView(host.pill(label("effectsUseDefault", "Use the default")) {
                    useDefaultPicture()
                    open()
                })
            }
        })
        note()?.let { sheet.row(it, onClick = null) }
        soundRows(sheet, label("effectsSound", "Sound")) { open() }
        sheet.show()
    }

    private fun presetFallback(key: String) = when (key) {
        "presetOldFilm" -> "Old film"
        "presetCleanUp" -> "Clean up"
        "presetBrighter" -> "Brighter"
        "presetCinema" -> "Cinema"
        else -> "Off"
    }

    /** The adjustments as pills (their values beside changed ones) and the picked one's slider. */
    private fun adjuster(sheet: PlayerSheet) {
        val pills = mutableListOf<TextView>()
        val value = TextView(host).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(host.dp(20), host.dp(4), host.dp(20), 0)
        }
        val slider = SeekBar(host).apply {
            progressTintList = ColorStateList.valueOf(playerAccent)
            thumbTintList = ColorStateList.valueOf(playerAccent)
            setPadding(host.dp(28), host.dp(10), host.dp(28), host.dp(10))
        }
        val reset = host.pill(label("effectsReset", "Reset")) {}
        fun pillText(adjustment: Adjustment): String {
            val name = label(adjustment.label, adjustment.fallback)
            val current = picture.value(adjustment.key)
            return if (current == 0) name else "$name ${shown(adjustment, current)}"
        }
        fun show(adjustment: Adjustment) {
            editing = adjustment.key
            val current = picture.value(adjustment.key)
            value.text = "${label(adjustment.label, adjustment.fallback)}  ${shown(adjustment, current)}"
            slider.max = adjustment.max - adjustment.min
            slider.progress = current - adjustment.min
            reset.alpha = if (current == 0) 0.4f else 1f
            ADJUSTMENTS.forEachIndexed { index, each -> restyle(pills[index], each.key == editing) }
        }
        val line = LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(host.dp(12), host.dp(6), host.dp(20), host.dp(6))
        }
        ADJUSTMENTS.forEach { adjustment ->
            val pill = host.pill(pillText(adjustment)) { show(adjustment) }
            pills += pill
            line.addView(pill)
        }
        sheet.custom(HorizontalScrollView(host).apply {
            isHorizontalScrollBarEnabled = false
            addView(line)
        })
        val row = LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(value, LinearLayout.LayoutParams(0, -2, 1f))
            addView(reset)
            setPadding(0, 0, host.dp(16), 0)
        }
        sheet.custom(row)
        sheet.custom(slider)
        slider.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                if (!fromUser) return
                val adjustment = ADJUSTMENTS.first { it.key == editing }
                val next = progress + adjustment.min
                // Asked for again: one that dropped for speed gets another try.
                if (next != 0) dropped.remove(adjustment.key)
                // Live while dragging; kept for the title when let go.
                setPicture(picture.with(adjustment.key, next), keep = false)
                value.text = "${label(adjustment.label, adjustment.fallback)}  ${shown(adjustment, next)}"
                pills[ADJUSTMENTS.indexOf(adjustment)].text = pillText(adjustment)
                reset.alpha = if (next == 0) 0.4f else 1f
            }

            override fun onStartTrackingTouch(bar: SeekBar) {}

            override fun onStopTrackingTouch(bar: SeekBar) {
                host.remember(JSONObject().put("pictureEffects", picture.toJson()))
            }
        })
        reset.setOnClickListener {
            val adjustment = ADJUSTMENTS.first { it.key == editing }
            setPicture(picture.with(adjustment.key, 0))
            pills[ADJUSTMENTS.indexOf(adjustment)].text = pillText(adjustment)
            show(adjustment)
        }
        show(ADJUSTMENTS.firstOrNull { it.key == editing } ?: ADJUSTMENTS.first())
    }

    private fun restyle(pill: TextView, picked: Boolean) {
        pill.setTextColor(if (picked) Color.BLACK else Color.WHITE)
        pill.background = rounded(if (picked) Color.WHITE else 0x66000000, host.dp(20).toFloat())
    }

    /** The Audio sheet's smart audio: four switches, and Make default once they differ. */
    fun audioRows(sheet: PlayerSheet, redraw: () -> Unit) = soundRows(sheet, label("smartAudio", "Smart audio"), redraw)

    /** Smart audio's switches under `title`: the Audio sheet's, and the Effects sheet's Sound. */
    private fun soundRows(sheet: PlayerSheet, title: String, redraw: () -> Unit) {
        sheet.header(title)
        val current = sound
        SMART_OPTIONS.forEach { option ->
            sheet.row(label(option.label, option.fallback), label(option.hint, option.hintFallback), option.get(current)) {
                setSound(option.toggled(sound))
                redraw()
            }
        }
        if (ownAudio != null && ownAudio != defaultAudio) {
            sheet.row(label("effectsMakeDefault", "Make default")) {
                makeSoundDefault()
                redraw()
            }
        }
    }

    private companion object {
        /** The pipeline failed in this run: later players leave picture effects off. */
        @Volatile
        var failedThisRun = false
    }
}

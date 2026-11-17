package app.raygun.audio

import android.net.Uri
import androidx.media3.common.C
import androidx.media3.common.text.Cue
import kotlin.concurrent.thread

/**
 * The time subtitles are read at: the playhead moved back by the delay (positive shows lines
 * later, negative earlier; any length, past the video's start or end).
 */
internal fun subtitleTimeMs(positionMs: Long, delayMs: Long): Long = positionMs - delayMs

/** The cues showing at `atUs` (start included, end not). */
internal fun timedAt(timed: List<TimedCues>, atUs: Long): List<TimedCues> =
    timed.filter { atUs >= it.startUs && atUs < it.endUs }

/** The next start or end after `atUs`; null when nothing changes anymore. */
internal fun nextEdgeUs(timed: List<TimedCues>, atUs: Long): Long? {
    var next: Long? = null
    for (entry in timed) {
        for (edge in longArrayOf(entry.startUs, entry.endUs)) {
            if (edge > atUs && edge != Long.MAX_VALUE && (next == null || edge < next)) next = edge
        }
    }
    return next
}

/** The shortest and longest wait between two looks at the playhead while it plays. */
internal const val CLOCK_MIN_WAIT_MS = 10L
internal const val CLOCK_MAX_WAIT_MS = 250L

/**
 * How long until the lines change: the media time to the next edge at the playing `speed`, kept
 * between [CLOCK_MIN_WAIT_MS] and [CLOCK_MAX_WAIT_MS] (so a seek or a new delay is seen soon even
 * when no line is near).
 */
internal fun waitMs(nextEdgeUs: Long?, atUs: Long, speed: Float): Long {
    if (nextEdgeUs == null) return CLOCK_MAX_WAIT_MS
    val media = (nextEdgeUs - atUs) / 1000.0
    val wall = media / speed.coerceAtLeast(0.1f)
    return wall.toLong().coerceIn(CLOCK_MIN_WAIT_MS, CLOCK_MAX_WAIT_MS)
}

/**
 * The main subtitles drawn from their file here when the chosen track is a subtitle file
 * (a sidecar): the file is read once ([SubtitleFiles]) and its lines are shown at the playhead
 * less the delay ([subtitleTimeMs]), looked at again when the next line starts or ends at the
 * playing speed. So the delay applies to what's on screen at once, works either way and longer
 * than the video, and follows seeks and speed changes exactly; Media3's own drawing of the same
 * track is set aside meanwhile. The video's own tracks stay with Media3's renderer
 * ([OffsetTextRenderer]).
 */
internal class SubtitleClock(private val host: VideoPlayerActivity) {
    private val parsed = mutableMapOf<Uri, List<TimedCues>>()
    private var reading: Uri? = null
    /** The chosen file, while its lines are drawn here. */
    private var active: Sidecar? = null
    private var shown: List<TimedCues>? = null

    /** Lines of the chosen file are drawn here: Media3's cues are set aside. */
    val drawing: Boolean get() = active?.let { parsed.containsKey(it.uri) } == true

    private val ticker = object : Runnable {
        override fun run() = tick()
    }

    /** A new video: nothing is drawn until a file is chosen again. */
    fun reset() {
        host.main.removeCallbacks(ticker)
        active = null
        shown = null
        reading = null
        parsed.clear()
    }

    /** The tracks changed: the chosen one, when it's a file, is read and drawn here. */
    fun onTracks() {
        val chosen = if (host.tracks.textOff() || host.liveTracks.primary != null) {
            null
        } else {
            host.tracks.list(C.TRACK_TYPE_TEXT).firstOrNull { host.tracks.chosen(it) }
                ?.let { host.sidecarOf(it.format) }
        }
        if (chosen == active) return
        active = chosen
        shown = null
        host.main.removeCallbacks(ticker)
        val sidecar = chosen ?: return
        if (parsed.containsKey(sidecar.uri)) {
            refresh()
            return
        }
        if (reading == sidecar.uri) return
        reading = sidecar.uri
        thread(name = "raygun-subtitle-clock") {
            val timed = runCatching { SubtitleFiles.parse(SubtitleFiles.read(host, sidecar.uri), sidecar.mime) }
                .onFailure { android.util.Log.w("RaygunVideo", "subtitle file not read for the clock", it) }
                .getOrNull()
            host.runOnUiThread {
                if (reading == sidecar.uri) reading = null
                // Unreadable here: Media3 keeps drawing it.
                if (timed == null || timed.isEmpty()) return@runOnUiThread
                parsed[sidecar.uri] = timed
                if (active == sidecar) refresh()
            }
        }
    }

    /** Draws now (a seek, a new delay or speed, a pause) and goes on from here. */
    fun refresh() {
        host.main.removeCallbacks(ticker)
        shown = null
        tick()
    }

    private fun tick() {
        val sidecar = active ?: return
        val timed = parsed[sidecar.uri] ?: return
        val atUs = subtitleTimeMs(host.player.currentPosition, host.state.subtitleDelayMs) * 1000
        val now = timedAt(timed, atUs)
        if (now != shown) {
            shown = now
            host.mainCues.set(now.flatMap { it.cues })
        }
        if (host.player.isPlaying) {
            val speed = host.player.playbackParameters.speed
            host.main.postDelayed(ticker, waitMs(nextEdgeUs(timed, atUs), atUs, speed))
        }
    }

    /** Media3's cues for the main subtitles: shown unless the file is drawn here. */
    fun fromMedia3(cues: List<Cue>) {
        if (!drawing) host.mainCues.set(cues)
    }
}

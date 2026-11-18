package app.raygun.audio

import android.net.Uri
import android.text.SpannableStringBuilder
import androidx.media3.common.C
import androidx.media3.common.text.Cue
import kotlin.concurrent.thread
import kotlin.math.ceil

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
internal const val CLOCK_MIN_WAIT_MS = 1L
internal const val CLOCK_MAX_WAIT_MS = 250L

/**
 * How long until the lines change: the media time to the next edge at the playing `speed`,
 * rounded up (a look a moment early would see the old lines and wait again), kept between
 * [CLOCK_MIN_WAIT_MS] and [CLOCK_MAX_WAIT_MS] (so a seek or a new delay is seen soon even when no
 * line is near).
 */
internal fun waitMs(nextEdgeUs: Long?, atUs: Long, speed: Float): Long {
    if (nextEdgeUs == null) return CLOCK_MAX_WAIT_MS
    val media = (nextEdgeUs - atUs) / 1000.0
    val wall = media / speed.coerceAtLeast(0.1f)
    return ceil(wall).toLong().coerceIn(CLOCK_MIN_WAIT_MS, CLOCK_MAX_WAIT_MS)
}

/**
 * One look of the clock: what shows at the player's `positionMs` less `delayMs`, and when to look
 * again (null while not playing: a pause, a stall, the end; the player's own events look again).
 * Nothing is carried from one look to the next but what's shown, so a stall, a seek, a speed
 * change or a new delay can't put the lines out of step: each look reads the playhead afresh.
 */
internal class ClockCore {
    var shown: List<TimedCues>? = null
        private set

    /** Drawn anew at the next look (a seek, a new list). */
    fun forget() {
        shown = null
    }

    /** The lines when they changed (else null), and the wait until the next look. */
    fun look(timed: List<TimedCues>, positionMs: Long, delayMs: Long, playing: Boolean, speed: Float): Pair<List<TimedCues>?, Long?> {
        val atUs = subtitleTimeMs(positionMs, delayMs) * 1000
        val now = timedAt(timed, atUs)
        val changed = if (now != shown) now.also { shown = it } else null
        val wait = if (playing) waitMs(nextEdgeUs(timed, atUs), atUs, speed) else null
        return changed to wait
    }
}

// ---- Join lines close together (Smart Subtitles) ----

/** Cues this close join when one of them is short (`apps/raygun/src/video/joinLines.ts`). */
internal const val JOIN_GAP_US = 250_000L
/** A cue shorter than this is a quick change. */
internal const val SHORT_CUE_US = 1_200_000L
/** A joined cue lasts this long at most. */
internal const val MAX_JOINED_US = 6_000_000L
/** A joined cue holds this many lines at most. */
internal const val MAX_JOINED_LINES = 4

/**
 * Entries that overlap, or follow each other within [JOIN_GAP_US] while one is short (lines
 * changing quickly), joined into one ([make]: from the first start to the last end, their lines
 * stacked, each once), within [MAX_JOINED_US] and [MAX_JOINED_LINES]; entries without lines
 * (pictures) stay as they are. Sorted by start. The same rules as the desktop's `joinCloseCues`.
 */
internal fun <T> joinClose(
    entries: List<T>,
    start: (T) -> Long,
    end: (T) -> Long,
    lines: (T) -> List<String>,
    make: (Long, Long, List<T>, List<String>) -> T,
): List<T> {
    val sorted = entries.sortedWith(compareBy(start, end))
    val out = ArrayList<T>(sorted.size)
    var run = ArrayList<T>()
    var runStart = 0L
    var runEnd = 0L
    var runLines = emptyList<String>()
    fun unique(list: List<String>): List<String> {
        val seen = HashSet<String>()
        return list.filter { line -> line.isNotBlank() && seen.add(line.trim()) }
    }
    fun flush() {
        when {
            run.size == 1 -> out += run[0]
            run.size > 1 -> out += make(runStart, runEnd, run, runLines)
        }
        run = ArrayList()
    }
    for (entry in sorted) {
        val own = lines(entry)
        val last = run.lastOrNull()
        if (last != null && own.isNotEmpty() && lines(last).isNotEmpty()) {
            val joinedEnd = maxOf(runEnd, end(entry))
            val joinedLines = unique(runLines + own)
            val short = { item: T -> end(item) - start(item) < SHORT_CUE_US }
            val joins = start(entry) < runEnd ||
                (start(entry) - runEnd < JOIN_GAP_US && (short(last) || short(entry)))
            if (joins && joinedEnd - runStart <= MAX_JOINED_US && joinedLines.size <= MAX_JOINED_LINES) {
                run += entry
                runEnd = joinedEnd
                runLines = joinedLines
                continue
            }
        }
        flush()
        run += entry
        runStart = start(entry)
        runEnd = end(entry)
        runLines = unique(own)
    }
    flush()
    return out
}

/** A file's cues with those close together joined ([joinClose]). */
internal fun joinedCues(timed: List<TimedCues>): List<TimedCues> = joinClose(
    timed,
    { it.startUs },
    { it.endUs },
    { entry ->
        // Only lines of words join: a picture's cue stays as it is.
        if (entry.cues.any { it.text.isNullOrEmpty() }) emptyList()
        else entry.cues.flatMap { it.text.toString().split('\n') }.map { it.trim() }.filter { it.isNotEmpty() }
    },
) { start, end, run, lines ->
    val first = run.first().cues.first()
    val text = SpannableStringBuilder()
    lines.forEachIndexed { index, line ->
        if (index > 0) text.append('\n')
        text.append(line)
    }
    TimedCues(start, end, listOf(first.buildUpon().setText(text).build()))
}

// ---- Keeping the subtitles on screen through a reload ----

/** A subtitle track as it's known again after the tracks are built anew. */
internal data class KeptTrack(val sidecar: String?, val id: String?, val language: String?, val label: String?)

/**
 * Which of `tracks` is `kept`: the same subtitle file, else the same own track by its id, else by
 * its language and name; null when it's gone.
 */
internal fun keptTrackIndex(tracks: List<KeptTrack>, kept: KeptTrack): Int? {
    val index = when {
        kept.sidecar != null -> tracks.indexOfFirst { it.sidecar == kept.sidecar }
        else -> tracks.indexOfFirst { it.sidecar == null && kept.id != null && it.id == kept.id }
            .takeIf { it >= 0 }
            ?: tracks.indexOfFirst { it.sidecar == null && it.language == kept.language && it.label == kept.label }
    }
    return index.takeIf { it >= 0 }
}

/**
 * The main subtitles drawn from their file here when the chosen track is a subtitle file
 * (a sidecar): the file is read once ([SubtitleFiles]) and its lines are shown at the playhead
 * less the delay ([subtitleTimeMs]), looked at again when the next line starts or ends at the
 * playing speed ([ClockCore]). So the delay applies to what's on screen at once, works either way
 * and longer than the video, and follows seeks and speed changes exactly; Media3's own drawing of
 * the same track is set aside meanwhile. The video's own tracks stay with Media3's renderer
 * ([OffsetTextRenderer]). The second subtitles' file lines change on the same looks.
 */
internal class SubtitleClock(private val host: VideoPlayerActivity) {
    private val parsed = mutableMapOf<Uri, List<TimedCues>>()
    private var reading: Uri? = null
    /** The chosen file, while its lines are drawn here. */
    private var active: Sidecar? = null
    private val core = ClockCore()
    /** The joined lines of a file ([joinedCues]), kept while it's the one drawn. */
    private var joined: Pair<Uri, List<TimedCues>>? = null

    /** Lines of the chosen file are drawn here: Media3's cues are set aside. */
    val drawing: Boolean get() = active?.let { parsed.containsKey(it.uri) } == true

    private val ticker = object : Runnable {
        override fun run() = tick()
    }

    /** A new video: nothing is drawn until a file is chosen again. */
    fun reset() {
        host.main.removeCallbacks(ticker)
        active = null
        core.forget()
        reading = null
        joined = null
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
        if (chosen == active) {
            // The same file after a reload: looked at again from where the video is.
            refresh()
            return
        }
        val wasDrawing = drawing
        active = chosen
        core.forget()
        host.main.removeCallbacks(ticker)
        // A file drawn here gave way: Media3's cues for what's chosen now show (not the file's
        // last lines, which would stay until Media3's next change).
        if (wasDrawing && chosen == null && host.liveTracks.primary == null) {
            host.mainCues.set(host.player.currentCues.cues)
        }
        val sidecar = chosen ?: return refresh()
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

    /** Draws now (a seek, a new delay or speed, a pause, a setting) and goes on from here. */
    fun refresh() {
        host.main.removeCallbacks(ticker)
        core.forget()
        tick()
    }

    /** The lines drawn for `sidecar`'s file: joined when Smart Subtitles joins close lines. */
    private fun linesOf(sidecar: Sidecar): List<TimedCues>? {
        val timed = parsed[sidecar.uri] ?: return null
        if (!host.joinCloseLines) return timed
        joined?.takeIf { it.first == sidecar.uri }?.let { return it.second }
        return joinedCues(timed).also { joined = sidecar.uri to it }
    }

    private fun tick() {
        host.main.removeCallbacks(ticker)
        val player = host.player
        val playing = player.isPlaying
        val speed = player.playbackParameters.speed
        val delay = host.state.subtitleDelayMs
        var wait: Long? = null
        val timed = active?.let { linesOf(it) }
        if (timed != null) {
            val (changed, next) = core.look(timed, player.currentPosition, delay, playing, speed)
            changed?.let { now -> host.mainCues.set(now.flatMap { it.cues }) }
            wait = next
        }
        // The second subtitles' file: its lines change on their own times too.
        if (host.second.active) {
            val positionMs = host.liveStream.clockMs()
            host.second.update(positionMs, delay)
            if (playing) {
                val atUs = subtitleTimeMs(positionMs, delay) * 1000
                val next = waitMs(host.second.nextEdgeUs(atUs), atUs, speed)
                wait = wait?.let { minOf(it, next) } ?: next
            }
        }
        if (wait != null) host.main.postDelayed(ticker, wait)
    }

    /** Media3's cues for the main subtitles: shown unless the file is drawn here. */
    fun fromMedia3(cues: List<Cue>) {
        if (!drawing) host.mainCues.set(cues)
    }
}

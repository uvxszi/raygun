package app.raygun.audio

import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Timeline
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.hls.HlsManifest
import org.json.JSONObject

/** Less than this to seek in, and a live stream's seek bar hides. */
private const val DVR_MIN_MS = 60_000L
/** Within this of where Media3 plays live streams, the picture counts as live. */
private const val AT_LIVE_SLACK_MS = 3_000L
/** A hold this close to what AI subtitles need is enough. */
private const val HOLD_SLACK_MS = 3_000L
/** Steps of the live delay sheet. */
private const val HOLD_STEP_MS = 5_000L

/**
 * Live streams (HLS without an end, DASH `dynamic`, or a request marked `live`): they start at
 * their live edge, keep no place to resume, and report their positions on the stream's own clock
 * ([clockMs]: Media3's position in its first period, which stays put while the live window
 * slides), with where the segment playing starts on it ([anchor]) so Rust can place the stream's
 * sound for AI subtitles. The controls show a LIVE badge (a tap goes back to the live edge) and
 * the seek bar becomes the DVR window, or hides when there's too little to seek in.
 *
 * AI subtitles on a live stream need its sound before the picture shows it: a window is heard
 * once all of it is out, so their cues come a window and the time to hear it late. Once Rust says
 * how far behind live that is (`liveDelayMs` of the AI track), this offers to hold the picture
 * that far behind the edge ([hold]; Media3 keeps a live offset a seek sets), and keeps it there as
 * the need grows. The More sheet's Live delay changes it.
 */
@OptIn(UnstableApi::class)
internal class PlayerLive(private val host: VideoPlayerActivity) {
    /** The request says the stream is live. */
    private var hinted = false
    /** A start asked for, which a live stream doesn't keep. */
    private var askedStart = 0L
    private var startChecked = false
    /** How far behind the live edge the picture is held (0: as Media3 plays live streams). */
    var heldMs = 0L
        private set
    /** What AI subtitles need now, while they're made for this stream. */
    private var neededMs: Long? = null
    /** The need last turned down; asked again only once it grows. */
    private var declinedMs = 0L
    private val window = Timeline.Window()

    val isLive: Boolean get() = hinted || host.player.isCurrentMediaItemLive

    /** "● LIVE": red at the live edge, grey behind it; a tap goes back to the edge. */
    val badge: TextView = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
        typeface = Typeface.DEFAULT_BOLD
        gravity = Gravity.CENTER
        setPadding(host.dp(10), host.dp(4), host.dp(10), host.dp(4))
        background = rounded(0x66000000, host.dp(12).toFloat())
        isClickable = true
        isFocusable = true
        contentDescription = host.label("goLive", "Go to the live edge")
        setOnClickListener {
            goLive()
            host.controls.bump()
        }
        visibility = View.GONE
    }
    private val dot = GradientDrawable().apply {
        shape = GradientDrawable.OVAL
        setSize(host.dp(8), host.dp(8))
    }

    private val promptText = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
        setPadding(host.dp(14), host.dp(8), host.dp(6), host.dp(8))
        maxWidth = host.dp(280)
    }

    /** The offer to hold the picture behind live for AI subtitles. */
    val prompt: LinearLayout = LinearLayout(host).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        background = rounded(0xE6202124.toInt(), host.dp(20).toFloat())
        setPadding(0, host.dp(4), host.dp(4), host.dp(4))
        addView(promptText)
        addView(host.pill(host.label("notNow", "Not now")) {
            declinedMs = neededMs ?: 0L
            visibility = View.GONE
        })
        addView(host.pill(host.label("liveDelay", "Delay"), filled = true) {
            neededMs?.let { hold(it) }
            visibility = View.GONE
        })
        visibility = View.GONE
    }

    /** A new video: a live request plays from its live edge. */
    fun onLoad(request: JSONObject, startMs: Long) {
        hinted = request.optBoolean("live")
        askedStart = startMs
        startChecked = false
        heldMs = 0L
        neededMs = null
        declinedMs = 0L
        prompt.visibility = View.GONE
    }

    /**
     * Whether `startMs` should be kept for this request: a live one starts where Media3 plays
     * live streams.
     */
    fun keepsStart(startMs: Long): Boolean = !hinted && startMs > 0

    /**
     * Once the stream can play: one that turned out live after a start was asked for goes to its
     * live edge instead; true when it did.
     */
    fun onReady(): Boolean {
        if (startChecked) return false
        startChecked = true
        if (!host.player.isCurrentMediaItemLive || askedStart <= 0 || hinted) return false
        host.player.seekToDefaultPosition()
        return true
    }

    private fun currentWindow(): Timeline.Window? {
        val timeline = host.player.currentTimeline
        if (timeline.isEmpty) return null
        return timeline.getWindow(host.player.currentMediaItemIndex, window)
    }

    /** Where the picture is on the stream's own clock; the position for anything not live. */
    fun clockMs(): Long {
        val position = host.player.currentPosition.coerceAtLeast(0)
        if (!isLive) return position
        val shown = currentWindow() ?: return position
        return position + shown.positionInFirstPeriodMs.coerceAtLeast(0)
    }

    /**
     * The segment playing (HLS) and where it starts on [clockMs]'s clock, by its media sequence
     * number: Rust places the stream's sound by it.
     */
    fun anchor(): JSONObject? {
        if (!isLive) return null
        val manifest = host.player.currentManifest as? HlsManifest ?: return null
        val playlist = manifest.mediaPlaylist
        if (playlist.segments.isEmpty()) return null
        val shown = currentWindow() ?: return null
        val positionUs = host.player.currentPosition * 1000
        val index = playlist.segments.indexOfLast { it.relativeStartTimeUs <= positionUs }.coerceAtLeast(0)
        val segment = playlist.segments[index]
        return JSONObject()
            .put("sequence", playlist.mediaSequence + index)
            .put("startMs", shown.positionInFirstPeriodMs.coerceAtLeast(0) + segment.relativeStartTimeUs / 1000)
    }

    /** How far the picture is behind the newest segment, ms; null when not known. */
    private fun behindEdgeMs(): Long? {
        val shown = currentWindow() ?: return null
        val edge = shown.durationMs.takeIf { it != C.TIME_UNSET && it > 0 } ?: return null
        return (edge - host.player.currentPosition).coerceAtLeast(0)
    }

    /** How far behind where Media3 plays live streams the picture is, ms. */
    private fun behindLiveMs(): Long {
        val shown = currentWindow() ?: return 0
        val live = shown.defaultPositionMs.takeIf { it != C.TIME_UNSET } ?: return 0
        return (live - host.player.currentPosition).coerceAtLeast(0)
    }

    /** Whether the seek bar is worth showing: a DVR window long enough to move about in. */
    fun seekable(): Boolean {
        val length = host.player.duration.takeIf { it != C.TIME_UNSET } ?: return false
        return host.player.isCurrentMediaItemSeekable && length >= DVR_MIN_MS
    }

    /** "−2:30" behind the live edge, or nothing at it. */
    fun positionText(): String {
        val behind = behindLiveMs()
        return if (behind > AT_LIVE_SLACK_MS || heldMs > 0) "-" + formatTime(behindEdgeMs() ?: behind) else ""
    }

    /** Refreshes the badge (from the controls' update). */
    fun update() {
        if (!isLive) {
            badge.visibility = View.GONE
            return
        }
        val atLive = heldMs == 0L && behindLiveMs() <= AT_LIVE_SLACK_MS
        dot.setColor(if (atLive) 0xFFE53935.toInt() else 0xFF9E9E9E.toInt())
        badge.setCompoundDrawablesRelativeWithIntrinsicBounds(dot, null, null, null)
        badge.compoundDrawablePadding = host.dp(6)
        badge.show(host.label("live", "LIVE"))
        badge.visibility = View.VISIBLE
    }

    /** Back to the live edge (where Media3 plays live streams), letting go of any hold. */
    fun goLive() {
        heldMs = 0L
        // At the edge on purpose: AI subtitles don't ask again until they need more.
        declinedMs = maxOf(declinedMs, neededMs ?: 0L)
        prompt.visibility = View.GONE
        host.player.seekToDefaultPosition()
        if (!host.player.playWhenReady) host.player.play()
    }

    /**
     * Holds the picture `delayMs` behind the newest segment (as far back as the stream keeps,
     * leaving one segment's room).
     */
    fun hold(delayMs: Long) {
        if (delayMs <= 0) {
            goLive()
            return
        }
        val shown = currentWindow() ?: return
        val edge = shown.durationMs.takeIf { it != C.TIME_UNSET && it > 0 } ?: return
        val room = targetMs().coerceAtMost(edge / 2)
        val position = (edge - delayMs).coerceIn(room, edge)
        heldMs = delayMs
        prompt.visibility = View.GONE
        host.player.seekTo(position)
        val held = edge - position
        if (held < delayMs - HOLD_SLACK_MS) {
            host.showHint(host.label("liveKeepsOnly", "This stream keeps only {n}: subtitles may come late", formatTime(edge)), 4_000)
        } else {
            host.showHint(host.label("liveDelayed", "Playing {n} behind live", seconds(held)), 2_500)
        }
    }

    /** The stream's segment length (HLS), else 6 s. */
    private fun targetMs(): Long {
        val manifest = host.player.currentManifest as? HlsManifest
        return manifest?.mediaPlaylist?.targetDurationUs?.div(1000)?.takeIf { it > 0 } ?: 6_000L
    }

    private fun seconds(ms: Long): String = host.label("seconds", "{n} s", ((ms + 500) / 1000).toString())

    /** AI subtitles' state for this stream, from the `subtitleTrack` updates of its AI track. */
    fun onSubtitleUpdate(update: JSONObject) {
        if (update.optString("type") != "subtitleTrack" || update.optString("kind") != "ai") return
        val status = update.optJSONObject("status")
        neededMs = status?.takeIf { it.optBoolean("running") }?.number("liveDelayMs")?.takeIf { it > 0 }
    }

    /** Several times a second: the offer, and a hold that keeps up as AI subtitles need more. */
    fun tick() {
        val need = neededMs
        if (!isLive || need == null || host.isLoading()) {
            prompt.visibility = View.GONE
            return
        }
        if (heldMs > 0) {
            // Accepted before: keep up without asking.
            if (need > heldMs + HOLD_SLACK_MS) hold(need)
            prompt.visibility = View.GONE
            return
        }
        val behind = behindEdgeMs() ?: return
        if (behind >= need - HOLD_SLACK_MS || need <= declinedMs + HOLD_SLACK_MS) {
            prompt.visibility = View.GONE
            return
        }
        promptText.show(host.label("liveDelayAsk", "AI subtitles need the picture {n} behind live", seconds(need)))
        prompt.visibility = View.VISIBLE
    }

    /** The More sheet's Live delay: back to live, AI subtitles' need, or a step either way. */
    fun delaySheet() {
        val sheet = PlayerSheet(host, host.label("liveDelayTitle", "Live delay"))
        val behind = behindEdgeMs() ?: 0L
        sheet.stepper(
            host.label("liveBehind", "Behind live"),
            if (heldMs > 0) seconds(heldMs) else host.label("liveAtEdge", "At the live edge"),
            listOf(
                "−5 s" to {
                    hold((heldMs.takeIf { it > 0 } ?: behind) - HOLD_STEP_MS)
                    delaySheet()
                },
                "+5 s" to {
                    hold((heldMs.takeIf { it > 0 } ?: behind) + HOLD_STEP_MS)
                    delaySheet()
                },
            ),
        )
        neededMs?.let { need ->
            sheet.row(host.label("liveMatchAi", "Keep AI subtitles in time ({n})", seconds(need)), selected = heldMs >= need - HOLD_SLACK_MS) {
                hold(need)
                sheet.dismiss()
            }
        }
        sheet.row(host.label("goLive", "Go to the live edge"), selected = heldMs == 0L && behindLiveMs() <= AT_LIVE_SLACK_MS) {
            goLive()
            sheet.dismiss()
        }
        sheet.show()
    }
}

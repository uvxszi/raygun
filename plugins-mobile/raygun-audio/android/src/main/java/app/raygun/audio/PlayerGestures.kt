package app.raygun.audio

import android.content.Context
import android.media.AudioManager
import android.os.SystemClock
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import kotlin.math.abs
import kotlin.math.roundToInt

/** How long repeated double taps on one side keep adding up. */
private const val TAP_STREAK_MS = 1_000L
/** A full-height swipe moves brightness or volume this far. */
private const val SWIPE_RANGE = 1.2f

/**
 * Touch on the picture (M45, after Aniyomi's `GestureHandler`): a tap shows or hides the controls;
 * double taps on the left, centre and right do what Settings → Player → Gestures says, adding up
 * when repeated; vertical swipes change brightness (dimming below the lowest) and volume (boosted
 * past 100 %); a sideways swipe previews a seek and seeks on release; holding plays faster until
 * let go. While locked only the tap works, to bring back the unlock button.
 */
internal class PlayerGestures(private val host: VideoPlayerActivity) {
    private val context: Context = host
    private val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private val slop = ViewConfiguration.get(context).scaledTouchSlop
    private val edge = context.dp(32)
    val brightnessBar = LevelBar(context, R.drawable.raygun_ic_brightness)
    val volumeBar = LevelBar(context, R.drawable.raygun_ic_volume)

    private enum class Mode { NONE, IGNORE, SEEK, BRIGHTNESS, VOLUME, HOLD }

    private var mode = Mode.NONE
    private var downX = 0f
    private var downY = 0f
    private var startPosition = 0L
    private var seekTarget = 0L
    private var startLevel = 0f
    /** Past 1, how much of the boost is in use (0.3 = +30 %). */
    var boost = 0f
        private set
    private var streakSide = 0
    private var streakAt = 0L
    private var streakMs = 0L
    private var heldSpeed = 1f
    private val hideBars = Runnable {
        brightnessBar.root.visibility = View.GONE
        volumeBar.root.visibility = View.GONE
    }

    private val detector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent): Boolean = true

        override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
            host.controls.toggle()
            return true
        }

        override fun onDoubleTap(e: MotionEvent): Boolean {
            if (host.controls.locked) return false
            doubleTap(e.x)
            return true
        }

        override fun onLongPress(e: MotionEvent) {
            if (!host.controls.locked && mode == Mode.NONE) startHold()
        }
    })

    fun onTouch(view: View, event: MotionEvent): Boolean {
        detector.onTouchEvent(event)
        if (host.controls.locked) return true
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downX = event.x
                downY = event.y
                mode = Mode.NONE
            }
            MotionEvent.ACTION_MOVE -> {
                val dx = event.x - downX
                val dy = event.y - downY
                if (mode == Mode.NONE && (abs(dx) > slop || abs(dy) > slop)) start(view, dx, dy)
                when (mode) {
                    Mode.SEEK -> moveSeek(dx)
                    Mode.BRIGHTNESS -> moveBrightness(view, dy)
                    Mode.VOLUME -> moveVolume(view, dy)
                    else -> {}
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                when (mode) {
                    Mode.SEEK -> {
                        if (event.actionMasked == MotionEvent.ACTION_UP) host.player.seekTo(seekTarget)
                        host.hideHint()
                    }
                    Mode.HOLD -> endHold()
                    Mode.BRIGHTNESS, Mode.VOLUME -> {
                        host.main.removeCallbacks(hideBars)
                        host.main.postDelayed(hideBars, 800)
                    }
                    else -> {}
                }
                mode = Mode.NONE
            }
        }
        return true
    }

    /** Decides what a swipe does from its direction and where it started. */
    private fun start(view: View, dx: Float, dy: Float) {
        val prefs = host.prefs
        val nearEdge = downX < edge || downY < edge || downX > view.width - edge || downY > view.height - edge
        mode = when {
            nearEdge -> Mode.IGNORE
            abs(dx) > abs(dy) -> {
                val player = host.player
                if (prefs.seekGesture && player.duration > 0 && player.isCurrentMediaItemSeekable) {
                    startPosition = player.currentPosition
                    seekTarget = startPosition
                    Mode.SEEK
                } else {
                    Mode.IGNORE
                }
            }
            else -> {
                val left = downX < view.width / 2f
                val brightnessSide = left != prefs.swapSides
                when {
                    brightnessSide && prefs.brightnessGesture -> {
                        startLevel = host.brightness
                        Mode.BRIGHTNESS
                    }
                    !brightnessSide && prefs.volumeGesture -> {
                        startLevel = volumeLevel()
                        Mode.VOLUME
                    }
                    else -> Mode.IGNORE
                }
            }
        }
        if (mode == Mode.SEEK) host.controls.hide()
    }

    private fun moveSeek(dx: Float) {
        val player = host.player
        // Aniyomi's 0.15 s per pixel, on a screen of this density.
        val perPixel = 150f * 2.75f / context.resources.displayMetrics.density
        seekTarget = (startPosition + (dx * perPixel).toLong()).coerceIn(0L, player.duration.coerceAtLeast(0L))
        val change = seekTarget - startPosition
        val sign = if (change < 0) "-" else "+"
        host.showHint("$sign${formatTime(abs(change))}\n${formatTime(seekTarget)} / ${formatTime(player.duration)}", 0)
    }

    private fun moveBrightness(view: View, dy: Float) {
        val level = (startLevel - dy / view.height * SWIPE_RANGE).coerceIn(-0.75f, 1f)
        host.setBrightness(level)
        brightnessBar.set((level + 0.75f) / 1.75f, "${(level * 100).roundToInt()}%", level < 0f)
        host.main.removeCallbacks(hideBars)
    }

    /** The volume as a fraction of the maximum, plus the boost in use. */
    private fun volumeLevel(): Float {
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        return audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat() / max + boost
    }

    private fun moveVolume(view: View, dy: Float) {
        val cap = host.prefs.volumeBoost
        val level = (startLevel - dy / view.height * SWIPE_RANGE).coerceIn(0f, 1f + cap)
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        val wanted = (minOf(level, 1f) * max).roundToInt()
        if (wanted != audio.getStreamVolume(AudioManager.STREAM_MUSIC)) {
            audio.setStreamVolume(AudioManager.STREAM_MUSIC, wanted, 0)
        }
        boost = (level - 1f).coerceAtLeast(0f)
        host.setBoost(boost)
        volumeBar.set(level / (1f + cap), "${(level * 100).roundToInt()}%", boost > 0f)
        host.main.removeCallbacks(hideBars)
    }

    /** Drops the boost (a new video starts at the system volume). */
    fun resetBoost() {
        boost = 0f
        host.setBoost(0f)
    }

    private fun doubleTap(x: Float) {
        val width = host.playerView.width.takeIf { it > 0 } ?: return
        val side = when {
            x < width * 2f / 5f -> -1
            x > width * 3f / 5f -> 1
            else -> 0
        }
        val prefs = host.prefs
        val action = when (side) {
            -1 -> prefs.doubleTapLeft
            1 -> prefs.doubleTapRight
            else -> prefs.doubleTapCenter
        }
        when (action) {
            "seek" -> if (side == 0) host.togglePlay() else seekStreak(side)
            "playPause" -> host.togglePlay()
            "switchEpisode" -> when (side) {
                -1 -> host.request("previous")
                1 -> host.requestNext()
                else -> host.togglePlay()
            }
            else -> {}
        }
    }

    /** Taps on the same side soon after each other add up: +10 s, +20 s… */
    private fun seekStreak(side: Int) {
        val now = SystemClock.uptimeMillis()
        val step = host.seekStepMs
        streakMs = if (side == streakSide && now - streakAt < TAP_STREAK_MS) streakMs + step else step
        streakSide = side
        streakAt = now
        host.seekBy(side * step)
        val seconds = streakMs / 1000
        host.showHint(if (side < 0) "« $seconds s" else "$seconds s »")
    }

    private fun startHold() {
        val speed = host.prefs.holdSpeed
        val player = host.player
        if (speed <= 0f || !player.isPlaying) return
        mode = Mode.HOLD
        heldSpeed = player.playbackParameters.speed
        host.applySpeed(speed)
        // The controls would cover the badge; the picture stays clear while held.
        host.controls.hide()
        host.showSpeedBadge("${formatSpeed(speed)} »")
    }

    private fun endHold() {
        host.applySpeed(heldSpeed)
        host.showSpeedBadge(null)
    }
}

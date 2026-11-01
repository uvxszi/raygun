package app.raygun.audio

import android.annotation.SuppressLint
import android.app.PictureInPictureParams
import android.content.Context
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.graphics.Color
import android.media.AudioManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Rational
import android.util.TypedValue
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlaybackException
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.mediacodec.MediaCodecUtil
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import org.json.JSONObject
import kotlin.math.abs

private const val REPORT_INTERVAL_MS = 5_000L
private const val SEEK_STEP_MS = 10_000L
private const val DEFAULT_USER_AGENT =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36"

/**
 * Raygun's video player (M38): Media3 ExoPlayer with the stock controls (seek bar, speed, audio and
 * subtitle tracks), plus double-tap seeking, brightness and volume swipes, picture-in-picture and
 * resume. Started by [RaygunAudioPlugin.videoOpen] with a JSON request; reports progress to Rust
 * through [NativeBridge.video] so the app can offer "continue watching".
 */
class VideoPlayerActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_REQUEST = "app.raygun.audio.VIDEO_REQUEST"
    }

    private lateinit var player: ExoPlayer
    private lateinit var playerView: PlayerView
    private lateinit var hint: TextView
    private var videoId = ""
    private var ended = false
    /** After a decoder fails mid-playback, software decoders are tried first. */
    private var softwareFirst = false
    private val codecSelector = MediaCodecSelector { mimeType, secure, tunneling ->
        val decoders = MediaCodecUtil.getDecoderInfos(mimeType, secure, tunneling)
        if (softwareFirst) decoders.sortedBy { if (it.softwareOnly) 0 else 1 } else decoders
    }
    private val main = Handler(Looper.getMainLooper())
    private val hideHint = Runnable { hint.visibility = View.GONE }
    private val ticker = object : Runnable {
        override fun run() {
            if (player.isPlaying) report("progress")
            main.postDelayed(this, REPORT_INTERVAL_MS)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val request = JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: "{}")
        videoId = request.optString("id")

        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        window.decorView.setBackgroundColor(Color.BLACK)
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        hideSystemBars()

        val headers = mutableMapOf<String, String>()
        request.optJSONObject("headers")?.let { json ->
            json.keys().forEach { key -> headers[key] = json.optString(key) }
        }
        val userAgent = headers.entries.firstOrNull { it.key.equals("User-Agent", true) }?.value
            ?: DEFAULT_USER_AGENT
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(userAgent)
            .setDefaultRequestProperties(headers.filterKeys { !it.equals("User-Agent", true) })
            .setAllowCrossProtocolRedirects(true)
        // A hardware decoder that fails (some phones, the emulator at 1080p) hands over to the next
        // one, ending with the software decoders.
        val renderers = DefaultRenderersFactory(this)
            .setEnableDecoderFallback(true)
            .setMediaCodecSelector(codecSelector)
        player = ExoPlayer.Builder(this, renderers)
            .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(this, http)))
            .setSeekBackIncrementMs(SEEK_STEP_MS)
            .setSeekForwardIncrementMs(SEEK_STEP_MS)
            .build()

        val subtitles = mutableListOf<MediaItem.SubtitleConfiguration>()
        request.optJSONArray("subtitles")?.let { list ->
            for (index in 0 until list.length()) {
                val track = list.optJSONObject(index) ?: continue
                val url = track.optString("url").takeIf { it.isNotBlank() } ?: continue
                subtitles += MediaItem.SubtitleConfiguration.Builder(Uri.parse(url))
                    .setMimeType(subtitleMime(track.optString("format"), url))
                    .setLanguage(track.optString("lang").ifBlank { null })
                    .setLabel(track.optString("label").ifBlank { null })
                    .build()
            }
        }
        val item = MediaItem.Builder()
            .setUri(request.optString("url"))
            .apply { streamMime(request.optString("format"))?.let { setMimeType(it) } }
            .setSubtitleConfigurations(subtitles)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(request.optString("title"))
                    .setSubtitle(request.optString("subtitle").ifBlank { null })
                    .build(),
            )
            .build()
        val startMs = request.optDouble("startMs", 0.0).toLong().coerceAtLeast(0)
        player.setMediaItem(item, startMs)
        player.prepare()
        player.playWhenReady = true
        player.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED && !ended) {
                    ended = true
                    report("ended")
                }
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (!isPlaying) report("progress")
            }

            override fun onPlayerError(error: PlaybackException) {
                val rendererFailed = error is ExoPlaybackException &&
                    error.type == ExoPlaybackException.TYPE_RENDERER
                if (rendererFailed && !softwareFirst) {
                    // Retry once from the same spot with software decoders.
                    softwareFirst = true
                    val position = player.currentPosition
                    player.seekTo(position)
                    player.prepare()
                    player.play()
                    return
                }
                report("error", error.message ?: error.errorCodeName)
                showHint(getString(android.R.string.dialog_alert_title) + ": " + error.errorCodeName)
            }
        })

        playerView = PlayerView(this).apply {
            this.player = this@VideoPlayerActivity.player
            setShowSubtitleButton(true)
            setShowNextButton(false)
            setShowPreviousButton(false)
            setShowFastForwardButton(true)
            setShowRewindButton(true)
            controllerShowTimeoutMs = 3_500
            keepScreenOn = true
        }
        hint = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            setBackgroundColor(0x99000000.toInt())
            setPadding(32, 16, 32, 16)
            visibility = View.GONE
        }
        val title = TextView(this).apply {
            text = request.optString("title")
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setPadding(48, 32, 48, 16)
            maxLines = 1
        }
        playerView.setControllerVisibilityListener(
            PlayerView.ControllerVisibilityListener { visibility -> title.visibility = visibility },
        )
        val root = FrameLayout(this).apply {
            addView(playerView, FrameLayout.LayoutParams(-1, -1))
            addView(title, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
            addView(hint, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
        }
        setContentView(root)

        val gestures = GestureDetector(this, Gestures())
        playerView.setOnTouchListener { _, event ->
            gestures.onTouchEvent(event)
            true
        }
        main.postDelayed(ticker, REPORT_INTERVAL_MS)
    }

    /** Double-tap sides to seek, tap to show the controls, swipe left/right half for brightness/volume. */
    private inner class Gestures : GestureDetector.SimpleOnGestureListener() {
        private var accumulated = 0f

        override fun onDown(event: MotionEvent): Boolean {
            accumulated = 0f
            return true
        }

        override fun onSingleTapConfirmed(event: MotionEvent): Boolean {
            if (playerView.isControllerFullyVisible) playerView.hideController() else playerView.showController()
            return true
        }

        override fun onDoubleTap(event: MotionEvent): Boolean {
            val width = playerView.width.takeIf { it > 0 } ?: return false
            when {
                event.x < width / 3f -> {
                    player.seekBack()
                    showHint("« ${SEEK_STEP_MS / 1000}s")
                }
                event.x > width * 2 / 3f -> {
                    player.seekForward()
                    showHint("${SEEK_STEP_MS / 1000}s »")
                }
                else -> if (player.isPlaying) player.pause() else player.play()
            }
            return true
        }

        override fun onScroll(start: MotionEvent?, event: MotionEvent, dx: Float, dy: Float): Boolean {
            val from = start ?: return false
            if (abs(event.y - from.y) < abs(event.x - from.x)) return false
            val height = playerView.height.takeIf { it > 0 } ?: return false
            val change = dy / height
            if (from.x < playerView.width / 2f) adjustBrightness(change) else adjustVolume(change)
            return true
        }

        private fun adjustBrightness(change: Float) {
            val attributes = window.attributes
            val current = if (attributes.screenBrightness < 0) 0.5f else attributes.screenBrightness
            attributes.screenBrightness = (current + change).coerceIn(0.01f, 1f)
            window.attributes = attributes
            showHint("☀ ${(attributes.screenBrightness * 100).toInt()}%")
        }

        private fun adjustVolume(change: Float) {
            val audio = getSystemService(Context.AUDIO_SERVICE) as AudioManager
            val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
            accumulated += change * max
            val steps = accumulated.toInt()
            if (steps == 0) return
            accumulated -= steps
            val volume = (audio.getStreamVolume(AudioManager.STREAM_MUSIC) + steps).coerceIn(0, max)
            audio.setStreamVolume(AudioManager.STREAM_MUSIC, volume, 0)
            showHint("🔊 ${volume * 100 / max.coerceAtLeast(1)}%")
        }
    }

    private fun showHint(text: String) {
        hint.text = text
        hint.visibility = View.VISIBLE
        main.removeCallbacks(hideHint)
        main.postDelayed(hideHint, 800)
    }

    private fun hideSystemBars() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
    }

    /** Leaving the app while a video plays continues it in a picture-in-picture window. */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        if (player.isPlaying && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val size = player.videoSize
            val ratio = if (size.width > 0 && size.height > 0) {
                Rational(size.width, size.height).let {
                    // Android refuses ratios beyond 2.39:1.
                    if (it.toFloat() > 2.39f) Rational(239, 100) else if (it.toFloat() < 0.42f) Rational(100, 239) else it
                }
            } else {
                Rational(16, 9)
            }
            enterPictureInPictureMode(PictureInPictureParams.Builder().setAspectRatio(ratio).build())
        }
    }

    override fun onPictureInPictureModeChanged(inPip: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(inPip, newConfig)
        playerView.useController = !inPip
        if (inPip) playerView.hideController()
    }

    override fun onStop() {
        super.onStop()
        val inPip = Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && isInPictureInPictureMode
        if (!inPip) player.pause()
        report("progress")
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        report("closed")
        player.release()
        super.onDestroy()
    }

    private fun report(state: String, error: String? = null) {
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 }
        val json = JSONObject()
            .put("id", videoId)
            .put("state", state)
            .put("positionMs", player.currentPosition.coerceAtLeast(0))
            .put("durationMs", duration ?: JSONObject.NULL)
            .put("error", error ?: JSONObject.NULL)
        NativeBridge.video(json.toString())
    }

    private fun streamMime(format: String): String? = when (format.lowercase()) {
        "hls", "m3u8" -> MimeTypes.APPLICATION_M3U8
        "dash", "mpd" -> MimeTypes.APPLICATION_MPD
        else -> null
    }

    private fun subtitleMime(format: String, url: String): String {
        val kind = format.lowercase().ifBlank { url.substringBefore('?').substringAfterLast('.').lowercase() }
        return when (kind) {
            "vtt", "webvtt" -> MimeTypes.TEXT_VTT
            "ass", "ssa" -> MimeTypes.TEXT_SSA
            "ttml", "dfxp" -> MimeTypes.APPLICATION_TTML
            else -> MimeTypes.APPLICATION_SUBRIP
        }
    }
}

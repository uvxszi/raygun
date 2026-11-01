package app.raygun.audio

import android.annotation.SuppressLint
import android.app.AlertDialog
import android.app.PictureInPictureParams
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.media.AudioManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.Rational
import android.util.TypedValue
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
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
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.abs

private const val REPORT_INTERVAL_MS = 5_000L
private const val TICK_MS = 500L
private const val DEFAULT_SEEK_STEP_MS = 10_000L
private const val DEFAULT_PROMPT_MS = 10_000L
private const val DEFAULT_USER_AGENT =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36"

/**
 * Raygun's video player (M38): Media3 ExoPlayer with the stock controls (seek bar, speed, audio and
 * subtitle tracks), plus double-tap seeking, brightness and volume swipes, picture-in-picture and
 * resume. Started by [RaygunAudioPlugin.videoOpen] with a JSON request; reports progress to Rust
 * through [NativeBridge.video] so the app can offer "continue watching".
 *
 * Playback sessions (M44): a request with `session: true` may open before any video is known
 * (`loading`). Rust then sends updates through [RaygunAudioPlugin.videoUpdate]: `load` swaps in a
 * video without closing, `episodes` and `sources` fill the dialogs and the previous/next buttons,
 * `loading` and `error` show a message. The player asks for the next or previous episode, another
 * episode or another source with `request` reports.
 */
class VideoPlayerActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_REQUEST = "app.raygun.audio.VIDEO_REQUEST"

        /** The open player, for updates from Rust. */
        @Volatile
        private var live: VideoPlayerActivity? = null

        /** Hands an update to the open player; false when none is open. */
        fun deliver(json: String): Boolean {
            val activity = live ?: return false
            activity.runOnUiThread { activity.applyUpdate(JSONObject(json)) }
            return true
        }
    }

    private lateinit var player: ExoPlayer
    private lateinit var sessionPlayer: SessionPlayer
    private lateinit var playerView: PlayerView
    private lateinit var hint: TextView
    private lateinit var title: TextView
    private lateinit var topBar: LinearLayout
    private lateinit var episodesButton: TextView
    private lateinit var sourcesButton: TextView
    private lateinit var loading: LinearLayout
    private lateinit var loadingText: TextView
    private lateinit var nextPrompt: LinearLayout
    private lateinit var nextText: TextView
    private var videoId = ""
    private var ended = false
    /** After a decoder fails mid-playback, software decoders are tried first. */
    private var softwareFirst = false
    /** How far a double tap or the skip buttons jump (Settings → Player). */
    private var seekStepMs = DEFAULT_SEEK_STEP_MS
    private var autoSubtitleLanguage: String? = null
    private var labels = JSONObject()

    // Session state (M44).
    private var session = false
    private var hasNext = false
    private var hasPrevious = false
    private var autoplay = true
    /** How long before the end the next-episode prompt shows; 0 shows it at the end only. */
    private var promptMs = DEFAULT_PROMPT_MS
    private var nextCancelled = false
    private var nextRequested = false
    private var episodes = JSONArray()
    private var sources = JSONArray()

    private val codecSelector = MediaCodecSelector { mimeType, secure, tunneling ->
        val decoders = MediaCodecUtil.getDecoderInfos(mimeType, secure, tunneling)
        if (softwareFirst) decoders.sortedBy { if (it.softwareOnly) 0 else 1 } else decoders
    }
    private val main = Handler(Looper.getMainLooper())
    private val hideHint = Runnable { hint.visibility = View.GONE }
    private var sinceReport = 0L
    private val ticker = object : Runnable {
        override fun run() {
            sinceReport += TICK_MS
            if (sinceReport >= REPORT_INTERVAL_MS) {
                sinceReport = 0
                if (player.isPlaying) report("progress")
            }
            updateNextPrompt()
            main.postDelayed(this, TICK_MS)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        live = this
        val request = JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: "{}")
        seekStepMs = request.optLong("seekStepMs", DEFAULT_SEEK_STEP_MS).coerceIn(1_000L, 300_000L)
        labels = request.optJSONObject("labels") ?: JSONObject()
        session = request.optBoolean("session", false)
        autoplay = request.optBoolean("autoplay", true)
        promptMs = request.optLong("nextPromptMs", DEFAULT_PROMPT_MS).coerceIn(0L, 60_000L)
        autoSubtitleLanguage = if (request.has("autoSubtitleLanguage") && !request.isNull("autoSubtitleLanguage")) {
            request.optString("autoSubtitleLanguage").ifBlank { java.util.Locale.getDefault().language }
        } else {
            null
        }

        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        window.decorView.setBackgroundColor(Color.BLACK)
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        hideSystemBars()

        // A hardware decoder that fails (some phones, the emulator at 1080p) hands over to the next
        // one, ending with the software decoders.
        val renderers = DefaultRenderersFactory(this)
            .setEnableDecoderFallback(true)
            .setMediaCodecSelector(codecSelector)
        player = ExoPlayer.Builder(this, renderers)
            .setSeekBackIncrementMs(seekStepMs)
            .setSeekForwardIncrementMs(seekStepMs)
            .build()
        autoSubtitleLanguage?.let { language ->
            // Subtitles turn on by themselves in the chosen language (blank: the device's).
            player.trackSelectionParameters = player.trackSelectionParameters
                .buildUpon()
                .setPreferredTextLanguage(language)
                .setSelectUndeterminedTextLanguage(true)
                .build()
        }
        player.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED && !ended) {
                    ended = true
                    report("ended")
                    // Autoplay moves on at the end unless the prompt was cancelled.
                    if (session && hasNext && autoplay && !nextCancelled) requestNext()
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
                // A session tries its next source; a single video says what went wrong.
                if (session) {
                    showLoading(label("tryingNext", "Trying the next source…"))
                } else {
                    showHint(getString(android.R.string.dialog_alert_title) + ": " + error.errorCodeName)
                }
            }
        })
        sessionPlayer = SessionPlayer(player)

        playerView = PlayerView(this).apply {
            this.player = sessionPlayer
            setShowSubtitleButton(true)
            setShowNextButton(session)
            setShowPreviousButton(session)
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
        title = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            maxLines = 2
        }
        episodesButton = pill(label("episodes", "Episodes")) { showEpisodes() }
        sourcesButton = pill(label("sources", "Sources")) { showSources() }
        topBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(24), dp(16), dp(40), dp(8))
            addView(title, LinearLayout.LayoutParams(0, -2, 1f))
            if (session) {
                addView(episodesButton)
                addView(sourcesButton)
            }
        }
        playerView.setControllerVisibilityListener(
            PlayerView.ControllerVisibilityListener { visibility ->
                topBar.visibility = if (loading.visibility == View.VISIBLE) View.VISIBLE else visibility
            },
        )
        loadingText = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            gravity = Gravity.CENTER
            setPadding(dp(24), dp(12), dp(24), 0)
        }
        loading = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            addView(ProgressBar(this@VideoPlayerActivity))
            addView(loadingText)
            visibility = View.GONE
        }
        nextText = TextView(this).apply {
            setTextColor(Color.BLACK)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(18), dp(10), dp(18), dp(10))
            background = rounded(Color.WHITE)
            setOnClickListener { requestNext() }
        }
        val cancelNext = pill(label("cancel", "Cancel")) {
            nextCancelled = true
            nextPrompt.visibility = View.GONE
        }
        nextPrompt = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, dp(24), dp(96))
            addView(cancelNext)
            addView(nextText)
            visibility = View.GONE
        }
        val root = FrameLayout(this).apply {
            addView(playerView, FrameLayout.LayoutParams(-1, -1))
            addView(loading, FrameLayout.LayoutParams(-1, -1))
            addView(topBar, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
            addView(hint, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
            addView(nextPrompt, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END))
        }
        setContentView(root)
        // Above the video and its controls, so the buttons get their taps.
        topBar.elevation = dp(8).toFloat()
        topBar.bringToFront()
        nextPrompt.elevation = dp(8).toFloat()
        nextPrompt.bringToFront()

        val gestures = GestureDetector(this, Gestures())
        playerView.setOnTouchListener { _, event ->
            gestures.onTouchEvent(event)
            true
        }

        title.text = heading(request)
        if (request.optString("url").isNotBlank()) {
            load(request)
        } else {
            // A session opens before its first video is known.
            showLoading(request.optString("loadingText").ifBlank { label("findingSource", "Finding a source…") })
        }
        // Updates sent before this window existed were dropped: ask for the session's state.
        if (session) request("ready")
        main.postDelayed(ticker, TICK_MS)
    }

    /** A new request while open (another video was started) replaces what plays. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val request = JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: return)
        report("switched")
        session = request.optBoolean("session", false)
        playerView.setShowNextButton(session)
        playerView.setShowPreviousButton(session)
        title.text = heading(request)
        if (request.optString("url").isNotBlank()) load(request) else showLoading(label("findingSource", "Finding a source…"))
    }

    /** An update from Rust (M44). */
    private fun applyUpdate(update: JSONObject) {
        Log.d("RaygunVideo", "update ${update.optString("type")}")
        when (update.optString("type")) {
            "load" -> update.optJSONObject("request")?.let { load(it) }
            "loading" -> {
                // Another episode is on its way: the current one stops here.
                if (player.isPlaying) {
                    player.pause()
                    report("progress")
                }
                showLoading(update.optString("text"))
            }
            "error" -> {
                hideLoading()
                showHint(update.optString("text"))
                if (update.optBoolean("showSources")) showSources()
            }
            "episodes" -> {
                episodes = update.optJSONArray("items") ?: JSONArray()
                hasNext = update.optBoolean("hasNext")
                hasPrevious = update.optBoolean("hasPrevious")
                autoplay = update.optBoolean("autoplay", autoplay)
                // A film has one video: no episode list, no previous or next.
                val several = episodes.length() > 1
                episodesButton.visibility = if (several) View.VISIBLE else View.GONE
                playerView.setShowNextButton(several)
                playerView.setShowPreviousButton(several)
                refreshButtons()
            }
            "sources" -> sources = update.optJSONArray("groups") ?: JSONArray()
        }
    }

    /** Plays a video: url, headers, format, subtitles, startMs, id, title, subtitle. */
    private fun load(request: JSONObject) {
        if (videoId.isNotEmpty() && videoId != request.optString("id")) report("switched")
        videoId = request.optString("id")
        ended = false
        nextCancelled = false
        nextRequested = false
        nextPrompt.visibility = View.GONE
        title.text = heading(request)

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
        val source = DefaultMediaSourceFactory(DefaultDataSource.Factory(this, http)).createMediaSource(item)
        val startMs = request.optDouble("startMs", 0.0).toLong().coerceAtLeast(0)
        player.setMediaSource(source, startMs)
        player.prepare()
        player.playWhenReady = true
        hideLoading()
    }

    /** Previous and next go to Rust's session instead of a playlist. */
    private inner class SessionPlayer(player: Player) : ForwardingPlayer(player) {
        private val stepping = setOf(
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        )

        override fun getAvailableCommands(): Player.Commands {
            val commands = super.getAvailableCommands().buildUpon()
            commands.removeAll(*stepping.toIntArray())
            if (hasNext) commands.addAll(Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
            if (hasPrevious) commands.addAll(Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
            return commands.build()
        }

        override fun isCommandAvailable(command: Int): Boolean = when (command) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> hasNext
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> hasPrevious
            else -> super.isCommandAvailable(command)
        }

        override fun hasNextMediaItem() = hasNext

        override fun hasPreviousMediaItem() = hasPrevious

        override fun seekToNext() = requestNext()

        override fun seekToNextMediaItem() = requestNext()

        override fun seekToPrevious() = request("previous")

        override fun seekToPreviousMediaItem() = request("previous")
    }

    /** Re-binds the controls so the previous/next buttons follow the session. */
    private fun refreshButtons() {
        playerView.player = null
        playerView.player = sessionPlayer
    }

    private fun requestNext() {
        if (nextRequested || !hasNext) return
        nextRequested = true
        nextPrompt.visibility = View.GONE
        request("next")
    }

    /** Near the end of an episode with a next one: a "Next episode" button, counting down with autoplay. */
    private fun updateNextPrompt() {
        if (!session || !hasNext || nextCancelled || nextRequested) {
            if (::nextPrompt.isInitialized) nextPrompt.visibility = View.GONE
            return
        }
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: return
        val left = duration - player.currentPosition
        val window = if (promptMs > 0) promptMs else 1_000L
        if (left > window || left < 0 || !player.isPlaying && !ended) {
            nextPrompt.visibility = View.GONE
            return
        }
        val seconds = ((left + 999) / 1000).coerceAtLeast(0)
        nextText.text = if (autoplay) {
            label("nextIn", "Next episode in {n} s").replace("{n}", seconds.toString())
        } else {
            label("nextEpisode", "Next episode")
        }
        nextPrompt.visibility = View.VISIBLE
    }

    private fun showEpisodes() {
        Log.d("RaygunVideo", "episodes: ${episodes.length()}")
        if (episodes.length() == 0) {
            // Not here yet: ask again.
            showHint(label("findingSource", "Finding a source…"))
            request("ready")
            return
        }
        val names = Array(episodes.length()) { index ->
            val episode = episodes.optJSONObject(index) ?: JSONObject()
            val marks = buildString {
                if (episode.optBoolean("watched")) append(" ✓")
                if (episode.optBoolean("downloaded")) append(" ⤓")
            }
            episode.optString("label") + marks
        }
        var current = -1
        for (index in 0 until episodes.length()) {
            if (episodes.optJSONObject(index)?.optBoolean("current") == true) current = index
        }
        val autoplayLabel = label(if (autoplay) "autoplayOn" else "autoplayOff", if (autoplay) "Autoplay: on" else "Autoplay: off")
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle(label("episodes", "Episodes"))
            .setSingleChoiceItems(names, current) { dialog, which ->
                dialog.dismiss()
                if (which != current) request("episode", JSONObject().put("index", which))
            }
            .setNeutralButton(autoplayLabel) { _, _ ->
                autoplay = !autoplay
                request("autoplay", JSONObject().put("on", autoplay))
            }
            .show()
    }

    private fun showSources() {
        // Each row is a link, or a server's state while it has none.
        val rows = mutableListOf<Pair<String, JSONObject?>>()
        var current = -1
        for (group in 0 until sources.length()) {
            val server = sources.optJSONObject(group) ?: continue
            val name = server.optString("name")
            val links = server.optJSONArray("links") ?: JSONArray()
            if (links.length() == 0) {
                val state = when (server.optString("state")) {
                    "resolving" -> label("resolving", "finding links…")
                    "failed" -> label("failed", "failed") + server.optString("error").let { if (it.isBlank()) "" else ": $it" }
                    else -> label("empty", "no videos")
                }
                rows += "$name · $state" to null
                continue
            }
            for (link in 0 until links.length()) {
                val entry = links.optJSONObject(link) ?: continue
                if (entry.optBoolean("current")) current = rows.size
                val failed = if (entry.optBoolean("failed")) " ✕" else ""
                rows += "$name · ${entry.optString("label")}$failed" to
                    JSONObject().put("group", group).put("link", link)
            }
        }
        Log.d("RaygunVideo", "sources: ${rows.size}")
        if (rows.isEmpty()) {
            showHint(label("findingSource", "Finding a source…"))
            request("ready")
            return
        }
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle(label("sources", "Sources"))
            .setSingleChoiceItems(rows.map { it.first }.toTypedArray(), current) { dialog, which ->
                dialog.dismiss()
                val target = rows[which].second ?: return@setSingleChoiceItems
                if (which != current) {
                    showLoading(label("findingSource", "Finding a source…"))
                    request("source", target)
                }
            }
            .show()
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
                    showHint("« ${seekStepMs / 1000}s")
                }
                event.x > width * 2 / 3f -> {
                    player.seekForward()
                    showHint("${seekStepMs / 1000}s »")
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

    private fun showLoading(text: String) {
        loadingText.text = text
        loading.visibility = View.VISIBLE
        topBar.visibility = View.VISIBLE
    }

    private fun hideLoading() {
        loading.visibility = View.GONE
        topBar.visibility = if (playerView.isControllerFullyVisible) View.VISIBLE else View.GONE
    }

    private fun showHint(text: String) {
        hint.text = text
        hint.visibility = View.VISIBLE
        main.removeCallbacks(hideHint)
        main.postDelayed(hideHint, 1_500)
    }

    private fun heading(request: JSONObject): String {
        val name = request.optString("title")
        val sub = request.optString("subtitle")
        return if (sub.isBlank()) name else "$name\n$sub"
    }

    private fun label(key: String, fallback: String): String = labels.optString(key).ifBlank { fallback }

    private fun pill(text: String, onClick: () -> Unit) = TextView(this).apply {
        this.text = text
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
        setPadding(dp(14), dp(8), dp(14), dp(8))
        background = rounded(0x66000000)
        isClickable = true
        setOnClickListener { onClick() }
        layoutParams = LinearLayout.LayoutParams(-2, -2).apply { marginStart = dp(8) }
    }

    private fun rounded(color: Int) = GradientDrawable().apply {
        setColor(color)
        cornerRadius = dp(20).toFloat()
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

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
        topBar.visibility = if (inPip) View.GONE else topBar.visibility
    }

    override fun onStop() {
        super.onStop()
        val inPip = Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && isInPictureInPictureMode
        if (!inPip) player.pause()
        report("progress")
    }

    override fun onDestroy() {
        if (live === this) live = null
        main.removeCallbacksAndMessages(null)
        report("closed")
        player.release()
        super.onDestroy()
    }

    /** Asks the session for something: the next or previous episode, an episode, a source. */
    private fun request(action: String, extra: JSONObject = JSONObject()) {
        val json = JSONObject()
            .put("id", videoId)
            .put("state", "request")
            .put("action", action)
            .put("positionMs", player.currentPosition.coerceAtLeast(0))
        extra.keys().forEach { key -> json.put(key, extra.get(key)) }
        NativeBridge.video(json.toString())
    }

    private fun report(state: String, error: String? = null) {
        if (videoId.isEmpty() && state != "closed") return
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

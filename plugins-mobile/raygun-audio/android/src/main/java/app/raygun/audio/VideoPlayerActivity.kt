package app.raygun.audio

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.media.audiofx.LoudnessEnhancer
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.OpenableColumns
import android.provider.Settings
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.common.text.CueGroup
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlaybackException
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.mediacodec.MediaCodecUtil
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.util.DebugTextViewHelper
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.PlayerView
import androidx.media3.ui.SubtitleView
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale
import kotlin.math.abs
import kotlin.math.log10

private const val REPORT_INTERVAL_MS = 5_000L
private const val TICK_MS = 250L
private const val DEFAULT_SEEK_STEP_MS = 10_000L
private const val DEFAULT_PROMPT_MS = 10_000L
private const val DEFAULT_USER_AGENT =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36"

/**
 * Raygun's video player (M38): Media3 ExoPlayer, reporting progress to Rust through
 * [NativeBridge.video] so the app can offer "continue watching". Started by
 * [RaygunAudioPlugin.videoOpen] with a JSON request.
 *
 * Playback sessions (M44): a request with `session: true` may open before any video is known.
 * Rust then sends updates through [RaygunAudioPlugin.videoUpdate]: `load` swaps in a video,
 * `episodes` and `sources` fill the sheets and previous/next, `loading` and `error` show a message,
 * and `chapters` adds AniSkip's. The player asks for things with `request` reports.
 *
 * Controls and settings (M45): its own overlay ([PlayerControls]), gestures ([PlayerGestures]),
 * tracks with a subtitle delay and a second subtitle track ([PlayerTracks]), sheets
 * ([PlayerSheets]) and tools ([PlayerTools]). It follows Settings → Player (`prefs` in each
 * request) and the title's memory (`titleState`), and saves changes made here with `state` and
 * `prefs` requests.
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

    internal lateinit var player: ExoPlayer
    internal lateinit var playerView: PlayerView
    internal lateinit var controls: PlayerControls
    internal lateinit var gestures: PlayerGestures
    internal lateinit var tracks: PlayerTracks
    internal lateinit var sheets: PlayerSheets
    internal lateinit var tools: PlayerTools
    internal lateinit var second: SecondSubtitles
    /** Subtitles over the whole screen, not inside the picture's frame (crop enlarges that). */
    internal lateinit var subtitleView: SubtitleView
    private lateinit var secondView: SubtitleView
    private lateinit var root: FrameLayout
    private lateinit var hint: TextView
    /** A small pill at the top while the picture is held for speed. */
    private lateinit var speedBadge: TextView
    private lateinit var loading: LinearLayout
    private lateinit var loadingText: TextView
    private lateinit var nextPrompt: LinearLayout
    private lateinit var nextText: TextView
    private lateinit var dim: View
    private lateinit var stats: TextView
    private var statsHelper: DebugTextViewHelper? = null
    private var openSheet: PlayerSheet? = null
    private var loudness: LoudnessEnhancer? = null

    internal val main = Handler(Looper.getMainLooper())
    internal var prefs = PlayerPrefs(JSONObject())
    private var labels = JSONObject()
    /** How far a double tap, the keys and the skip buttons jump (Settings → Player). */
    internal var seekStepMs = DEFAULT_SEEK_STEP_MS
    private var autoSubtitleLanguage: String? = null
    private var heading = ""
    private var videoId = ""
    /** The video playing, as Rust sent it. */
    private var current = JSONObject()
    private var ended = false
    /** Software decoders first: the setting, or after a hardware decoder failed. */
    private var softwareFirst = false
    private var tracksPicked = false
    private var aniskipAsked = false
    private var manualRotation = false
    private var aspect = "fit"
    private var pendingSubtitle: String? = null
    private var externalOnly = false

    // The title's memory and the video's chapters (M45).
    internal var state = TitleState(null)
    private var group = ""
    internal var chapters: List<Chapter> = emptyList()
    internal val sidecars = mutableListOf<Sidecar>()
    /** Subtitle files added from the device for this video. */
    private val added = mutableListOf<Sidecar>()

    // The skip button.
    private var countdownChapter = -1
    private var countdownEndsAt = 0L
    private val cancelledCountdowns = mutableSetOf<Int>()
    private val autoSkipped = mutableSetOf<Int>()

    // The sleep timer: when it runs out (elapsed realtime; 0 = off), or at the episode's end.
    private var sleepAt = 0L
    internal var sleepAtEnd = false

    // Session state (M44).
    internal var session = false
    internal var hasNext = false
    internal var hasPrevious = false
    internal var autoplay = true
    /** How long before the end the next-episode prompt shows; 0 shows it at the end only. */
    private var promptMs = DEFAULT_PROMPT_MS
    private var nextCancelled = false
    private var nextRequested = false
    internal var episodes = JSONArray()
    internal var sources = JSONArray()
    internal val several: Boolean get() = episodes.length() > 1
    internal val statsShown: Boolean get() = statsHelper != null

    /** Brightness from -0.75 (dimmed below the lowest) to 1; the screen's own until changed. */
    internal var brightness: Float = Float.NaN
        get() {
            if (field.isNaN()) {
                field = window.attributes.screenBrightness.takeIf { it >= 0f } ?: systemBrightness()
            }
            return field
        }
        private set

    private val codecSelector = MediaCodecSelector { mimeType, secure, tunneling ->
        val decoders = MediaCodecUtil.getDecoderInfos(mimeType, secure, tunneling)
        if (softwareFirst) decoders.sortedBy { if (it.softwareOnly) 0 else 1 } else decoders
    }
    private val hideHintLater = Runnable { hint.visibility = View.GONE }
    private val resyncSubtitles = Runnable { player.seekTo(player.currentPosition) }
    private var sinceReport = 0L
    private val ticker = object : Runnable {
        override fun run() {
            tick()
            main.postDelayed(this, TICK_MS)
        }
    }

    private val subtitleFile = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let { addSubtitleFile(it) }
    }
    private val externalApp = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        onExternalResult(result.resultCode, result.data)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        live = this
        val request = JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: "{}")
        readSettings(request)

        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        window.decorView.setBackgroundColor(Color.BLACK)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        applyOrientation()
        hideSystemBars()

        tracks = PlayerTracks(this)
        softwareFirst = prefs.decoder == "software"
        // A hardware decoder that fails (some phones, the emulator at 1080p) hands over to the
        // next one, ending with the software decoders.
        val renderers = PlayerRenderers(this) { tracks.text = it }
            .setEnableDecoderFallback(true)
            .setMediaCodecSelector(codecSelector)
        player = ExoPlayer.Builder(this, renderers)
            .setSeekBackIncrementMs(seekStepMs)
            .setSeekForwardIncrementMs(seekStepMs)
            .build()
        player.addListener(listener)

        playerView = PlayerView(this).apply {
            player = this@VideoPlayerActivity.player
            useController = false
            setShowBuffering(PlayerView.SHOW_BUFFERING_NEVER)
            setKeepContentOnPlayerReset(true)
            keepScreenOn = true
        }
        // The player's own subtitle view sits inside the picture's frame; this one covers the screen.
        playerView.subtitleView?.visibility = View.GONE
        subtitleView = SubtitleView(this)
        secondView = SubtitleView(this)
        second = SecondSubtitles(this, secondView)
        dim = View(this).apply {
            setBackgroundColor(Color.BLACK)
            alpha = 0f
        }
        stats = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
            typeface = Typeface.MONOSPACE
            setBackgroundColor(0x99000000.toInt())
            setPadding(dp(8), dp(6), dp(8), dp(6))
            visibility = View.GONE
        }
        hint = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            gravity = Gravity.CENTER
            background = rounded(0x99000000.toInt(), dp(16).toFloat())
            setPadding(dp(20), dp(12), dp(20), dp(12))
            visibility = View.GONE
        }
        speedBadge = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            background = rounded(0x99000000.toInt(), dp(14).toFloat())
            setPadding(dp(12), dp(5), dp(12), dp(5))
            visibility = View.GONE
        }
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
            background = rounded(Color.WHITE, dp(20).toFloat())
            setOnClickListener { requestNext() }
        }
        nextPrompt = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, dp(24), dp(112))
            addView(pill(label("cancel", "Cancel")) {
                nextCancelled = true
                nextPrompt.visibility = View.GONE
            })
            addView(nextText, LinearLayout.LayoutParams(-2, -2).apply { marginStart = dp(8) })
            visibility = View.GONE
        }
        sheets = PlayerSheets(this)
        tools = PlayerTools(this)
        controls = PlayerControls(this)
        gestures = PlayerGestures(this)

        val brightnessGravity = if (prefs.swapSides) Gravity.END else Gravity.START
        val volumeGravity = if (prefs.swapSides) Gravity.START else Gravity.END
        root = FrameLayout(this).apply {
            addView(playerView, FrameLayout.LayoutParams(-1, -1))
            addView(dim, FrameLayout.LayoutParams(-1, -1))
            addView(subtitleView, FrameLayout.LayoutParams(-1, -1))
            // The second subtitle track, its cues placed at the top.
            addView(secondView, FrameLayout.LayoutParams(-1, -1))
            addView(stats, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.START).apply {
                topMargin = dp(72)
                marginStart = dp(16)
            })
            addView(gestures.brightnessBar.root, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER_VERTICAL or brightnessGravity).apply {
                marginStart = dp(56)
                marginEnd = dp(56)
            })
            addView(gestures.volumeBar.root, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER_VERTICAL or volumeGravity).apply {
                marginStart = dp(56)
                marginEnd = dp(56)
            })
            addView(controls.root, FrameLayout.LayoutParams(-1, -1))
            addView(loading, FrameLayout.LayoutParams(-1, -1))
            addView(hint, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
            addView(speedBadge, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply {
                topMargin = dp(20)
            })
            addView(nextPrompt, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END))
        }
        setContentView(root)
        playerView.setOnTouchListener { view, event -> gestures.onTouch(view, event) }
        // Rotation and picture-in-picture change the height subtitles are placed in.
        subtitleView.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            liftSubtitles(controls.visible && !controls.locked)
        }
        applySubtitleStyle()
        prefs.brightness?.let { setBrightness(it) }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = onBack()
        })
        ContextCompat.registerReceiver(
            this,
            tools.pipReceiver,
            IntentFilter(PlayerTools.ACTION_PIP),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )

        heading = heading(request)
        controls.setTitle(heading)
        controls.setSession(session, several, hasPrevious, hasNext)
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

    /** What every request carries: settings, words, seek step, subtitles, session. */
    private fun readSettings(request: JSONObject) {
        request.optJSONObject("prefs")?.let { prefs = PlayerPrefs(it) }
        request.optJSONObject("labels")?.let { labels = it }
        if (request.has("seekStepMs")) {
            seekStepMs = request.optLong("seekStepMs", DEFAULT_SEEK_STEP_MS).coerceIn(1_000L, 300_000L)
        }
        if (request.has("autoSubtitleLanguage")) {
            autoSubtitleLanguage = if (request.isNull("autoSubtitleLanguage")) {
                null
            } else {
                request.optString("autoSubtitleLanguage").ifBlank { Locale.getDefault().language }
            }
        }
        if (request.has("session")) session = request.optBoolean("session", false)
        if (request.has("autoplay")) autoplay = request.optBoolean("autoplay", true)
        if (request.has("nextPromptMs")) {
            promptMs = request.optLong("nextPromptMs", DEFAULT_PROMPT_MS).coerceIn(0L, 60_000L)
        }
    }

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            if (playbackState == Player.STATE_READY) onReady()
            if (playbackState == Player.STATE_ENDED && !ended) {
                ended = true
                report("ended")
                if (sleepAtEnd) {
                    // The sleep timer was set for this episode's end.
                    sleepAtEnd = false
                    controls.setNote(null)
                    showHint(label("sleepEnded", "Sleep timer ended"))
                } else if (session && hasNext && autoplay && !nextCancelled) {
                    // Autoplay moves on at the end unless the prompt was cancelled.
                    requestNext()
                }
            }
            controls.update()
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (!isPlaying) report("progress")
            controls.update()
            controls.bump()
            updatePip()
        }

        override fun onPlayerError(error: PlaybackException) {
            val rendererFailed = error is ExoPlaybackException &&
                error.type == ExoPlaybackException.TYPE_RENDERER
            if (rendererFailed && !softwareFirst) {
                // Retry once from the same spot with software decoders.
                softwareFirst = true
                reload(player.currentPosition)
                return
            }
            report("error", error.message ?: error.errorCodeName)
            // A session tries its next source; a single video says what went wrong.
            if (session) {
                showLoading(label("tryingNext", "Trying the next source…"))
            } else {
                showHint(label("cantPlay", "Can't play this: {n}", error.errorCodeName), 0)
            }
        }

        override fun onTracksChanged(tracks: Tracks) {
            if (tracks.groups.isEmpty()) return
            if (!tracksPicked) {
                tracksPicked = true
                this@VideoPlayerActivity.tracks.pick(state, prefs, subtitleLanguages())
            }
            // A subtitle file just added from the device plays at once.
            pendingSubtitle?.let { name ->
                val track = this@VideoPlayerActivity.tracks.list(C.TRACK_TYPE_TEXT).firstOrNull { it.format.label == name }
                if (track != null) {
                    pendingSubtitle = null
                    this@VideoPlayerActivity.tracks.select(C.TRACK_TYPE_TEXT, track)
                }
            }
        }

        override fun onAudioSessionIdChanged(audioSessionId: Int) {
            attachLoudness(audioSessionId)
        }

        override fun onCues(cueGroup: CueGroup) {
            subtitleView.setCues(cueGroup.cues)
        }

        override fun onVideoSizeChanged(videoSize: VideoSize) {
            // The frame is measured again for the new size; the chosen aspect must hold for it.
            if (videoSize.width > 0) playerView.post { applyAspect(aspect) }
            if (prefs.orientation == "video" && !manualRotation && videoSize.width > 0 && videoSize.height > 0) {
                requestedOrientation = if (videoSize.width >= videoSize.height) {
                    ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
                } else {
                    ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
                }
            }
            updatePip()
        }
    }

    /** Once a video can play: AniSkip's lookup needs its length. */
    private fun onReady() {
        if (aniskipAsked) return
        val anime = current.optJSONObject("anime") ?: return
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: return
        aniskipAsked = true
        request("duration", JSONObject().put("durationMs", duration).put("anime", anime))
    }

    /** A new request while open (another video was started) replaces what plays. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val request = JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: return)
        report("switched")
        readSettings(request)
        if (!request.has("session")) session = false
        heading = heading(request)
        controls.setTitle(heading)
        controls.setSession(session, several, hasPrevious, hasNext)
        if (request.optString("url").isNotBlank()) load(request) else showLoading(label("findingSource", "Finding a source…"))
    }

    /** An update from Rust (M44, M45). */
    private fun applyUpdate(update: JSONObject) {
        Log.d("RaygunVideo", "update ${update.optString("type")}")
        when (update.optString("type")) {
            "load" -> update.optJSONObject("request")?.let { load(it) }
            // The session's link plays elsewhere: a YouTube video in its embed (M46.35).
            "close" -> {
                report("closed")
                finish()
            }
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
                showHint(update.optString("text"), 0)
                controls.show()
                if (update.optBoolean("showSources")) sheets.sources()
            }
            "episodes" -> {
                episodes = update.optJSONArray("items") ?: JSONArray()
                hasNext = update.optBoolean("hasNext")
                hasPrevious = update.optBoolean("hasPrevious")
                autoplay = update.optBoolean("autoplay", autoplay)
                controls.setSession(session, several, hasPrevious, hasNext)
                updatePip()
            }
            "sources" -> sources = update.optJSONArray("groups") ?: JSONArray()
            "chapters" -> if (update.optString("id") == videoId) {
                // AniSkip's, beside any the source gave.
                val found = Chapter.list(update.optJSONArray("items")).filter { new ->
                    chapters.none { abs(it.startMs - new.startMs) < 1_000 }
                }
                chapters = (chapters + found).sortedBy { it.startMs }
                controls.setChapters(chapters)
            }
        }
    }

    /** Plays a video: url, headers, format, subtitles, startMs, id, title, subtitle, and M45's extras. */
    private fun load(request: JSONObject) {
        if (videoId.isNotEmpty() && videoId != request.optString("id")) {
            report("switched")
            added.clear()
        }
        current = request
        videoId = request.optString("id")
        readSettings(request)
        ended = false
        nextCancelled = false
        nextRequested = false
        nextPrompt.visibility = View.GONE
        heading = heading(request)
        controls.setTitle(heading)
        applySubtitleStyle()

        state = TitleState(request.optJSONObject("titleState"))
        group = request.optString("group").ifBlank { videoId }
        chapters = Chapter.list(request.optJSONArray("chapters"))
        controls.setChapters(chapters)
        countdownChapter = -1
        cancelledCountdowns.clear()
        autoSkipped.clear()
        aniskipAsked = false
        tracksPicked = false
        pendingSubtitle = null
        subtitleView.setCues(null)

        sidecars.clear()
        request.optJSONArray("subtitles")?.let { list ->
            for (index in 0 until list.length()) {
                val track = list.optJSONObject(index) ?: continue
                val url = track.optString("url").takeIf { it.isNotBlank() } ?: continue
                sidecars += Sidecar(
                    Uri.parse(url),
                    subtitleMime(track.optString("format"), url),
                    track.optString("lang").ifBlank { null },
                    track.optString("label").ifBlank { null },
                )
            }
        }
        sidecars += added
        val secondChoice = state.secondary
        val secondTrack = secondChoice?.let { choice ->
            sidecars.firstOrNull { sameLanguage(it.language, choice.language) && it.label == choice.label }
                ?: sidecars.firstOrNull { sameLanguage(it.language, choice.language) }
        }
        if (secondTrack != null) loadSecond(secondTrack) else second.clear()

        val startMs = request.optDouble("startMs", 0.0).toLong().coerceAtLeast(0)
        if (request.optBoolean("external")) {
            launchExternal(request, startMs, only = true)
            return
        }
        tracks.reset(subtitleLanguages(), audioLanguages())
        player.setMediaSource(buildSource(request), startMs)
        player.prepare()
        player.playWhenReady = true
        applySpeed(state.speed ?: prefs.defaultSpeed)
        applyAspect(state.aspect ?: prefs.aspect)
        tracks.text.delayUs = state.subtitleDelayMs * 1000
        gestures.resetBoost()
        hideLoading()
        hideHint()
        controls.show()
        updatePip()
    }

    /** The media source for `request`, with its subtitle files and the ones added here. */
    private fun buildSource(request: JSONObject): MediaSource {
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
        val subtitles = sidecars.map { sidecar ->
            MediaItem.SubtitleConfiguration.Builder(sidecar.uri)
                .setMimeType(sidecar.mime)
                .setLanguage(sidecar.language)
                .setLabel(sidecar.label)
                .build()
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
        return DefaultMediaSourceFactory(DefaultDataSource.Factory(this, http)).createMediaSource(item)
    }

    /** Prepares the same video again from `positionMs` (another decoder, an added subtitle file). */
    private fun reload(positionMs: Long) {
        if (current.optString("url").isBlank()) return
        tracksPicked = false
        player.setMediaSource(buildSource(current), positionMs)
        player.prepare()
        player.play()
    }

    private fun subtitleLanguages(): List<String> =
        autoSubtitleLanguage?.let { listOf(it, prefs.subtitleFallback) }
            ?.filter { it.isNotBlank() }
            ?.distinct()
            ?: emptyList()

    private fun audioLanguages(): List<String> =
        listOf(prefs.audioLanguage, prefs.audioFallback).filter { it.isNotBlank() }.distinct()

    // ---- Speed, aspect, orientation ----

    /** Plays at `speed` without remembering it (holding for speed, a new video). */
    internal fun applySpeed(speed: Float) {
        val pitch = if (prefs.pitchCorrection) 1f else speed
        player.playbackParameters = PlaybackParameters(speed, pitch)
        controls.update()
    }

    /** Plays at `speed` and remembers it for the title. */
    internal fun setSpeed(speed: Float) {
        applySpeed(speed)
        state.speed = speed
        remember(JSONObject().put("speed", (speed * 100).toInt()))
        showHint(formatSpeed(speed))
    }

    /** The speed chip: the next preset up, round to the first. */
    internal fun cycleSpeed() {
        val now = player.playbackParameters.speed
        val presets = prefs.speedPresets.sorted()
        setSpeed(presets.firstOrNull { it > now + 0.001f } ?: presets.first())
        controls.bump()
    }

    private fun applyAspect(mode: String) {
        aspect = mode
        val wanted = when (mode) {
            "crop" -> AspectRatioFrameLayout.RESIZE_MODE_ZOOM
            "stretch" -> AspectRatioFrameLayout.RESIZE_MODE_FILL
            else -> AspectRatioFrameLayout.RESIZE_MODE_FIT
        }
        playerView.resizeMode = wanted
    }

    /** Fit → Crop → Stretch, remembered for the title. */
    internal fun cycleAspect() {
        val next = when (aspect) {
            "fit" -> "crop"
            "crop" -> "stretch"
            else -> "fit"
        }
        applyAspect(next)
        state.aspect = next
        remember(JSONObject().put("aspect", next))
        showHint(
            when (next) {
                "crop" -> label("aspectCrop", "Crop")
                "stretch" -> label("aspectStretch", "Stretch")
                else -> label("aspectFit", "Fit")
            },
        )
        controls.bump()
    }

    private fun applyOrientation() {
        requestedOrientation = when (prefs.orientation) {
            "free" -> ActivityInfo.SCREEN_ORIENTATION_SENSOR
            "portrait" -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            "reversePortrait" -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
            "sensorPortrait" -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
            "landscape" -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            "reverseLandscape" -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
            // "video" follows the video once its size is known.
            else -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
    }

    /** The rotate button: portrait ⇄ landscape for this viewing. */
    internal fun rotate() {
        manualRotation = true
        val landscape = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        requestedOrientation = if (landscape) {
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
        } else {
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
        controls.bump()
    }

    internal fun toggleRemaining() {
        prefs.showRemaining = !prefs.showRemaining
        savePrefs(JSONObject().put("showRemaining", prefs.showRemaining))
        controls.update()
        controls.bump()
    }

    // ---- Playing ----

    internal fun togglePlay() {
        when {
            player.playbackState == Player.STATE_ENDED -> {
                player.seekTo(0)
                player.play()
            }
            player.playWhenReady -> player.pause()
            else -> player.play()
        }
        controls.update()
    }

    internal fun seekBy(deltaMs: Long) {
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 }
        val target = (player.currentPosition + deltaMs).coerceAtLeast(0)
        player.seekTo(if (duration != null) target.coerceAtMost(duration) else target)
    }

    internal fun setBrightness(level: Float) {
        brightness = level.coerceIn(-0.75f, 1f)
        val attributes = window.attributes
        attributes.screenBrightness = if (brightness <= 0f) 0.01f else brightness.coerceIn(0.01f, 1f)
        window.attributes = attributes
        dim.alpha = if (brightness < 0f) -brightness else 0f
    }

    private fun systemBrightness(): Float =
        Settings.System.getInt(contentResolver, Settings.System.SCREEN_BRIGHTNESS, 128) / 255f

    /** Volume past 100 %: `boost` 0.3 adds 30 %. */
    internal fun setBoost(boost: Float) {
        val enhancer = loudness ?: return
        runCatching {
            enhancer.setTargetGain((2000 * log10(1.0 + boost)).toInt())
            enhancer.setEnabled(boost > 0f)
        }
    }

    private fun attachLoudness(audioSessionId: Int) {
        loudness?.release()
        loudness = null
        // 0 is the device's whole output, not this player's.
        if (audioSessionId == 0 || audioSessionId == C.AUDIO_SESSION_ID_UNSET) return
        loudness = runCatching { LoudnessEnhancer(audioSessionId) }.getOrNull()
        setBoost(gestures.boost)
    }

    // ---- Chapters and the skip button ----

    /** The chapter playing at `positionMs`, with its index; none past a chapter's end. */
    internal fun chapterAt(positionMs: Long): Pair<Int, Chapter>? {
        val index = chapters.indexOfLast { it.startMs <= positionMs }
        if (index < 0) return null
        val chapter = chapters[index]
        return if (chapter.endMs != null && positionMs >= chapter.endMs) null else index to chapter
    }

    private fun chapterEnd(index: Int): Long =
        chapters[index].endMs
            ?: chapters.getOrNull(index + 1)?.startMs
            ?: player.duration.coerceAtLeast(0)

    internal fun chapterName(chapter: Chapter, index: Int): String = chapter.name ?: when (chapter.kind) {
        "opening" -> label("opening", "Opening")
        "ending" -> label("ending", "Ending")
        "recap" -> label("recap", "Recap")
        else -> label("chapter", "Chapter {n}", (index + 1).toString())
    }

    /** The title's own intro length, else the setting's. */
    internal fun introMs(): Long = state.introMs ?: prefs.introMs

    internal fun setIntroLength(ms: Long?) {
        state.introMs = ms
        remember(JSONObject().put("introMs", ms ?: JSONObject.NULL))
    }

    private fun updateSkip() {
        val button = controls.skipButton
        val dontSkip = controls.dontSkipButton
        val position = player.currentPosition
        val found = chapterAt(position)
        if (found != null && found.second.skippable && player.duration > 0 && !controls.locked) {
            val (index, chapter) = found
            if (chapterEnd(index) > position + 1_000) {
                when {
                    prefs.skipCountdown && index !in cancelledCountdowns -> {
                        val now = SystemClock.elapsedRealtime()
                        if (countdownChapter != index) {
                            countdownChapter = index
                            countdownEndsAt = now + prefs.skipWaitMs
                        }
                        // The countdown waits while paused.
                        if (!player.isPlaying) countdownEndsAt += TICK_MS
                        val left = countdownEndsAt - now
                        if (left <= 0) {
                            skipChapter(index)
                            return
                        }
                        button.text = label("skippingIn", "Skipping in {n} s", ((left + 999) / 1000).toString())
                        button.visibility = View.VISIBLE
                        dontSkip.visibility = View.VISIBLE
                        return
                    }
                    prefs.autoSkip && index !in autoSkipped -> {
                        autoSkipped += index
                        skipChapter(index)
                        return
                    }
                    prefs.skipButton -> {
                        button.text = when (chapter.kind) {
                            "opening" -> label("skipOpening", "Skip opening")
                            "ending" -> label("skipEnding", "Skip ending")
                            else -> label("skipRecap", "Skip recap")
                        }
                        button.visibility = View.VISIBLE
                        dontSkip.visibility = View.GONE
                        return
                    }
                }
            }
        }
        countdownChapter = -1
        dontSkip.visibility = View.GONE
        val intro = introMs()
        if (prefs.skipButton && intro > 0 && controls.visible && !controls.locked && player.duration > 0) {
            button.text = label("skipBy", "+{n} s", (intro / 1000).toString())
            button.visibility = View.VISIBLE
        } else {
            button.visibility = View.GONE
        }
    }

    private fun skipChapter(index: Int) {
        player.seekTo(chapterEnd(index))
        countdownChapter = -1
        showHint(label("skipped", "Skipped"))
    }

    internal fun onSkipTapped() {
        val found = chapterAt(player.currentPosition)
        if (found != null && found.second.skippable && chapterEnd(found.first) > player.currentPosition) {
            skipChapter(found.first)
        } else {
            seekBy(introMs())
        }
        controls.bump()
    }

    internal fun cancelSkipCountdown() {
        if (countdownChapter >= 0) cancelledCountdowns += countdownChapter
        countdownChapter = -1
        controls.dontSkipButton.visibility = View.GONE
    }

    // ---- Subtitles ----

    internal fun applySubtitleStyle() {
        if (!::subtitleView.isInitialized) return
        val look = prefs.style
        val family = when (look.font) {
            "sans" -> Typeface.SANS_SERIF
            "serif" -> Typeface.SERIF
            "mono" -> Typeface.MONOSPACE
            else -> Typeface.DEFAULT
        }
        val styleFlags = (if (look.bold) Typeface.BOLD else 0) or (if (look.italic) Typeface.ITALIC else 0)
        val edge = when (look.edge) {
            "none" -> CaptionStyleCompat.EDGE_TYPE_NONE
            "shadow" -> CaptionStyleCompat.EDGE_TYPE_DROP_SHADOW
            "raised" -> CaptionStyleCompat.EDGE_TYPE_RAISED
            "depressed" -> CaptionStyleCompat.EDGE_TYPE_DEPRESSED
            else -> CaptionStyleCompat.EDGE_TYPE_OUTLINE
        }
        val caption = CaptionStyleCompat(
            look.textColor,
            look.backgroundColor,
            Color.TRANSPARENT,
            edge,
            look.edgeColor,
            Typeface.create(family, styleFlags),
        )
        listOf(subtitleView, secondView).forEach { view ->
            view.setStyle(caption)
            view.setFractionalTextSize(SubtitleView.DEFAULT_TEXT_SIZE_FRACTION * look.size / 100f)
            view.setApplyEmbeddedStyles(!look.ignoreEmbedded)
            view.setApplyEmbeddedFontSizes(!look.ignoreEmbedded)
        }
        liftSubtitles(::controls.isInitialized && controls.visible && !controls.locked)
    }

    /**
     * Places subtitles at the chosen height, and above the bottom bar while the controls show.
     * Media3 puts many cues (WebVTT's among them) on the last line, out of reach of its bottom
     * padding fraction, so the view's own padding moves them instead.
     */
    internal fun liftSubtitles(controlsShown: Boolean) {
        if (!::subtitleView.isInitialized || !::root.isInitialized) return
        val height = subtitleView.height.takeIf { it > 0 } ?: root.height
        val fraction = prefs.style.position / 100f
        val lift = if (controlsShown) maxOf(fraction, 0.24f) else fraction
        subtitleView.setBottomPaddingFraction(0f)
        subtitleView.setPadding(0, 0, 0, (height * lift).toInt())
    }

    /** A style change made in the player: shown at once and saved to Settings. */
    internal fun updateStyle(look: SubtitleLook) {
        prefs.style = look
        applySubtitleStyle()
        savePrefs(JSONObject().put("subtitleStyle", look.toJson()))
    }

    /** Positive delays show subtitles later. Changing it re-reads cues from the same spot. */
    internal fun setSubtitleDelay(ms: Long) {
        val delay = ms.coerceIn(-600_000L, 600_000L)
        state.subtitleDelayMs = delay
        tracks.text.delayUs = delay * 1000
        remember(JSONObject().put("subtitleDelayMs", delay))
        main.removeCallbacks(resyncSubtitles)
        main.postDelayed(resyncSubtitles, 700)
    }

    internal fun loadSecond(sidecar: Sidecar) {
        second.load(sidecar) { problem -> showHint(problem) }
    }

    internal fun pickSubtitleFile() {
        try {
            subtitleFile.launch(arrayOf("*/*"))
        } catch (_: ActivityNotFoundException) {
            showHint(label("noOtherApp", "No other app can play this video"))
        }
    }

    /** A subtitle file chosen on the device joins the video's tracks and plays. */
    private fun addSubtitleFile(uri: Uri) {
        val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: uri.lastPathSegment ?: "subtitles"
        val sidecar = Sidecar(uri, subtitleMime("", name), null, name)
        added += sidecar
        sidecars += sidecar
        pendingSubtitle = name
        reload(player.currentPosition)
    }

    // ---- Tools ----

    internal fun openElsewhere() = launchExternal(current, player.currentPosition, only = false)

    /** Hands the video to another app; `only` closes the player when it comes back. */
    private fun launchExternal(request: JSONObject, startMs: Long, only: Boolean) {
        if (request.optString("url").isBlank()) return
        val intent = tools.externalIntent(request, prefs.externalPackage, startMs)
        val launch = if (prefs.externalPackage.isBlank()) {
            Intent.createChooser(intent, label("openElsewhere", "Open in another app"))
        } else {
            intent
        }
        try {
            externalOnly = only
            if (only) showLoading(label("openElsewhere", "Open in another app")) else player.pause()
            externalApp.launch(launch)
        } catch (_: ActivityNotFoundException) {
            externalOnly = false
            showHint(label("noOtherApp", "No other app can play this video"), 0)
            if (only) {
                // Raygun's own player instead.
                load(JSONObject(request.toString()).put("external", false))
            }
        }
    }

    private fun onExternalResult(resultCode: Int, data: Intent?) {
        val result = tools.externalResult(resultCode, data)
        if (result != null && videoId.isNotEmpty()) {
            val json = JSONObject()
                .put("id", videoId)
                .put("state", if (result.finished) "ended" else "progress")
                .put("positionMs", result.positionMs ?: 0L)
                .put("durationMs", result.durationMs ?: JSONObject.NULL)
                .put("error", JSONObject.NULL)
            NativeBridge.video(json.toString())
        }
        if (externalOnly) {
            externalOnly = false
            finish()
        } else {
            result?.positionMs?.let { player.seekTo(it) }
        }
    }

    internal fun setDecoder(mode: String) {
        prefs.decoder = mode
        softwareFirst = mode == "software"
        savePrefs(JSONObject().put("decoder", mode))
        reload(player.currentPosition)
        showHint(
            if (softwareFirst) label("decoderSoftware", "Software first") else label("decoderAuto", "Hardware, then software"),
        )
    }

    internal fun toggleStats() {
        val helper = statsHelper
        if (helper == null) {
            stats.visibility = View.VISIBLE
            statsHelper = DebugTextViewHelper(player, stats).also { it.start() }
        } else {
            helper.stop()
            statsHelper = null
            stats.visibility = View.GONE
        }
    }

    /** The sleep timer: after `durationMs`, at this episode's end, or off. */
    internal fun setSleep(durationMs: Long?, atEnd: Boolean) {
        sleepAt = durationMs?.let { SystemClock.elapsedRealtime() + it } ?: 0L
        sleepAtEnd = atEnd
        updateSleep()
    }

    internal fun sleepSet(): Boolean = sleepAt > 0 || sleepAtEnd

    internal fun sleepNote(): String? = when {
        sleepAt > 0 -> label("sleepLeft", "Sleep timer: {n} left", formatTime(sleepAt - SystemClock.elapsedRealtime()))
        sleepAtEnd -> label("sleepEndOfEpisode", "End of this episode")
        else -> null
    }

    private fun updateSleep() {
        if (sleepAt > 0 && SystemClock.elapsedRealtime() >= sleepAt) {
            sleepAt = 0
            player.pause()
            showHint(label("sleepEnded", "Sleep timer ended"), 0)
        }
        controls.setNote(sleepNote())
    }

    // ---- Picture-in-picture ----

    internal fun enterPip() {
        closeSheet(openSheet)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            enterPictureInPictureMode(tools.pipParams())
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            @Suppress("DEPRECATION")
            enterPictureInPictureMode()
        }
    }

    /** Keeps the window's buttons (and auto-enter on Android 12+) in step with playback. */
    internal fun updatePip() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O || !::tools.isInitialized) return
        val inPip = isInPictureInPictureMode
        if (inPip || Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            runCatching { setPictureInPictureParams(tools.pipParams()) }
        }
    }

    /** Leaving the app while a video plays continues it in a small window (Settings → Player). */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        // Android 12+ enters by itself (auto-enter); earlier versions are asked here.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S && prefs.pipOnLeave && player.isPlaying) enterPip()
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        if (isInPictureInPictureMode) {
            closeSheet(openSheet)
            controls.hide()
            hint.visibility = View.GONE
        }
        stats.visibility = if (statsHelper != null && !isInPictureInPictureMode) View.VISIBLE else View.GONE
        liftSubtitles(false)
    }

    // ---- Back, keys, sheets ----

    private fun onBack() {
        when {
            openSheet != null -> closeSheet(openSheet)
            controls.locked -> controls.show()
            prefs.pipOnBack && player.isPlaying -> enterPip()
            else -> finish()
        }
    }

    /** The top bar's back button. */
    internal fun leave() = finish()

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (openSheet != null) return super.onKeyDown(keyCode, event)
        val episodeKeys = prefs.mediaKeys == "switchEpisode" && session && several
        when (keyCode) {
            KeyEvent.KEYCODE_MEDIA_NEXT -> if (episodeKeys) requestNext() else seekBy(seekStepMs)
            KeyEvent.KEYCODE_MEDIA_PREVIOUS -> if (episodeKeys) request("previous") else seekBy(-seekStepMs)
            KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> seekBy(seekStepMs)
            KeyEvent.KEYCODE_MEDIA_REWIND -> seekBy(-seekStepMs)
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK, KeyEvent.KEYCODE_SPACE -> togglePlay()
            KeyEvent.KEYCODE_MEDIA_PLAY -> player.play()
            KeyEvent.KEYCODE_MEDIA_PAUSE -> player.pause()
            KeyEvent.KEYCODE_DPAD_LEFT -> if (controls.visible || controls.locked) return super.onKeyDown(keyCode, event) else seekBy(-seekStepMs)
            KeyEvent.KEYCODE_DPAD_RIGHT -> if (controls.visible || controls.locked) return super.onKeyDown(keyCode, event) else seekBy(seekStepMs)
            KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER ->
                if (controls.visible) return super.onKeyDown(keyCode, event) else controls.show()
            else -> return super.onKeyDown(keyCode, event)
        }
        return true
    }

    internal fun showSheet(sheet: PlayerSheet) {
        openSheet?.let { root.removeView(it.root) }
        openSheet = sheet
        root.addView(sheet.root, FrameLayout.LayoutParams(-1, -1))
        controls.bump()
    }

    internal fun closeSheet(sheet: PlayerSheet?) {
        if (sheet == null || openSheet !== sheet) return
        root.removeView(sheet.root)
        openSheet = null
        sheet.dismissed()
    }

    // ---- Session requests (M44) ----

    internal fun requestNext() {
        if (nextRequested || !hasNext) return
        nextRequested = true
        nextPrompt.visibility = View.GONE
        request("next")
    }

    /** Near the end of an episode with a next one: a "Next episode" button, counting down with autoplay. */
    private fun updateNextPrompt() {
        if (!session || !hasNext || nextCancelled || nextRequested) {
            nextPrompt.visibility = View.GONE
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
            label("nextIn", "Next episode in {n} s", seconds.toString())
        } else {
            label("nextEpisode", "Next episode")
        }
        nextPrompt.visibility = View.VISIBLE
    }

    private fun tick() {
        sinceReport += TICK_MS
        if (sinceReport >= REPORT_INTERVAL_MS) {
            sinceReport = 0
            if (player.isPlaying) report("progress")
        }
        updateNextPrompt()
        if (controls.visible) controls.update()
        updateSkip()
        updateSleep()
        if (second.active) second.update(player.currentPosition, state.subtitleDelayMs)
    }

    // ---- Messages ----

    internal fun showLoading(text: String) {
        loadingText.text = text
        loading.visibility = View.VISIBLE
        controls.showTopOnly()
    }

    private fun hideLoading() {
        loading.visibility = View.GONE
    }

    internal fun isLoading(): Boolean = ::loading.isInitialized && loading.visibility == View.VISIBLE

    /** A centred message; `forMs` 0 keeps it until [hideHint]. */
    internal fun showHint(text: String, forMs: Long = 1_500) {
        hint.text = text
        hint.visibility = View.VISIBLE
        main.removeCallbacks(hideHintLater)
        if (forMs > 0) main.postDelayed(hideHintLater, forMs)
    }

    internal fun hideHint() {
        main.removeCallbacks(hideHintLater)
        hint.visibility = View.GONE
    }

    /** The held speed, small and out of the way at the top; null hides it. */
    internal fun showSpeedBadge(text: String?) {
        speedBadge.text = text ?: ""
        speedBadge.visibility = if (text == null) View.GONE else View.VISIBLE
    }

    private fun heading(request: JSONObject): String {
        val name = request.optString("title")
        val sub = request.optString("subtitle")
        return if (sub.isBlank()) name else "$name\n$sub"
    }

    internal fun headingText(): String = heading

    /** A word in the app's language (`videoPlayer.*`), `{n}` filled with `value`. */
    internal fun label(key: String, fallback: String, value: String? = null): String {
        val text = labels.optString(key).ifBlank { fallback }
        return if (value == null) text else text.replace("{n}", value)
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

    override fun onStop() {
        super.onStop()
        val inPip = Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && isInPictureInPictureMode
        if (!inPip && !externalOnly) player.pause()
        report("progress")
        if (prefs.rememberBrightness && !brightness.isNaN()) {
            request("brightness", JSONObject().put("value", brightness.coerceIn(0f, 1f).toDouble()))
        }
    }

    override fun onDestroy() {
        if (live === this) live = null
        main.removeCallbacksAndMessages(null)
        runCatching { unregisterReceiver(tools.pipReceiver) }
        statsHelper?.stop()
        loudness?.release()
        report("closed")
        player.release()
        super.onDestroy()
    }

    /** Remembers something for the title (speed, aspect, tracks, delay, intro length). */
    internal fun remember(change: JSONObject) {
        request("state", JSONObject().put("group", group).put("change", change))
    }

    /** Saves a setting changed in the player to Settings → Player. */
    internal fun savePrefs(change: JSONObject) {
        request("prefs", JSONObject().put("change", change))
    }

    /** Asks Rust for something: the next or previous episode, an episode, a source, a save. */
    internal fun request(action: String, extra: JSONObject = JSONObject()) {
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

    private fun streamMime(format: String): String? = when (format.lowercase(Locale.ROOT)) {
        "hls", "m3u8" -> MimeTypes.APPLICATION_M3U8
        "dash", "mpd" -> MimeTypes.APPLICATION_MPD
        else -> null
    }
}

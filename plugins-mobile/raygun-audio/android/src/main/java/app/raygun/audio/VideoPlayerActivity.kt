package app.raygun.audio

import android.Manifest
import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.icu.text.PluralRules
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
import android.view.Surface
import android.view.View
import android.view.WindowManager
import android.view.animation.DecelerateInterpolator
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.common.text.CueGroup
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.upstream.DefaultBandwidthMeter
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
import kotlin.concurrent.thread
import kotlin.math.abs
import kotlin.math.log10

private const val REPORT_INTERVAL_MS = 5_000L
private const val TICK_MS = 250L
private const val DEFAULT_SEEK_STEP_MS = 10_000L
private const val DEFAULT_PROMPT_MS = 10_000L
private const val DEFAULT_USER_AGENT =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36"
/** What Media3 buffers before it starts a video, and before it goes on after running dry. */
private const val START_BUFFER_MS = 1_000L
private const val REBUFFER_MS = 2_000L
/** A seek just before buffering means starting, not running dry. */
private const val SEEK_GRACE_MS = 600L
/** An ending that reaches this close to the end is the credits: the next-episode prompt shows. */
private const val CREDITS_SLACK_MS = 5_000L
/** How long subtitles take to slide out of the controls' way and back. */
private const val SUBTITLE_SLIDE_MS = 200L

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
 * `prefs` requests. Tracks are chosen by the viewer's languages ([LanguagePrefs]); waiting shows
 * [PlayerLoading] and failures [PlayerProblem].
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
            activity.runOnUiThread {
                // One bad update must not end Raygun (an exception here would, on the main thread).
                runCatching { activity.applyUpdate(JSONObject(json)) }
                    .onFailure { error -> android.util.Log.w("Raygun", "video update failed", error) }
            }
            return true
        }

        private val HEX_COLOR = Regex("^#[0-9a-fA-F]{6}$")
        private val AUDIO_ATTRIBUTES: AudioAttributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
            .build()
    }

    internal lateinit var player: ExoPlayer
    internal lateinit var playerView: PlayerView
    internal lateinit var controls: PlayerControls
    internal lateinit var gestures: PlayerGestures
    internal lateinit var tracks: PlayerTracks
    internal lateinit var sheets: PlayerSheets
    internal lateinit var tools: PlayerTools
    /** Play on TV: the devices sheet and the TV's remote. */
    internal lateinit var cast: PlayerCast
    /** The picture's effects and smart audio ([PlayerEffects]). */
    internal lateinit var effects: PlayerEffects
    internal lateinit var second: SecondSubtitles
    /** AI subtitles and translations made as the video plays. */
    internal lateinit var liveTracks: LiveSubtitles
    /** Live streams: their clock, the LIVE badge, holding the picture behind for AI subtitles. */
    internal lateinit var liveStream: PlayerLive
    /** The Quality button's sheet. */
    internal lateinit var quality: PlayerQuality
    /** Sources: clean names with their options, or the raw list. */
    internal lateinit var sourcesSheet: PlayerSourcesSheet
    /** Audio: the video's tracks, another source's sound beside the picture, the audio delay. */
    internal lateinit var audio: PlayerAudio
    /** Subtitles over the part of the picture on screen (crop makes the picture larger). */
    internal lateinit var subtitleView: SubtitleView
    private lateinit var secondView: SubtitleView
    /** What each subtitle view shows, in the viewer's style. */
    internal lateinit var mainCues: StyledCues
    private lateinit var secondCues: StyledCues
    /** Where the subtitle views slide to while the controls show (px; 0 when hidden). */
    private var mainShift = 0f
    private var secondShift = 0f
    /** The shade behind the controls, over the whole window. */
    private lateinit var vignette: PlayerVignette
    private lateinit var root: FrameLayout
    private lateinit var hint: TextView
    /** Double taps' seek, on the side tapped. */
    private lateinit var seekBubble: TextView
    /** A small pill at the top while the picture is held for speed. */
    private lateinit var speedBadge: TextView
    private lateinit var loading: PlayerLoading
    private lateinit var problem: PlayerProblem
    private lateinit var nextPrompt: LinearLayout
    private lateinit var nextText: TextView
    private lateinit var dim: View
    private lateinit var stats: TextView
    private var statsHelper: DebugTextViewHelper? = null
    private var openSheet: PlayerSheet? = null
    private lateinit var systemBars: PlayerSystemBars
    private var loudness: LoudnessEnhancer? = null

    internal val main = Handler(Looper.getMainLooper())
    internal var prefs = PlayerPrefs(JSONObject())
    private var labels = JSONObject()
    /** How far a double tap, the keys and the skip buttons jump (Settings → Player). */
    internal var seekStepMs = DEFAULT_SEEK_STEP_MS
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
    /** The rotate button was held: the screen stays exactly as it was then. */
    private var rotationKept = false
    private var orientationBeforeKeeping = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    private var aspect = "fit"
    private var pendingSubtitle: String? = null
    private var externalOnly = false

    // Languages (the app's, the subtitles' and when they turn on) and the source's User-Agent.
    private var appLanguage = ""
    private var subtitleLanguage = ""
    private var subtitleMode = "foreign"
    private var spokenLanguage: String? = null
    internal var languages = LanguagePrefs(emptyList(), emptyList(), "foreign", null)
    private var userAgent = DEFAULT_USER_AGENT
    /** Music may play on beside the video: the player then leaves the audio focus alone. */
    private var keepMusicPlaying = false

    // Waiting: whether this video has shown a frame, and why it buffers.
    private var started = false
    private var seekedAt = 0L
    private var rebuffering = false
    /** The next video is one picked in Sources, not the next one tried. */
    private var pickedSource = false

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
    /**
     * Loop this video (⋮ More): Media3 repeats the item with the same decoders and buffers, and
     * the next round is buffered ahead from memory or [VideoCache]. Kept for this player only.
     */
    internal var looping = false
    internal var episodes = JSONArray()
    internal var sources = JSONArray()
    /** The order Sources lists them in by clean name (Rust's `order`). */
    internal var sourceOrder: JSONArray? = null
    /** The source playing, once it has played a while. */
    internal var playingFrom: String? = null
    /** A link played on its own (Rust's `link`): its own links play first. */
    internal var linkSession = false
    /** It has nothing else to switch to (Rust's `alone`): Sources says "Only this link". */
    internal var sourcesAlone = false
    /** Another link would play if this one failed (Rust's `fallback`). */
    private var sourcesFallback = true
    /** How the torrent playing is doing (Rust's `transfer` updates): peers and speed, for the ring's note. */
    internal var transferNote: String? = null
        private set
    /** The media source reads through [VideoCache]; a live stream must not (its playlists change). */
    private var cachedSource = false
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
    private val hideSeekBubble = Runnable { seekBubble.visibility = View.GONE }
    private val resyncSubtitles = Runnable { player.seekTo(player.currentPosition) }
    private var sinceReport = 0L
    private val ticker = object : Runnable {
        override fun run() {
            // A tick that throws must not end Raygun; the next one comes as usual.
            runCatching { tick() }
                .onFailure { error -> android.util.Log.w("Raygun", "video tick failed", error) }
            main.postDelayed(this, TICK_MS)
        }
    }

    private val subtitleFile = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let { addSubtitleFile(it) }
    }
    private val audioFile = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let { addAudioFile(it) }
    }
    private val externalApp = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        onExternalResult(result.resultCode, result.data)
    }
    private val localNetwork = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        cast.permissionAnswered(granted)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        CrashLog.install(this)
        // Recreated by the system in a new process (after Raygun ended): nothing in Rust knows this
        // player, so it would wait for ever; Raygun's own screen comes instead.
        if (savedInstanceState != null && DirectCalls.plugin == null) {
            finish()
            return
        }
        live = this
        // Language names for choosing tracks, ready before the first video's are known.
        thread(name = "raygun-languages") { Languages.warm() }
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
        systemBars = PlayerSystemBars(window)
        systemBars.apply(false)

        tracks = PlayerTracks(this)
        softwareFirst = prefs.decoder == "software"
        effects = PlayerEffects(this)
        // A hardware decoder that fails (some phones, the emulator at 1080p) hands over to the
        // next one, ending with the software decoders.
        val renderers = PlayerRenderers(this, effects.audio, effects.graph) { tracks.text = it }
            .setEnableDecoderFallback(true)
            .setMediaCodecSelector(codecSelector)
        player = ExoPlayer.Builder(this, renderers)
            .setSeekBackIncrementMs(seekStepMs)
            .setSeekForwardIncrementMs(seekStepMs)
            // Calls and other apps pause the video, unplugged headphones too.
            .setAudioAttributes(AUDIO_ATTRIBUTES, !keepMusicPlaying)
            .setHandleAudioBecomingNoisy(true)
            .build()
        player.addListener(listener)

        playerView = PlayerView(this).apply {
            player = this@VideoPlayerActivity.player
            useController = false
            setShowBuffering(PlayerView.SHOW_BUFFERING_NEVER)
            setKeepContentOnPlayerReset(true)
            keepScreenOn = true
        }
        // The player's own subtitle view sits inside the picture's frame; these follow the part
        // of the frame on screen.
        playerView.subtitleView?.visibility = View.GONE
        subtitleView = SubtitleView(this)
        secondView = SubtitleView(this)
        mainCues = StyledCues(subtitleView)
        secondCues = StyledCues(secondView)
        second = SecondSubtitles(this, secondCues)
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
        seekBubble = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            background = rounded(0x80000000.toInt(), dp(28).toFloat())
            setPadding(dp(18), dp(10), dp(18), dp(10))
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
        nextText = TextView(this).apply {
            setTextColor(Color.BLACK)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            typeface = Typeface.DEFAULT_BOLD
            setPadding(dp(18), dp(10), dp(18), dp(10))
            background = rounded(Color.WHITE, dp(20).toFloat())
            setOnClickListener { requestNext() }
        }
        nextPrompt = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(pill(label("cancel", "Cancel")) {
                nextCancelled = true
                nextPrompt.visibility = View.GONE
            })
            addView(nextText, LinearLayout.LayoutParams(-2, -2).apply { marginStart = dp(8) })
            visibility = View.GONE
        }
        loading = PlayerLoading(this)
        sheets = PlayerSheets(this)
        liveTracks = LiveSubtitles(this)
        tools = PlayerTools(this)
        problem = PlayerProblem(this)
        liveStream = PlayerLive(this)
        quality = PlayerQuality(this)
        sourcesSheet = PlayerSourcesSheet(this)
        audio = PlayerAudio(this)
        controls = PlayerControls(this)
        gestures = PlayerGestures(this)
        cast = PlayerCast(this)

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
            // Under the controls: their buttons stay clear while the ring waits in the middle.
            addView(loading.root, FrameLayout.LayoutParams(-1, -1))
            addView(controls.root, FrameLayout.LayoutParams(-1, -1))
            // A TV's remote covers the player while it plays there.
            addView(cast.root, FrameLayout.LayoutParams(-1, -1))
            addView(problem.root, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
            addView(hint, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
            addView(seekBubble, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER_VERTICAL or Gravity.START))
            addView(speedBadge, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply {
                topMargin = dp(20)
            })
            addView(nextPrompt, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END).apply {
                marginEnd = dp(24)
                bottomMargin = dp(32)
            })
            // The offer to hold a live stream behind its edge for AI subtitles, where the
            // next-episode prompt would be.
            addView(liveStream.prompt, FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END).apply {
                marginEnd = dp(24)
                bottomMargin = dp(32)
            })
        }
        setContentView(root)
        fitControls(root, controls.root)
        playerView.setOnTouchListener { view, event -> gestures.onTouch(view, event) }
        // Rotation, the aspect and picture-in-picture move the picture: the subtitles follow it.
        playerView.findViewById<View>(androidx.media3.ui.R.id.exo_content_frame)
            ?.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> root.post { placeSubtitles() } }
        root.addOnLayoutChangeListener { _, left, _, right, _, _, _, _, _ ->
            loading.fitTo(right - left)
            problem.fitTo(right - left)
            root.post { placeSubtitles() }
        }
        subtitleView.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            liftSubtitles(controls.visible && !controls.locked)
        }
        applySubtitleStyle()
        installVignette()
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
        // A TV is connected: what opens plays there.
        request.optJSONObject("cast")?.let { cast.show(it) }
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

    /** What every request carries: settings, words, seek step, languages, session. */
    private fun readSettings(request: JSONObject) {
        request.optJSONObject("prefs")?.let { prefs = PlayerPrefs(it) }
        request.optJSONObject("labels")?.let { labels = it }
        if (request.has("seekStepMs")) {
            seekStepMs = request.optLong("seekStepMs", DEFAULT_SEEK_STEP_MS).coerceIn(1_000L, 300_000L)
        }
        if (request.has("appLanguage")) appLanguage = request.optString("appLanguage")
        if (request.has("subtitleLanguage")) subtitleLanguage = request.optString("subtitleLanguage")
        if (request.has("subtitleMode")) subtitleMode = request.optString("subtitleMode", "foreign")
        request.optString("accent").takeIf { HEX_COLOR.matches(it) }?.let { playerAccent = Color.parseColor(it) }
        request.optString("userAgent").takeIf { it.isNotBlank() }?.let { userAgent = it }
        if (request.has("keepMusicPlaying")) {
            val keep = request.optBoolean("keepMusicPlaying", false)
            if (keep != keepMusicPlaying && ::player.isInitialized) player.setAudioAttributes(AUDIO_ATTRIBUTES, !keep)
            keepMusicPlaying = keep
        }
        if (request.has("session")) session = request.optBoolean("session", false)
        if (request.has("autoplay")) autoplay = request.optBoolean("autoplay", true)
        if (request.has("nextPromptMs")) {
            promptMs = request.optLong("nextPromptMs", DEFAULT_PROMPT_MS).coerceIn(0L, 60_000L)
        }
    }

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            when (playbackState) {
                Player.STATE_READY -> {
                    rebuffering = false
                    // A stream that turned out live starts at its live edge, not at a place asked for.
                    liveStream.onReady()
                    onReady()
                }
                // Running dry while playing, not starting or seeking: Media3 wants more first.
                Player.STATE_BUFFERING ->
                    rebuffering = started && SystemClock.elapsedRealtime() - seekedAt > SEEK_GRACE_MS
            }
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
                } else {
                    controls.show()
                }
            }
            updateWaiting()
            controls.update()
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (!isPlaying) report("progress")
            updateWaiting()
            controls.update()
            controls.bump()
            updatePip()
        }

        override fun onPositionDiscontinuity(
            oldPosition: Player.PositionInfo,
            newPosition: Player.PositionInfo,
            reason: Int,
        ) {
            if (reason == Player.DISCONTINUITY_REASON_SEEK) seekedAt = SystemClock.elapsedRealtime()
            if (reason == Player.DISCONTINUITY_REASON_AUTO_TRANSITION && looping) wrapped()
        }

        override fun onRenderedFirstFrame() {
            started = true
            pickedSource = false
            updateWaiting()
        }

        override fun onTimelineChanged(timeline: Timeline, reason: Int) {
            // A live stream read through the cache would get its first playlist again and
            // again: it's loaded anew without it, from its live edge.
            if (cachedSource && player.isCurrentMediaItemLive && current.optString("url").isNotBlank()) {
                cachedSource = false
                Log.i("RaygunVideo", "live stream: reloading without the cache")
                player.setMediaSource(buildSource(current, cached = false))
                player.prepare()
            }
            if (::controls.isInitialized) controls.update()
        }

        override fun onPlayerError(error: PlaybackException) {
            // The picture effects' pipeline failed: the video goes on without them.
            if (effects.onPlayerError(error)) return
            // A live stream fell behind its window: back to its live edge.
            if (error.errorCode == PlaybackException.ERROR_CODE_BEHIND_LIVE_WINDOW) {
                player.seekToDefaultPosition()
                player.prepare()
                return
            }
            // Another source's sound in the source: it goes, and the video plays on with its own
            // (should the video fail again, that's the video's own failure).
            if (audio.dropAfterError()) {
                Log.w("RaygunVideo", "the sound from another source failed: ${error.errorCodeName}", error)
                return
            }
            val rendererFailed = error is ExoPlaybackException &&
                error.type == ExoPlaybackException.TYPE_RENDERER
            if (rendererFailed && !softwareFirst) {
                // Retry once from the same spot with software decoders.
                softwareFirst = true
                reload(player.currentPosition)
                return
            }
            val (message, detail) = describeProblem(error) { key, fallback -> label(key, fallback) }
            Log.w("RaygunVideo", "playback failed: $detail")
            report("error", detail)
            // A session tries its next source; a single video, or a link played on its own with
            // nothing else to switch to, says what went wrong.
            if (session && !(linkSession && !sourcesFallback)) {
                showLoading(label("tryingNext", "That source failed. Trying the next one…"), message)
            } else {
                showProblem(message, detail, retry = { reload(player.currentPosition) }, sources = session, elsewhere = true)
            }
        }

        override fun onTracksChanged(tracks: Tracks) {
            if (tracks.groups.isEmpty()) return
            val choices = this@VideoPlayerActivity.tracks
            if (!tracksPicked) {
                tracksPicked = true
                choices.pick(state, prefs, languages)
            }
            // Another source's sound just added plays; Audio has it, the delay, the files and
            // smart audio.
            audio.onTracksChanged()
            controls.setAudioChoice(true)
            // The stream's own sizes go into the Quality menu.
            controls.setQualityChoice(quality.offered)
            quality.reportVariants()
            // A subtitle file just added from the device plays at once.
            pendingSubtitle?.let { name ->
                val track = choices.list(C.TRACK_TYPE_TEXT).firstOrNull { it.format.label == name }
                if (track != null) {
                    pendingSubtitle = null
                    choices.select(C.TRACK_TYPE_TEXT, track)
                }
            }
            // An open subtitle sheet shows what Media3 settled on.
            sheets.tracksChanged()
        }

        override fun onAudioSessionIdChanged(audioSessionId: Int) {
            attachLoudness(audioSessionId)
        }

        override fun onCues(cueGroup: CueGroup) {
            // A live track shows instead while it's the main one.
            if (liveTracks.primary == null) mainCues.set(cueGroup.cues)
        }

        override fun onVideoSizeChanged(videoSize: VideoSize) {
            // The frame is measured again for the new size; the chosen aspect must hold for it.
            if (videoSize.width > 0) playerView.post { applyAspect(aspect) }
            // Another variant plays: the Quality menu marks it.
            quality.reportVariants()
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
        // Reported once; nothing more goes to the old video's progress.
        videoId = ""
        player.pause()
        readSettings(request)
        if (!request.has("session")) session = false
        // The new session says what it is with its sources.
        linkSession = false
        sourcesAlone = false
        sourcesFallback = true
        heading = heading(request)
        controls.setTitle(heading)
        controls.setSession(session, several, hasPrevious, hasNext)
        request.optJSONObject("cast")?.let { cast.show(it) }
        if (request.optString("url").isNotBlank()) {
            load(request)
        } else {
            showLoading(request.optString("loadingText").ifBlank { label("findingSource", "Finding a source…") })
        }
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
                pickedSource = false
                showLoading(update.optString("text"))
            }
            "error" -> {
                showProblem(update.optString("text"), null, retry = { request("retry") }, sources = session, elsewhere = false)
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
            "sources" -> {
                sources = update.optJSONArray("groups") ?: JSONArray()
                sourceOrder = update.optJSONArray("order")
                playingFrom = update.text("playingFrom")
                linkSession = update.optBoolean("link")
                sourcesAlone = update.optBoolean("alone")
                sourcesFallback = update.optBoolean("fallback", true)
                sourcesSheet.refresh()
                // What the wait is for, as the sources answer (a failure's reason stays meanwhile).
                if (loading.finding) {
                    sourcesChecked()?.let { loading.setNote(it) }
                } else if (loading.shown && !started) {
                    loading.setNote(sourceNote())
                }
            }
            // A torrent playing: its peers and speed while the picture waits for pieces.
            "transfer" -> if (update.optString("id") == videoId) {
                transferNote = transferText(update)
                if (loading.shown && !loading.finding && !problem.shown) loading.setNote(waitingNote())
            }
            "castDevices", "cast", "castError" -> cast.onUpdate(update)
            // The Quality menu's steps (`video_quality::rows`).
            "qualities" -> quality.onUpdate(update)
            // Other sources' streams of the episode, for their sound (`video_session::audio`).
            "audioSources" -> audio.onUpdate(update)
            "subtitleTools", "subtitleTrack", "subtitleAsk", "subtitleNotice" -> {
                // A live stream's AI subtitles say how far behind live the picture must play.
                if (update.optString("id") == videoId) liveStream.onSubtitleUpdate(update)
                liveTracks.onUpdate(update)
            }
            "prefs" -> settingsChanged(update)
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
        liveTracks.onLoad(videoId)
        readSettings(request)
        spokenLanguage = request.text("spokenLanguage")
        languages = LanguagePrefs.from(prefs, appLanguage, subtitleLanguage, subtitleMode, spokenLanguage)
        ended = false
        started = false
        transferNote = null
        rebuffering = false
        nextCancelled = false
        nextRequested = false
        nextPrompt.visibility = View.GONE
        heading = heading(request)
        controls.setTitle(heading)
        controls.setSeekStep(seekStepMs)
        controls.setAudioChoice(false)
        applySubtitleStyle()

        state = TitleState(request.optJSONObject("titleState"), request.optJSONObject("lastSubtitles"))
        effects.onLoad(request)
        group = request.optString("group").ifBlank { videoId }
        // Another episode starts with its own sound; the title's audio delay applies.
        audio.onLoad(videoId, state)
        chapters = Chapter.list(request.optJSONArray("chapters"))
        controls.setChapters(chapters)
        countdownChapter = -1
        cancelledCountdowns.clear()
        autoSkipped.clear()
        aniskipAsked = false
        tracksPicked = false
        pendingSubtitle = null
        mainCues.set(null)

        sidecars.clear()
        request.optJSONArray("subtitles")?.let { list ->
            for (index in 0 until list.length()) {
                val track = list.optJSONObject(index) ?: continue
                val url = track.text("url") ?: continue
                val language = track.text("lang")
                val label = track.text("label")
                sidecars += Sidecar(
                    Uri.parse(url),
                    subtitleMime(track.text("format").orEmpty(), url),
                    // "English" as a language reads as `en`, so Media3 and the sheets agree.
                    Languages.code(language) ?: language,
                    label ?: language,
                    // The addon that found it, or its file's name: under its name in the sheets.
                    track.text("source"),
                )
            }
        }
        sidecars += added
        val secondChoice = state.secondary
        val secondTrack = secondChoice?.let { choice ->
            sidecars.firstOrNull { sameLanguage(it.language, choice.language) && it.label == choice.label }
                ?: sidecars.firstOrNull { sameLanguage(it.language, choice.language) }
        } ?: state.carried.secondary?.let { choice ->
            // Without a choice of this title's own: the last one made in any title.
            val candidates = sidecars.map { SubtitleCandidate(it.language, it.label, it.choice().kind ?: SubtitleKind.PLAIN) }
            carriedMatch(candidates, choice)?.let { sidecars[it] }
        }
        if (secondTrack != null) loadSecond(secondTrack) else second.clear()

        val startMs = request.optDouble("startMs", 0.0).toLong().coerceAtLeast(0)
        liveStream.onLoad(request, startMs)
        quality.onLoad()
        controls.setQualityChoice(session)
        if (request.optBoolean("external")) {
            launchExternal(request, startMs, only = true)
            return
        }
        if (cast.active) {
            // It plays on the TV; here it only names what plays.
            problem.hide()
            loading.hide()
            return
        }
        tracks.reset(languages, prefs.maxHeight, prefs.preferH264)
        // A live stream (or a start at 0) begins where Media3 begins it: a live stream's live edge.
        val source = buildSource(request, cached = !request.optBoolean("live"))
        if (liveStream.keepsStart(startMs)) player.setMediaSource(source, startMs) else player.setMediaSource(source)
        effects.beforePrepare()
        player.prepare()
        player.playWhenReady = true
        applySpeed(state.speed ?: prefs.defaultSpeed)
        applyAspect(state.aspect ?: prefs.aspect)
        tracks.text.delayUs = state.subtitleDelayMs * 1000
        gestures.resetBoost()
        problem.hide()
        loading.hide()
        hideHint()
        controls.show()
        updateWaiting()
        updatePip()
    }

    /**
     * The media source for `request`, with its subtitle files and the ones added here; `cached`
     * reads it through [VideoCache] (never a live stream, whose playlists change as it goes on).
     */
    private fun buildSource(request: JSONObject, cached: Boolean = cachedSource): MediaSource {
        val headers = mutableMapOf<String, String>()
        request.optJSONObject("headers")?.let { json ->
            json.keys().forEach { key -> headers[key] = json.optString(key) }
        }
        // The source's own User-Agent, else the one Raygun asked the source with.
        val agent = headers.entries.firstOrNull { it.key.equals("User-Agent", true) }?.value ?: userAgent
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(agent)
            .setDefaultRequestProperties(headers.filterKeys { !it.equals("User-Agent", true) })
            .setAllowCrossProtocolRedirects(true)
        val subtitles = sidecars.mapIndexed { index, sidecar ->
            MediaItem.SubtitleConfiguration.Builder(sidecar.uri)
                .setMimeType(sidecar.mime)
                .setLanguage(sidecar.language)
                .setLabel(sidecar.label)
                // Its track is known by it ([sidecarOf]), several files in one language and name too.
                .setId(sidecarId(index))
                .build()
        }
        val item = MediaItem.Builder()
            .setUri(request.optString("url"))
            .apply { streamMime(request.optString("format"))?.let { setMimeType(it) } }
            .setSubtitleConfigurations(subtitles)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(request.text("title"))
                    .setSubtitle(request.text("subtitle"))
                    .build(),
            )
            .build()
        val upstream = DefaultDataSource.Factory(this, http)
        val data = if (cached) VideoCache.wrap(this, Uri.parse(request.optString("url")), upstream) else upstream
        cachedSource = cached
        // Another source's sound beside the picture, and the audio delay ([PlayerAudio]).
        return audio.wrap(DefaultMediaSourceFactory(data).createMediaSource(item))
    }

    /** Prepares the same video again from `positionMs` (another decoder, an added subtitle file, a retry). */
    private fun reload(positionMs: Long) {
        if (current.optString("url").isBlank()) return
        problem.hide()
        tracksPicked = false
        started = false
        // A live stream comes back at its live edge (or as far behind it as it was held).
        if (liveStream.isLive) {
            player.setMediaSource(buildSource(current, cached = false))
        } else {
            player.setMediaSource(buildSource(current), positionMs)
        }
        player.prepare()
        player.play()
        updateWaiting()
    }

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

    private fun setAspect(next: String) {
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
    }

    /** Fit → Crop → Stretch, remembered for the title. */
    internal fun cycleAspect() {
        setAspect(
            when (aspect) {
                "fit" -> "crop"
                "crop" -> "stretch"
                else -> "fit"
            },
        )
        controls.bump()
    }

    /** A pinch out fills the screen, a pinch in fits the whole picture again. */
    internal fun pinchAspect(fill: Boolean) {
        val next = if (fill) "crop" else "fit"
        if (next != aspect) setAspect(next)
    }

    private fun applyOrientation() {
        requestedOrientation = screenOrientationFor(prefs.orientation)
    }

    /** The rotate button: portrait ⇄ landscape for this viewing; once held, a tap lets it turn again. */
    internal fun rotate() {
        if (rotationKept) {
            rotationKept = false
            requestedOrientation = orientationBeforeKeeping
            controls.setRotationLocked(false)
            showHint(label("rotationFree", "Rotation unlocked"))
            controls.bump()
            return
        }
        manualRotation = true
        val landscape = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        requestedOrientation = if (landscape) {
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
        } else {
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
        controls.bump()
    }

    /**
     * Holding the rotate button keeps the screen exactly as it is now: this side up, not the
     * opposite one when the phone turns over.
     */
    internal fun lockRotation() {
        if (!rotationKept) orientationBeforeKeeping = requestedOrientation
        rotationKept = true
        manualRotation = true
        requestedOrientation = currentOrientation()
        controls.setRotationLocked(true)
        showHint(label("rotationLocked", "Rotation locked"))
        controls.bump()
    }

    /** The orientation the screen shows now, side and direction (landscape or reverse landscape). */
    private fun currentOrientation(): Int {
        val rotation = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            display?.rotation ?: Surface.ROTATION_0
        } else {
            @Suppress("DEPRECATION")
            windowManager.defaultDisplay.rotation
        }
        val landscape = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        // Phones stand portrait at rotation 0, most tablets lie landscape.
        val naturalPortrait = (rotation == Surface.ROTATION_0 || rotation == Surface.ROTATION_180) != landscape
        return if (naturalPortrait) {
            when (rotation) {
                Surface.ROTATION_90 -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                Surface.ROTATION_180 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
                Surface.ROTATION_270 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
                else -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            }
        } else {
            when (rotation) {
                Surface.ROTATION_90 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
                Surface.ROTATION_180 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
                Surface.ROTATION_270 -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                else -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            }
        }
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
            // After a failure, playing again starts the video again.
            player.playerError != null && !session -> reload(player.currentPosition)
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
        val row = controls.skipRow
        val button = controls.skipButton
        val dontSkip = controls.dontSkipButton
        // The next-episode prompt takes the skip button's place over the credits.
        if (nextPrompt.visibility == View.VISIBLE || isInPip()) {
            row.visibility = View.GONE
            return
        }
        row.visibility = View.VISIBLE
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
                        button.show(label("skippingIn", "Skipping in {n} s", ((left + 999) / 1000).toString()))
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
                        button.show(
                            when (chapter.kind) {
                                "opening" -> label("skipOpening", "Skip opening")
                                "ending" -> label("skipEnding", "Skip ending")
                                else -> label("skipRecap", "Skip recap")
                            },
                        )
                        button.visibility = View.VISIBLE
                        dontSkip.visibility = View.GONE
                        return
                    }
                }
            }
        }
        countdownChapter = -1
        dontSkip.visibility = View.GONE
        // Jumping the intro's length: early in an episode of a series without an opening chapter.
        val intro = introMs()
        val early = position in 1_000L..300_000L
        val offered = prefs.skipButton && intro > 0 && session && several && early &&
            chapters.none { it.kind == "opening" }
        if (offered && controls.visible && !controls.locked && player.duration > 0) {
            button.show(label("skipBy", "+{n} s", (intro / 1000).toString()))
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
        val styleFlags = (if (look.bold) Typeface.BOLD else 0) or (if (look.italic) Typeface.ITALIC else 0)
        val caption = CaptionStyleCompat(
            look.textColor,
            // The background is drawn by the lines themselves, in rounded boxes (`restyled`).
            if (look.roundedBackground()) Color.TRANSPARENT else look.backgroundColor,
            Color.TRANSPARENT,
            edgeType(look.edge),
            look.edgeColor,
            // The system's caption families (`subtitleFamily`), on both subtitle views.
            subtitleTypeface(look.font, styleFlags),
        )
        listOf(subtitleView, secondView).forEach { view ->
            view.setStyle(caption)
            // A share of the picture's height as it shows (the views take the picture's place,
            // `placeSubtitles`), whether or not the controls lift the lines; the size setting is
            // a factor of it.
            view.setFractionalTextSize(SubtitleView.DEFAULT_TEXT_SIZE_FRACTION * look.size / 100f, true)
            // "Ignore the file's styles" is applied to the cues (`restyled`), so the background
            // boxes drawn with them stay.
            view.setApplyEmbeddedStyles(true)
            view.setApplyEmbeddedFontSizes(true)
        }
        // The lines on screen take the style now, the file's own colours under the viewer's.
        mainCues.look = look
        secondCues.look = look
        liftSubtitles(::controls.isInitialized && controls.visible && !controls.locked)
    }

    /**
     * Fits the subtitle views to the part of the picture on screen, so lines are sized to the
     * picture as it shows, not to the screen: in portrait they sit at the picture's foot at its
     * size, and a cropped picture larger than the screen keeps them on screen. Until the
     * picture's frame is laid out they take the place the picture will have ([subtitleArea]).
     */
    private fun placeSubtitles() {
        if (!::root.isInitialized) return
        val width = root.width
        val height = root.height
        if (width <= 0 || height <= 0) return
        val frame = playerView.findViewById<View>(androidx.media3.ui.R.id.exo_content_frame)
        val video = player.videoSize
        val area = subtitleArea(
            width,
            height,
            frame?.takeIf { video.width > 0 },
            video.width * video.pixelWidthHeightRatio,
            video.height.toFloat(),
        )
        for (view in listOf(subtitleView, secondView)) {
            val params = view.layoutParams as FrameLayout.LayoutParams
            if (params.leftMargin == area.left && params.topMargin == area.top &&
                params.width == area.width() && params.height == area.height()
            ) {
                continue
            }
            params.gravity = Gravity.TOP or Gravity.START
            params.leftMargin = area.left
            params.topMargin = area.top
            params.width = area.width()
            params.height = area.height()
            view.layoutParams = params
        }
        liftSubtitles(::controls.isInitialized && controls.visible && !controls.locked)
    }

    /**
     * Places subtitles at the chosen height, and out of the controls' way while they show: the
     * lines slide up above the bottom bar, the second subtitles (at the top) down below the top
     * bar, and back as the controls hide. Media3 puts many cues (WebVTT's among them) on the last
     * line, out of reach of its bottom padding fraction, so the view's own padding holds the
     * chosen height; it changes only with that setting, and the controls only move the views
     * (`translationY`), so the lines keep their size and wrapping. The controls call this as they
     * show and hide, and the shade behind them follows.
     */
    internal fun liftSubtitles(controlsShown: Boolean) {
        if (::vignette.isInitialized) vignette.shown = controlsShown
        if (!::subtitleView.isInitialized || !::root.isInitialized) return
        // The place `placeSubtitles` gave the view, before it is laid out there.
        val params = subtitleView.layoutParams as? FrameLayout.LayoutParams
        val height = params?.height?.takeIf { it > 0 } ?: subtitleView.height.takeIf { it > 0 } ?: root.height
        val top = params?.topMargin ?: subtitleView.top
        val chosen = (height * prefs.style.position / 100f).toInt()
        subtitleView.setBottomPaddingFraction(0f)
        if (subtitleView.paddingBottom != chosen) subtitleView.setPadding(0, 0, 0, chosen)
        val shown = controlsShown && ::controls.isInitialized
        val up = if (shown) liftAbove(top + height, chosen, root.height - controls.barHeight(), dp(8)) else 0
        val down = if (shown) {
            dropBelow(top + (height * SECOND_LINE_TOP).toInt(), controls.topBarBottom(), dp(4))
        } else {
            0
        }
        mainShift = slide(subtitleView, -up.toFloat(), mainShift)
        secondShift = slide(secondView, down.toFloat(), secondShift)
    }

    /**
     * Moves a subtitle view to `target` px from its place, gliding where animations are on (not
     * in picture-in-picture or before it's laid out); `current` is where it was going. Returns
     * the new target.
     */
    private fun slide(view: View, target: Float, current: Float): Float {
        if (target == current) return current
        view.animate().cancel()
        if (!DeviceState.animatorsEnabled() || !view.isLaidOut || isInPip()) {
            view.translationY = target
        } else {
            view.animate()
                .translationY(target)
                .setDuration(SUBTITLE_SLIDE_MS)
                .setInterpolator(DecelerateInterpolator())
                .start()
        }
        return target
    }

    /** A style change made in the player: shown at once and saved to Settings. */
    internal fun updateStyle(look: SubtitleLook) {
        previewStyle(look)
        savePrefs(JSONObject().put("subtitleStyle", look.toJson()))
    }

    /** Shows `look` on the subtitles without saving it yet (a colour being picked). */
    internal fun previewStyle(look: SubtitleLook) {
        prefs.style = look
        applySubtitleStyle()
    }

    /**
     * Settings changed in the app while this plays (Settings → Video player, the subtitle
     * language, the seek step): the player follows them at once, the subtitles' style included,
     * instead of from its next video.
     */
    private fun settingsChanged(update: JSONObject) {
        readSettings(update)
        effects.onPrefs(update)
        languages = LanguagePrefs.from(prefs, appLanguage, subtitleLanguage, subtitleMode, spokenLanguage)
        controls.setSeekStep(seekStepMs)
        applySubtitleStyle()
        root.post { placeSubtitles() }
    }

    /**
     * The shade behind the controls ([PlayerVignette]), over the picture and under the subtitles:
     * a child of the window-sized root, not of the controls (which keep to the safe area), so it
     * reaches every edge.
     */
    private fun installVignette() {
        vignette = PlayerVignette(this)
        root.addView(vignette, root.indexOfChild(dim) + 1, FrameLayout.LayoutParams(-1, -1))
        PlayerVignette.takeOverShades(controls.root)
        vignette.shown = controls.visible && !controls.locked
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
        second.load(sidecar) { problem ->
            showHint(label("secondFailed", "Couldn't load those subtitles") + "\n" + problem)
        }
    }

    internal fun pickSubtitleFile() {
        try {
            subtitleFile.launch(arrayOf("*/*"))
        } catch (_: ActivityNotFoundException) {
            showHint(label("noFilePicker", "No app here can pick a file"))
        }
    }

    internal fun pickAudioFile() {
        try {
            audioFile.launch(arrayOf("audio/*", "video/*"))
        } catch (_: ActivityNotFoundException) {
            showHint(label("noFilePicker", "No app here can pick a file"))
        }
    }

    /** A sound file chosen on the device plays beside the picture ([PlayerAudio]). */
    private fun addAudioFile(uri: Uri) {
        val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: uri.lastPathSegment ?: label("audioFile", "Audio file")
        audio.addFile(uri, name)
    }

    /** The User-Agent the video's source was asked with (its own when it names one). */
    internal val sourceUserAgent: String get() = userAgent

    /** The same video built again from where it is (another sound, a first audio delay), playing as it was. */
    internal fun reloadKeepingPlace() {
        // A TV plays it: nothing here is built again.
        if (::cast.isInitialized && cast.active) return
        val playing = player.playWhenReady
        reload(player.currentPosition)
        player.playWhenReady = playing
    }

    /** A subtitle file chosen on the device joins the video's tracks and plays. */
    private fun addSubtitleFile(uri: Uri) {
        val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: uri.lastPathSegment ?: "subtitles"
        // "Film.en.srt" and "Film.English.srt" say their language before the extension.
        val language = Languages.code(name.substringBeforeLast('.').substringAfterLast('.'))
        val sidecar = Sidecar(uri, subtitleMime("", name), language, name)
        added += sidecar
        sidecars += sidecar
        pendingSubtitle = name
        val playing = player.playWhenReady
        reload(player.currentPosition)
        player.playWhenReady = playing
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
            if (only) showLoading(label("playingElsewhere", "Playing in another app")) else player.pause()
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

    /** When subtitles turn on (`foreign`, `always`, `never`): saved, and tracks picked again. */
    internal fun setSubtitleMode(mode: String) {
        subtitleMode = mode
        savePrefs(JSONObject().put("subtitleMode", mode))
        repickTracks()
    }

    internal val currentSubtitleMode: String get() = subtitleMode

    /** The audio language to look for ("" for yours, `original`, or a code): saved and applied. */
    internal fun setAudioLanguage(language: String) {
        prefs.audioLanguage = language
        savePrefs(JSONObject().put("audioLanguage", language))
        repickTracks()
    }

    /** Picks audio and subtitles again by the languages; this title's own picks still win. */
    private fun repickTracks() {
        languages = LanguagePrefs.from(prefs, appLanguage, subtitleLanguage, subtitleMode, spokenLanguage)
        if (tracksPicked) tracks.pick(state, prefs, languages)
    }

    /** The largest picture automatic quality may use, from now on. */
    internal fun setResolution(resolution: String) {
        prefs.preferredResolution = resolution
        savePrefs(JSONObject().put("preferredResolution", resolution))
        tracks.limit(prefs.maxHeight)
    }

    internal fun setSeekStep(seconds: Int) {
        seekStepMs = seconds * 1000L
        controls.setSeekStep(seekStepMs)
        updatePip()
        request("seekStep", JSONObject().put("seconds", seconds))
    }

    /** Whether the video can wait in a small window while the app shows something else. */
    internal val canPip: Boolean
        get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    /**
     * Settings → Video player in the app: the video pauses in its small window (tapping it comes
     * back), and the app opens the screen.
     */
    internal fun openAppSettings() {
        player.pause()
        request("openSettings")
        enterPip()
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

    /** Loop this video on or off; the next-episode prompt waits while it loops. */
    internal fun toggleLoop() {
        looping = !looping
        player.repeatMode = if (looping) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
        updateNextPrompt()
        showHint(if (looping) label("loopOn", "Looping this video") else label("loopOff", "Loop off"))
    }

    /** Back at the start of a looping video: its intro may be skipped again, and a sleep timer
     * set for the end stops it here. */
    private fun wrapped() {
        autoSkipped.clear()
        cancelledCountdowns.clear()
        countdownChapter = -1
        report("progress")
        if (sleepAtEnd) {
            sleepAtEnd = false
            player.pause()
            controls.setNote(null)
            showHint(label("sleepEnded", "Sleep timer ended"))
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

    private fun isInPip(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && isInPictureInPictureMode

    internal fun enterPip() {
        closeSheet(openSheet)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            runCatching { enterPictureInPictureMode(tools.pipParams()) }
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
            seekBubble.visibility = View.GONE
            speedBadge.visibility = View.GONE
            nextPrompt.visibility = View.GONE
            gestures.brightnessBar.root.visibility = View.GONE
            gestures.volumeBar.root.visibility = View.GONE
        } else if (lifecycle.currentState == Lifecycle.State.CREATED) {
            // The small window was closed, not opened up again: the player closes with it.
            finish()
            return
        }
        loading.compact(isInPictureInPictureMode)
        problem.root.alpha = if (isInPictureInPictureMode) 0f else 1f
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
        if (cast.onKey(keyCode)) return true
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
        // The same sheet shown again after a change keeps its place in the list.
        val previous = openSheet
        previous?.let { root.removeView(it.root) }
        openSheet = sheet
        root.addView(sheet.root, FrameLayout.LayoutParams(-1, -1))
        if (previous != null && previous.title == sheet.title) sheet.scrolled = previous.scrolled
        controls.bump()
        syncSystemBars()
    }

    /** Whether `sheet` is the one showing. */
    internal fun sheetShown(sheet: PlayerSheet?): Boolean = sheet != null && openSheet === sheet

    internal fun closeSheet(sheet: PlayerSheet?) {
        if (sheet == null || openSheet !== sheet) return
        root.removeView(sheet.root)
        openSheet = null
        sheet.dismissed()
        syncSystemBars()
    }

    // ---- Play on TV ----

    internal fun askLocalNetwork() = localNetwork.launch(Manifest.permission.ACCESS_LOCAL_NETWORK)

    /** A TV took the video over: the copy here stops (Rust saves the TV's position). */
    internal fun castStarted() {
        closeSheet(openSheet)
        nextPrompt.visibility = View.GONE
        problem.hide()
        loading.hide()
        player.pause()
        player.stop()
    }

    /** Played to the end on the TV: the next episode follows there, as at the end here. */
    internal fun castFinished() {
        if (session && hasNext && autoplay && !nextCancelled) requestNext()
    }

    /** The subtitle file on, by its index in the video's `subtitles`, for the TV. */
    internal fun chosenSubtitleIndex(): Int? {
        if (tracks.textOff()) return null
        val format = tracks.list(C.TRACK_TYPE_TEXT).firstOrNull { tracks.chosen(it) }?.format ?: return null
        val count = current.optJSONArray("subtitles")?.length() ?: 0
        sidecarIndex(format.id)?.let { index -> return index.takeIf { it < count } }
        return sidecars.take(count).indexOfFirst {
            it.label == format.label && (it.language == format.language || sameLanguage(it.language, format.language))
        }.takeIf { it >= 0 }
    }

    /**
     * The subtitle file a Media3 track came from, by the id it was given ([sidecarId]); else, for
     * a track without an id, by its name and language. Null for the video's own tracks.
     */
    internal fun sidecarOf(format: Format): Sidecar? {
        sidecarIndex(format.id)?.let { return sidecars.getOrNull(it) }
        if (format.id != null) return null
        return sidecars.firstOrNull {
            it.label == format.label && (it.language == format.language || sameLanguage(it.language, format.language))
        }
    }

    // ---- Session requests (M44) ----

    internal fun requestNext() {
        if (nextRequested || !hasNext) return
        nextRequested = true
        nextPrompt.visibility = View.GONE
        request("next")
    }

    /** Plays a link picked in Sources, from where the video is. */
    internal fun chooseSource(group: Int, link: Int) {
        pickedSource = true
        showLoading(label("loadingSource", "Loading {n}…", sourceName(group, link) ?: label("sources", "Sources")))
        request("source", JSONObject().put("group", group).put("link", link))
    }

    /** A step picked in Quality that another link plays: the ring names its source and size. */
    internal fun pickedQuality(source: String?, step: String, again: Boolean) {
        pickedSource = true
        val name = listOfNotNull(source?.takeIf { it.isNotBlank() }, step).joinToString(" · ")
        showLoading(
            if (again) label("qualityNextSource", "Trying another source with {n}…", step)
            else label("loadingSource", "Loading {n}…", name),
        )
    }

    /** A step asked for in Quality that nothing else plays: the ring goes, the video plays on. */
    internal fun pickedNothing() {
        if (!pickedSource) return
        pickedSource = false
        loading.hide()
        controls.setBusy(false)
        updateWaiting()
    }

    /** The name of the source playing, for the Quality sheet. */
    internal fun playingSourceName(): String? {
        for (group in 0 until sources.length()) {
            val server = sources.optJSONObject(group) ?: continue
            val links = server.optJSONArray("links") ?: continue
            if ((0 until links.length()).any { links.optJSONObject(it)?.optBoolean("current") == true }) {
                return server.optString("name").ifBlank { null }
            }
        }
        return null
    }

    /**
     * Near the end of an episode with a next one: a "Next episode" button, counting down with
     * autoplay. Over credits (an ending chapter that runs to the end) it shows from their start.
     */
    private fun updateNextPrompt() {
        if (!session || !hasNext || nextCancelled || nextRequested || looping || isInPip() || liveStream.isLive) {
            nextPrompt.visibility = View.GONE
            return
        }
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: return
        val left = duration - player.currentPosition
        val countdown = if (promptMs > 0) promptMs else 1_000L
        val credits = chapters.lastOrNull { it.kind == "ending" && (it.endMs ?: duration) >= duration - CREDITS_SLACK_MS }
        val window = maxOf(countdown, credits?.let { duration - it.startMs } ?: 0L)
        if (left > window || left < 0 || !player.isPlaying && !ended) {
            nextPrompt.visibility = View.GONE
            return
        }
        val seconds = ((left + 999) / 1000).coerceAtLeast(0)
        nextText.show(
            if (autoplay && left <= countdown) {
                label("nextIn", "Next episode in {n} s", seconds.toString())
            } else {
                label("nextEpisode", "Next episode")
            },
        )
        nextPrompt.visibility = View.VISIBLE
    }

    /** The next-episode prompt sits where the skip button does. */
    internal fun placeNextPrompt() {
        if (!::nextPrompt.isInitialized || !::controls.isInitialized) return
        val bottom = controls.bottomOffset()
        // The live delay offer sits there too.
        val prompts = if (::liveStream.isInitialized) listOf(nextPrompt, liveStream.prompt) else listOf(nextPrompt)
        prompts.forEach { prompt ->
            val params = prompt.layoutParams as? FrameLayout.LayoutParams ?: return@forEach
            if (params.bottomMargin != bottom) {
                params.bottomMargin = bottom
                prompt.layoutParams = params
            }
        }
    }

    private fun tick() {
        if (cast.active) {
            cast.tick()
            return
        }
        sinceReport += TICK_MS
        if (sinceReport >= REPORT_INTERVAL_MS) {
            sinceReport = 0
            if (player.isPlaying) report("progress")
        }
        updateNextPrompt()
        if (controls.visible) controls.update()
        updateSkip()
        updateSleep()
        updateWaiting()
        liveStream.tick()
        // A live stream's AI subtitles are timed by its own clock.
        if (second.active) second.update(liveStream.clockMs(), state.subtitleDelayMs)
    }

    // ---- Waiting and problems ----

    /** Waiting for a video to be found or to start: the ring over a dimmed picture, with `text`. */
    internal fun showLoading(text: String, note: String? = null) {
        problem.hide()
        loading.show(text, note ?: sourcesChecked(), dim = true)
        controls.showTopOnly()
        controls.setBusy(true)
    }

    /**
     * The ring while a video buffers, with how much of what it needs has come in, and before it
     * starts which source it is.
     */
    private fun updateWaiting() {
        if (!::loading.isInitialized) return
        if (loading.finding || problem.shown) {
            controls.setBusy(loading.shown)
            return
        }
        val waiting = videoId.isNotEmpty() && player.playWhenReady && player.playbackState == Player.STATE_BUFFERING
        // After a seek (the bar, a double tap, the keys) the ring waits: most seeks are ready
        // before it would be read, and one that comes and goes at 99 % is only noise.
        val seekWait = !loading.shown && !seekWaitShows(started, SystemClock.elapsedRealtime() - seekedAt)
        if (!waiting || seekWait) {
            if (loading.shown) loading.hide()
            controls.setBusy(false)
            return
        }
        val ahead = (player.bufferedPosition - player.currentPosition).coerceAtLeast(0)
        val needed = if (rebuffering) REBUFFER_MS else START_BUFFER_MS
        loading.ring.progress = if (ahead > 0) (ahead.toFloat() / needed).coerceAtMost(0.99f) else null
        val note = waitingNote()
        if (loading.shown) loading.setNote(note) else loading.show(null, note, dim = false)
        controls.setBusy(true)
    }

    /** A failure the viewer can act on: try again, another source, another app. */
    private fun showProblem(text: String, info: String?, retry: (() -> Unit)?, sources: Boolean, elsewhere: Boolean) {
        loading.hide()
        hideHint()
        problem.show(text, info, retry, sources, elsewhere && current.optString("url").isNotBlank())
        controls.show()
        controls.setBusy(false)
    }

    /** "3 of 8 sources checked" while a session's sources answer; null once all have. */
    private fun sourcesChecked(): String? {
        val total = sources.length()
        if (total < 2) return null
        val answered = (0 until total).count { sources.optJSONObject(it)?.optString("state") != "resolving" }
        if (answered >= total) return null
        return countLabel(
            "sourcesChecked",
            "{n} of {of} source checked",
            "{n} of {of} sources checked",
            answered.toString(),
            total,
        )
    }

    /** Under the ring while the picture waits: the source before it starts, and a torrent's peers. */
    private fun waitingNote(): String? =
        listOfNotNull(if (started) null else sourceNote(), transferNote).joinToString(" · ").ifBlank { null }

    /** "12 peers · 1.4 MB/s" for a torrent playing, or that peers are being found. */
    private fun transferText(update: JSONObject): String {
        val peers = update.optInt("peers")
        val speed = update.optLong("downloadBps")
        val parts = mutableListOf(
            if (peers == 0) {
                label("torrentFindingPeers", "Finding peers…")
            } else {
                countLabel("torrentPeers", "{n} peer", "{n} peers", peers.toString(), peers)
            },
        )
        if (speed > 0) {
            parts += if (speed >= 1_000_000) {
                String.format(Locale.ROOT, "%.1f MB/s", speed / 1e6)
            } else {
                "${(speed / 1000).coerceAtLeast(1)} kB/s"
            }
        }
        if (update.optBoolean("dataSaver")) parts += label("torrentDataSaver", "Data Saver doesn't make torrents smaller")
        return parts.joinToString(" · ")
    }

    /** "Trying source 2 of 5 · Vidstream" for the link loading in a session, or just its name. */
    private fun sourceNote(): String? {
        var total = 0
        var failed = 0
        var name: String? = null
        for (group in 0 until sources.length()) {
            val links = sources.optJSONObject(group)?.optJSONArray("links") ?: continue
            for (link in 0 until links.length()) {
                val entry = links.optJSONObject(link) ?: continue
                total += 1
                if (entry.optBoolean("failed")) failed += 1
                if (entry.optBoolean("current")) name = sourceName(group, link)
            }
        }
        val playing = name ?: return null
        val number = failed + 1
        return if (pickedSource || total < 2 || number < 2) {
            playing
        } else {
            label("tryingSource", "Trying source {n} of {of}", number.toString(), maxOf(total, number).toString()) +
                " · " + playing
        }
    }

    /** A link's source as the viewer knows it: "Vidstream · 1080p", or a stream's own name. */
    private fun sourceName(group: Int, link: Int): String? {
        val server = sources.optJSONObject(group) ?: return null
        val groupName = server.optString("name").trim()
        // A Stremio stream's label starts with its name, which often names the addon already.
        val label = server.optJSONArray("links")?.optJSONObject(link)?.optString("label")
            ?.substringBefore(" · ")?.trim().orEmpty()
        return when {
            label.isEmpty() || label == groupName -> groupName.ifEmpty { null }
            groupName.isEmpty() || label.startsWith(groupName) -> label
            else -> "$groupName · $label"
        }
    }

    internal fun isLoading(): Boolean = ::loading.isInitialized && (loading.finding || problem.shown)

    // ---- Messages ----

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

    /** A double tap's seek, shown on the side tapped. */
    internal fun showSeekHint(side: Int, text: String) {
        seekBubble.text = text
        val params = seekBubble.layoutParams as FrameLayout.LayoutParams
        val gravity = Gravity.CENTER_VERTICAL or if (side < 0) Gravity.START else Gravity.END
        val margin = root.width / 8
        if (params.gravity != gravity || params.marginStart != margin) {
            params.gravity = gravity
            params.marginStart = margin
            params.marginEnd = margin
            seekBubble.layoutParams = params
        }
        seekBubble.visibility = View.VISIBLE
        main.removeCallbacks(hideSeekBubble)
        main.postDelayed(hideSeekBubble, 700)
    }

    /** The held speed, small and out of the way at the top; null hides it. */
    internal fun showSpeedBadge(text: String?) {
        speedBadge.text = text ?: ""
        speedBadge.visibility = if (text == null) View.GONE else View.VISIBLE
    }

    private fun heading(request: JSONObject): String {
        val name = request.text("title").orEmpty()
        val sub = request.text("subtitle") ?: return name
        return "$name\n$sub"
    }

    internal fun headingText(): String = heading

    /** A word in the app's language (`videoPlayer.*`), `{n}` filled with `value`. */
    internal fun label(key: String, fallback: String, value: String? = null): String {
        val text = labels.optString(key).ifBlank { fallback }
        return if (value == null) text else text.replace("{n}", value)
    }

    /** A word with two numbers: `{n}` and `{of}`. */
    internal fun label(key: String, fallback: String, value: String, of: String): String =
        label(key, fallback, value).replace("{of}", of)

    /**
     * A word that counts something ("3 of 8 lines"): the plural form the app's language takes for
     * `count` (i18next's `<key>_one`, `<key>_few`, `<key>_other`…, picked by Android's plural
     * rules for the language the labels came in), `{n}` filled with `value` and `{of}` with
     * `count`. English `one`/`other` when no label is there.
     */
    internal fun countLabel(key: String, one: String, other: String, value: String, count: Int): String {
        val form = pluralForm(count)
        val text = sequenceOf("${key}_$form", "${key}_other", key)
            .map { labels.optString(it) }
            .firstOrNull { it.isNotBlank() }
            ?: if (count == 1) one else other
        return text.replace("{n}", value).replace("{of}", count.toString())
    }

    /** The plural form (`one`, `few`, `other`…) of `count` in the labels' language. */
    private fun pluralForm(count: Int): String {
        val tag = labels.optString("locale")
        val locale = if (tag.isBlank()) Locale.getDefault() else Locale.forLanguageTag(tag)
        return runCatching { PluralRules.forLocale(locale).select(count.toDouble()) }.getOrDefault("other")
    }

    /**
     * Shows the system bars while the controls or a sheet show, hides them otherwise
     * (`PlayerSystemBars`). The controls call this as they show and hide; `again` re-applies the
     * state after the system may have changed it.
     */
    internal fun syncSystemBars(again: Boolean = false) {
        if (!::systemBars.isInitialized) return
        val shown = ::controls.isInitialized && controls.visible
        val locked = ::controls.isInitialized && controls.locked
        systemBars.apply(PlayerSystemBars.wanted(shown, locked, openSheet != null, isInPip()), again)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) syncSystemBars(again = true)
    }

    override fun onStop() {
        super.onStop()
        if (!::player.isInitialized) return
        if (!isInPip() && !externalOnly) player.pause()
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
        if (::player.isInitialized) {
            report("closed")
            player.release()
        }
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
        putPlace(json)
        extra.keys().forEach { key -> json.put(key, extra.get(key)) }
        NativeBridge.video(json.toString())
    }

    /**
     * Where the video is, for Rust: its position, or a live stream's place on its own clock with
     * where the segment playing starts on it (AI subtitles place the stream's sound by it).
     */
    private fun putPlace(json: JSONObject) {
        if (!::liveStream.isInitialized || !liveStream.isLive) {
            json.put("positionMs", player.currentPosition.coerceAtLeast(0))
            return
        }
        json.put("positionMs", liveStream.clockMs()).put("live", true)
        liveStream.anchor()?.let { json.put("liveAnchor", it) }
    }

    private fun report(state: String, error: String? = null) {
        if (videoId.isEmpty() && state != "closed") return
        // A TV plays it: its position is the one saved, not the stopped copy's here.
        if (::cast.isInitialized && cast.active && state != "closed") return
        val live = ::liveStream.isInitialized && liveStream.isLive
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 && !live }
        val json = JSONObject()
            .put("id", videoId)
            .put("state", state)
            .put("durationMs", duration ?: JSONObject.NULL)
            .put("error", error ?: JSONObject.NULL)
            .put("looping", looping)
        putPlace(json)
        // The connection's speed while this source plays, for Sources.
        val bandwidth = DefaultBandwidthMeter.getSingletonInstance(this).bitrateEstimate / 1000
        if (bandwidth > 0) json.put("bandwidthKbps", bandwidth)
        NativeBridge.video(json.toString())
    }

    private fun streamMime(format: String): String? = when (format.lowercase(Locale.ROOT)) {
        "hls", "m3u8" -> MimeTypes.APPLICATION_M3U8
        "dash", "mpd" -> MimeTypes.APPLICATION_MPD
        else -> null
    }
}

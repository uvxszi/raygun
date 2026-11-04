package app.raygun.audio

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.content.res.Configuration
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.provider.DocumentsContract
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.media.audiofx.PresetReverb
import android.provider.Settings
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.core.view.WindowCompat
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import org.json.JSONArray
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.MoreExecutors
import org.json.JSONObject
import kotlin.concurrent.thread

@InvokeArg
class LoadArgs {
    var loadId: Int = 0
    lateinit var url: String
    var title: String = ""
    var artist: String? = null
    var artworkUrl: String? = null
    var startMs: Double? = null
    var autoplay: Boolean = true
    var volume: Double? = null
    /** The song's gain in dB: its own volume plus normalization (M46.12). */
    var gainDb: Double? = null
    /** Its end trim, ms. */
    var endMs: Double? = null
    /** Fade from the playing song into this one, ms (a crossfade); 0 cuts. */
    var fadeMs: Double? = null
    /** Measure its loudness and report it. */
    var measure: Boolean = false
}

@InvokeArg
class MulticastArgs {
    var hold: Boolean = false
}

@InvokeArg
class AudioOutputArgs {
    var id: String? = null
}

@InvokeArg
class GainArgs {
    var gainDb: Double? = null
}

@InvokeArg
class EndArgs {
    var endMs: Double? = null
}

@InvokeArg
class FadeArgs {
    var fadeMs: Double? = null
}

/** A background capture of a stream's sound (M46.49, [SongCapture]). */
@InvokeArg
class CaptureStartArgs {
    lateinit var session: String
    lateinit var url: String
    var mime: String? = null
    var fromMs: Double = 0.0
    var speed: Double = 4.0
    lateinit var dir: String
}

@InvokeArg
class CaptureArgs {
    lateinit var session: String
}

@InvokeArg
class SpeechItem {
    var id: String = ""
    var text: String = ""
}

@InvokeArg
class SpeakArgs {
    var items: List<SpeechItem> = emptyList()
    var rate: Double = 1.0
}

@InvokeArg
class ThemeArgs {
    var theme: String = "system"
}

@InvokeArg
class FolderArgs {
    var uri: String = ""
}

@InvokeArg
class OutputArgs {
    var place: String = ""
    var tree: String? = null
    var path: Array<String> = emptyArray()
    var name: String = ""
    var mime: String = "application/octet-stream"
}

@InvokeArg
class FinishArgs {
    var uri: String = ""
    var ok: Boolean = true
}

@InvokeArg
class ShareArgs {
    var path: String = ""
    var mime: String = "application/octet-stream"
    var title: String = ""
}

@InvokeArg
class SeekArgs {
    var positionMs: Double? = null
}

@InvokeArg
class VolumeArgs {
    var volume: Double? = null
}

@InvokeArg
class RateArgs {
    var rate: Double? = null
}

@InvokeArg
class PathArgs {
    lateinit var path: String
}

@InvokeArg
class UrlArgs {
    lateinit var url: String
}

private const val PROGRESS_INTERVAL_MS = 500L
/** Playing this long without the position moving is silence, not playback. */
private const val STUCK_MS = 2500L
/** A song measured this long reports its loudness, s; one that ends sooner, if it played this long. */
private const val MEASURED_ENOUGH_S = 45.0
private const val MEASURED_AT_LEAST_S = 10.0

/** Android's preset reverbs by Raygun's names (M46.12). */
private fun reverbPreset(name: String?): Short? = when (name) {
    "smallRoom" -> PresetReverb.PRESET_SMALLROOM
    "mediumRoom" -> PresetReverb.PRESET_MEDIUMROOM
    "largeRoom" -> PresetReverb.PRESET_LARGEROOM
    "mediumHall" -> PresetReverb.PRESET_MEDIUMHALL
    "largeHall" -> PresetReverb.PRESET_LARGEHALL
    "plate" -> PresetReverb.PRESET_PLATE
    else -> null
}

/** Rust's `SoundPrefs` JSON as the processor's settings: tenths of a dB, percentages. */
private fun soundConfig(sound: JSONObject): SoundConfig {
    val bands = sound.optJSONArray("eqBands")
    return SoundConfig(
        eqEnabled = sound.optBoolean("eqEnabled", false),
        bands = DoubleArray(EQ_FREQUENCIES.size) { (bands?.optDouble(it, 0.0) ?: 0.0) / 10 },
        preampDb = sound.optDouble("preamp", 0.0) / 10,
        bassDb = sound.optDouble("bassBoost", 0.0) / 100 * 12,
        balance = (sound.optDouble("balance", 0.0) / 100).coerceIn(-1.0, 1.0),
        mono = sound.optBoolean("mono", false),
        normalize = sound.optBoolean("normalize", false),
        targetLufs = sound.optDouble("normalizeTarget", -14.0),
    )
}

/**
 * Native playback engine for the Rust `PlaybackService` (ARCHITECTURE §6.2). Executes engine
 * commands on a Media3 player hosted by [RaygunPlaybackService] and reports back through
 * [NativeBridge], tagging every report with the load id of the command that started the track.
 */
@UnstableApi
@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
        Permission(strings = [Manifest.permission.READ_MEDIA_VIDEO], alias = "video"),
        Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
        Permission(strings = [Manifest.permission.ACCESS_LOCAL_NETWORK], alias = "localNetwork"),
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications"),
    ],
)
class RaygunAudioPlugin(private val activity: Activity) : Plugin(activity) {
    private val main = Handler(Looper.getMainLooper())
    private var controller: MediaController? = null
    private val pending = mutableListOf<(MediaController) -> Unit>()
    private var connecting = false
    private var loadId: Int? = null
    /** The playing song's end trim, ms (M46.12), and whether it was reached. */
    private var endMs: Double? = null
    private var endReported = false
    private var theme = "system"
    private val speech by lazy { Speech(activity.applicationContext) }
    private val storage by lazy { Storage(activity.applicationContext) }
    /** Rust's player as it last reported itself, for the media session (see MirrorPlayer). */
    private var mirrorState: MirrorState? = null
    /** The status last reported for the current load, so each is sent once. */
    private var lastStatus: String? = null
    /** Set while `load` sets up the player: the states in between aren't the song's. */
    private var quiet = false
    // The watchdog: where the playing song's position last moved, and the recoveries tried.
    private var stuckPosition = -1L
    private var stuckSince = 0L
    private var recoveries = 0

    /** Waits for the song's end trim while it plays. */
    private val endWatch = Runnable { enginePlayer()?.let { checkEnd(it) } }

    private val progressTicker = object : Runnable {
        override fun run() {
            enginePlayer()?.let {
                reportProgress(it)
                watch()
            }
            main.postDelayed(this, PROGRESS_INTERVAL_MS)
        }
    }

    // The controller's events say when the session's player changed; what it is now is read from
    // the deck itself (enginePlayer), which the session shows whenever a load is current.
    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            val player = enginePlayer() ?: return
            if (isPlaying && loadId != null) {
                main.removeCallbacks(progressTicker)
                main.post(progressTicker)
                scheduleEnd(player)
            } else if (!isPlaying) {
                main.removeCallbacks(progressTicker)
                main.removeCallbacks(endWatch)
                reportProgress(player)
            }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            val player = enginePlayer() ?: return
            when (playbackState) {
                Player.STATE_READY -> reportProgress(player)
                Player.STATE_ENDED -> {
                    reportLoudnessIfHeard()
                    report("ended")
                }
                else -> Unit
            }
        }

        override fun onEvents(player: Player, events: Player.Events) {
            if (events.containsAny(
                    Player.EVENT_PLAY_WHEN_READY_CHANGED,
                    Player.EVENT_PLAYBACK_STATE_CHANGED,
                    Player.EVENT_IS_PLAYING_CHANGED,
                    Player.EVENT_PLAYBACK_SUPPRESSION_REASON_CHANGED,
                )
            ) {
                reportStatus()
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            report("error", JSONObject().put("message", error.message ?: error.errorCodeName))
        }
    }

    /**
     * The player the engine's commands drive: the service's current deck, directly. Never through
     * the media session: while a stream is found the session shows [MirrorPlayer], which offers
     * none of the commands a load needs (the controller drops them) and hands play and pause to
     * Rust as if the listener had pressed them. Raygun's own commands also skip the session's
     * routing of system controls ([RaygunSessionPlayer]).
     */
    private fun enginePlayer(): Player? = RaygunPlaybackService.instance?.deck?.player ?: controller

    /**
     * Tells Rust what the player does, from all of its state rather than one change: a song that
     * gets ready while paused (from the notification, a headset, a denied audio focus) or while
     * another app holds the focus says "paused", so Rust's player never shows playing over silence.
     */
    private fun reportStatus() {
        if (quiet) return
        val player = enginePlayer() ?: return
        val state = player.playbackState
        // The pause at an end trim isn't the listener's.
        if (state == Player.STATE_IDLE || state == Player.STATE_ENDED || endReported) return
        val status = when {
            !player.playWhenReady -> "paused"
            state == Player.STATE_BUFFERING -> "buffering"
            player.isPlaying -> "playing"
            // Ready, but another app has the audio focus (a call, the assistant).
            else -> "paused"
        }
        if (status == lastStatus) return
        lastStatus = status
        report(status)
    }

    /**
     * A song that "plays" without its position moving is silent: the audio output stalled, or
     * started where Android doesn't let it play. Seeking to where it is flushes the output; then
     * pausing and playing again restarts it and asks for the audio focus again; failing both, it
     * pauses (and says so), rather than show playing over silence.
     */
    private fun watch() {
        val player = RaygunPlaybackService.instance?.deck?.player ?: return
        if (!player.isPlaying || player.playbackState != Player.STATE_READY) {
            stuckPosition = -1L
            return
        }
        val now = android.os.SystemClock.elapsedRealtime()
        val position = player.currentPosition
        if (position != stuckPosition) {
            // Moving again: a later stall starts over, rather than counting every hiccup in a
            // song towards pausing it.
            if (stuckPosition >= 0) recoveries = 0
            stuckPosition = position
            stuckSince = now
            return
        }
        if (now - stuckSince < STUCK_MS) return
        stuckSince = now
        recoveries += 1
        android.util.Log.w("RaygunAudio", "playing without moving at ${position}ms, recovery $recoveries")
        when (recoveries) {
            1 -> player.seekTo(position)
            2 -> {
                player.pause()
                player.play()
            }
            else -> {
                recoveries = 0
                player.pause()
            }
        }
    }

    /**
     * Which player the media session shows: the native one while it holds a song, else Rust's
     * player as reported ([MirrorPlayer]); and whether the webview must keep playing meanwhile.
     */
    private fun syncSession() {
        val service = RaygunPlaybackService.instance
        val state = mirrorState
        val native = loadId != null
        if (service != null) {
            if (native) service.useDeck() else if (state != null) service.useMirror()
        }
        WebAwake.set(!native && state != null && (state.active || state.webActiveState()))
    }

    private fun MirrorState.webActiveState() = web && status != "ended" && status != "error"

    @Command
    fun load(invoke: Invoke) {
        val args = invoke.parseArgs(LoadArgs::class.java)
        withController { controller ->
            val service = RaygunPlaybackService.instance
            val fadeMs = (args.fadeMs ?: 0.0).toLong()
            // A fade from the song playing here, or from the webview's YouTube player.
            val crossfading = fadeMs > 0 && service != null
            loadId = args.loadId
            endMs = args.endMs
            endReported = false
            lastStatus = null
            recoveries = 0
            stuckPosition = -1L
            quiet = true
            main.removeCallbacks(endWatch)
            // The session shows the deck before anything goes into it: notification and headset
            // buttons are about this song from now on, and the mirror lets go of the audio focus
            // before the deck asks for it.
            syncSession()
            val player: Player = service?.deck?.player ?: controller
            val metadata = MediaMetadata.Builder()
                .setTitle(args.title)
                .setArtist(args.artist)
                .setArtworkUri(args.artworkUrl?.let(Uri::parse))
                .build()
            val item = MediaItem.Builder()
                .setUri(args.url)
                .setMediaId(args.loadId.toString())
                .setMediaMetadata(metadata)
                .build()
            val startMs = (args.startMs ?: 0.0).toLong()
            val gainDb = args.gainDb ?: 0.0
            player.volume = (args.volume ?: 1.0).toFloat()
            if (service != null && crossfading) {
                // The next song starts on the other player while this one fades out (M46.12).
                service.crossfade(item, startMs, fadeMs, gainDb, args.measure)
            } else {
                service?.startTrack(gainDb, args.measure)
                player.setMediaItem(item, startMs)
                player.prepare()
                player.playWhenReady = args.autoplay
            }
            service?.deck?.processor?.onMeasured = measured(args.loadId)
            quiet = false
            if (enginePlayer()?.currentMediaItem == null) {
                // Nothing took the song: say so now, so the next stream gets its turn, rather
                // than leave Rust waiting for its start timeout.
                report("error", JSONObject().put("message", "The player didn't take this song"))
                return@withController
            }
            reportStatus()
        }
        invoke.resolve()
    }

    /** Reports a song's loudness once enough of it was heard (called on the audio thread). */
    private fun measured(id: Int): (LoudnessMeter) -> Unit = { meter ->
        if (meter.seconds >= MEASURED_ENOUGH_S) {
            RaygunPlaybackService.instance?.deck?.processor?.stopMeasuring()
            val lufs = meter.integrated()
            if (lufs != null) {
                main.post {
                    if (loadId == id) report("loudness", JSONObject().put("lufs", lufs))
                }
            }
        }
    }

    /** A song that ends before it was measured long enough still reports, if it played 10 s. */
    private fun reportLoudnessIfHeard() {
        val processor = RaygunPlaybackService.instance?.deck?.processor ?: return
        if (!processor.isMeasuring || processor.loudness.seconds < MEASURED_AT_LEAST_S) return
        processor.stopMeasuring()
        processor.loudness.integrated()?.let { report("loudness", JSONObject().put("lufs", it)) }
    }

    /** The equalizer and effects (M46.12): every player's processor, and the reverb. */
    @Command
    fun setEffects(invoke: Invoke) {
        val sound = invoke.getArgs().optJSONObject("sound") ?: JSONObject()
        val config = soundConfig(sound)
        val reverb = reverbPreset(sound.optString("reverb"))
        RaygunSound.config = config
        RaygunSound.reverb = reverb
        main.post { RaygunPlaybackService.instance?.applySound(config, reverb) }
        invoke.resolve()
    }

    /** The playing song's volume was edited. */
    @Command
    fun setTrackGain(invoke: Invoke) {
        val args = invoke.parseArgs(GainArgs::class.java)
        main.post { RaygunPlaybackService.instance?.deck?.processor?.setGainDb(args.gainDb ?: 0.0) }
        invoke.resolve()
    }

    /** The playing song's end trim was edited. */
    @Command
    fun setEnd(invoke: Invoke) {
        val args = invoke.parseArgs(EndArgs::class.java)
        withEngine {
            endMs = args.endMs
            endReported = false
            scheduleEnd(it)
        }
        invoke.resolve()
    }

    /** Fades the playing song out, then stops: YouTube's embed in the webview takes over. */
    @Command
    fun fadeOut(invoke: Invoke) {
        val args = invoke.parseArgs(FadeArgs::class.java)
        withController {
            loadId = null
            main.removeCallbacks(progressTicker)
            main.removeCallbacks(endWatch)
            val service = RaygunPlaybackService.instance
            if (service != null) {
                service.retire(service.deck, (args.fadeMs ?: 0.0).toLong())
            } else {
                it.stop()
                it.clearMediaItems()
            }
            syncSession()
        }
        invoke.resolve()
    }

    private fun scheduleEnd(player: Player) {
        main.removeCallbacks(endWatch)
        val end = endMs ?: return
        if (endReported || !player.isPlaying) return
        val speed = player.playbackParameters.speed.takeIf { it > 0 } ?: 1f
        val remaining = ((end - player.currentPosition) / speed).toLong()
        if (remaining <= 0) checkEnd(player) else main.postDelayed(endWatch, remaining)
    }

    /** At the end trim: report the end and pause; Rust loads what's next. */
    private fun checkEnd(player: Player) {
        val end = endMs ?: return
        if (endReported) return
        if (player.currentPosition + 40 < end) {
            scheduleEnd(player)
            return
        }
        endReported = true
        reportLoudnessIfHeard()
        player.pause()
        report("ended")
    }

    @Command
    fun play(invoke: Invoke) {
        withEngine {
            it.play()
            reportStatus()
        }
        invoke.resolve()
    }

    @Command
    fun pause(invoke: Invoke) {
        withEngine {
            it.pause()
            reportStatus()
        }
        invoke.resolve()
    }

    @Command
    fun seek(invoke: Invoke) {
        val args = invoke.parseArgs(SeekArgs::class.java)
        withEngine {
            it.seekTo((args.positionMs ?: 0.0).toLong())
            endReported = false
            scheduleEnd(it)
        }
        invoke.resolve()
    }

    @Command
    fun setVolume(invoke: Invoke) {
        val args = invoke.parseArgs(VolumeArgs::class.java)
        withEngine { it.volume = (args.volume ?: 1.0).toFloat() }
        invoke.resolve()
    }

    /** Playback speed (M46.8); Media3 keeps it for the next items. */
    @Command
    fun setRate(invoke: Invoke) {
        val args = invoke.parseArgs(RateArgs::class.java)
        val rate = (args.rate ?: 1.0).coerceIn(0.5, 2.0).toFloat()
        withEngine { it.setPlaybackSpeed(rate) }
        invoke.resolve()
    }

    @Command
    fun stop(invoke: Invoke) {
        withEngine {
            loadId = null
            main.removeCallbacks(progressTicker)
            main.removeCallbacks(endWatch)
            it.stop()
            it.clearMediaItems()
            syncSession()
        }
        invoke.resolve()
    }

    /**
     * Asks for permission to read the device's music (M07) and returns the public folders the
     * library should scan. Android 13+ uses READ_MEDIA_AUDIO; older versions READ_EXTERNAL_STORAGE.
     */
    @Command
    fun requestLibraryAccess(invoke: Invoke) {
        if (getPermissionState(libraryAlias()) == PermissionState.GRANTED) {
            resolveLibraryAccess(invoke)
        } else {
            requestPermissionForAlias(libraryAlias(), invoke, "libraryAccessResult")
        }
    }

    @PermissionCallback
    fun libraryAccessResult(invoke: Invoke) {
        resolveLibraryAccess(invoke)
    }

    private fun libraryAlias() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) "audio" else "storage"

    private fun resolveLibraryAccess(invoke: Invoke) {
        val folders = JSONArray()
            .put(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC).absolutePath)
            .put(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS).absolutePath)
        val result = JSObject()
        result.put("granted", getPermissionState(libraryAlias()) == PermissionState.GRANTED)
        result.put("folders", folders)
        invoke.resolve(result)
    }

    /**
     * Android 17's local network permission: whether the system restricts the local network for
     * Raygun (Android 17+ with target SDK 37+), and the permission's state. `blocked` means
     * Android won't ask again, so only the app's settings can allow it.
     */
    @Command
    fun localNetworkState(invoke: Invoke) {
        invoke.resolve(localNetworkAccess())
    }

    /** Shows the system dialog for local network access unless it is granted or not needed. */
    @Command
    fun requestLocalNetwork(invoke: Invoke) {
        if (!localNetworkEnforced() || getPermissionState("localNetwork") == PermissionState.GRANTED) {
            invoke.resolve(localNetworkAccess())
        } else {
            requestPermissionForAlias("localNetwork", invoke, "localNetworkResult")
        }
    }

    @PermissionCallback
    fun localNetworkResult(invoke: Invoke) {
        invoke.resolve(localNetworkAccess())
    }

    private fun localNetworkEnforced() =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.CINNAMON_BUN &&
            activity.applicationInfo.targetSdkVersion >= Build.VERSION_CODES.CINNAMON_BUN

    private fun localNetworkAccess(): JSObject {
        val enforced = localNetworkEnforced()
        val state = getPermissionState("localNetwork")
        val result = JSObject()
        result.put("enforced", enforced)
        result.put("granted", !enforced || state == PermissionState.GRANTED)
        result.put("blocked", enforced && state == PermissionState.DENIED)
        return result
    }

    /** Opens Raygun's page in the system settings, where a permission can be allowed by hand. */
    @Command
    fun openAppSettings(invoke: Invoke) {
        activity.startActivity(
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null)),
        )
        invoke.resolve()
    }

    /**
     * Installs a downloaded update (M46.47): Android's installer for the APK in the app's cache.
     * Without "Install unknown apps" for Raygun, its settings page opens instead and
     * `needsPermission` comes back; the user allows it and tries again.
     */
    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        activity.runOnUiThread {
            try {
                val pm = activity.packageManager
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !pm.canRequestPackageInstalls()) {
                    activity.startActivity(
                        Intent(
                            Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                            Uri.parse("package:${activity.packageName}"),
                        ),
                    )
                    invoke.resolve(JSObject().put("started", false).put("needsPermission", true))
                    return@runOnUiThread
                }
                val uri = androidx.core.content.FileProvider.getUriForFile(
                    activity,
                    "${activity.packageName}.fileprovider",
                    java.io.File(args.path),
                )
                activity.startActivity(
                    Intent(Intent.ACTION_VIEW)
                        .setDataAndType(uri, "application/vnd.android.package-archive")
                        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
                )
                invoke.resolve(JSObject().put("started", true).put("needsPermission", false))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "The installer didn't open")
            }
        }
    }

    /** Whether Raygun may install its own updates ("Install unknown apps"). */
    @Command
    fun canInstallPackages(invoke: Invoke) {
        val allowed = Build.VERSION.SDK_INT < Build.VERSION_CODES.O ||
            activity.packageManager.canRequestPackageInstalls()
        invoke.resolve(JSObject().put("allowed", allowed))
    }

    /** Opens a web address in the phone's browser (the release page, the repository). */
    @Command
    fun openUrl(invoke: Invoke) {
        val args = invoke.parseArgs(UrlArgs::class.java)
        val uri = Uri.parse(args.url)
        if (uri.scheme != "https") {
            invoke.reject("Only https addresses open")
            return
        }
        activity.runOnUiThread {
            try {
                activity.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "No browser opened it")
            }
        }
    }

    /** Whether the phone can read aloud (starts its text-to-speech engine the first time). */
    @Command
    fun ttsAvailable(invoke: Invoke) {
        speech.whenReady { available -> invoke.resolve(JSObject().put("available", available)) }
    }

    @Command
    fun ttsSpeak(invoke: Invoke) {
        val args = invoke.parseArgs(SpeakArgs::class.java)
        speech.whenReady { available ->
            if (available && speech.speak(args.items.map { it.id to it.text }, args.rate.toFloat())) {
                invoke.resolve()
            } else {
                invoke.reject("Text-to-speech isn't available")
            }
        }
    }

    @Command
    fun ttsStop(invoke: Invoke) {
        main.post { speech.stop() }
        invoke.resolve()
    }

    /**
     * Matches the status and navigation bar icons to Raygun's theme (M09): dark icons on the light
     * theme, light icons otherwise. "system" follows the OS night mode, re-checked on resume.
     */
    /** Opens the video player (M38) with a request: url, title, headers, subtitles, startMs, id. */
    @Command
    fun videoOpen(invoke: Invoke) {
        val request = invoke.getArgs().toString()
        activity.startActivity(
            Intent(activity, VideoPlayerActivity::class.java)
                .putExtra(VideoPlayerActivity.EXTRA_REQUEST, request),
        )
        invoke.resolve()
    }

    /**
     * Sends an update to the open video player (M44): a video to `load`, the session's `episodes`
     * and `sources`, or a `loading`/`error` message. Dropped when no player is open: the user
     * closed it, which ended the session.
     */
    @Command
    fun videoUpdate(invoke: Invoke) {
        VideoPlayerActivity.deliver(invoke.getArgs().toString())
        invoke.resolve()
    }

    // ---- The built-in browser (M46.3): see WebsiteActivity ----

    private var websiteReports: WebsiteReports? = null

    /** Reports from the browser's process reach Rust through this receiver. */
    private fun listenToWebsites() {
        if (websiteReports != null) return
        val receiver = WebsiteReports()
        androidx.core.content.ContextCompat.registerReceiver(
            activity,
            receiver,
            android.content.IntentFilter(WebsiteActivity.ACTION_REPORT),
            androidx.core.content.ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        websiteReports = receiver
    }

    @Command
    fun websiteOpen(invoke: Invoke) {
        listenToWebsites()
        activity.startActivity(
            Intent(activity, WebsiteActivity::class.java)
                .putExtra(WebsiteActivity.EXTRA_REQUEST, invoke.getArgs().toString()),
        )
        invoke.resolve()
    }

    // ---- Browser checks passed by hand (experimental): see BrowserCheckActivity ----

    /** Opens `{url, userAgent}` for the person to pass its check; answers `{cookies}` on close. */
    @Command
    fun browserCheck(invoke: Invoke) {
        val args = invoke.getArgs()
        val intent = Intent(activity, BrowserCheckActivity::class.java)
            .putExtra(BrowserCheckActivity.EXTRA_URL, args.getString("url", ""))
            .putExtra(BrowserCheckActivity.EXTRA_USER_AGENT, args.getString("userAgent", ""))
        startActivityForResult(invoke, intent, "browserCheckClosed")
    }

    @ActivityCallback
    fun browserCheckClosed(invoke: Invoke, result: ActivityResult) {
        val cookies = result.data?.getStringExtra(BrowserCheckActivity.EXTRA_COOKIES).orEmpty()
        invoke.resolve(JSObject().put("cookies", cookies))
    }

    // ---- Connect (M46.15): hearing other devices' mDNS needs a multicast lock ----

    private var multicastLock: WifiManager.MulticastLock? = null

    @Command
    fun setMulticast(invoke: Invoke) {
        val args = invoke.parseArgs(MulticastArgs::class.java)
        val wifi = activity.applicationContext.getSystemService(WifiManager::class.java)
        if (args.hold) {
            val lock = multicastLock ?: wifi?.createMulticastLock("raygun-connect")?.also {
                it.setReferenceCounted(false)
                multicastLock = it
            }
            lock?.acquire()
        } else {
            multicastLock?.release()
        }
        invoke.resolve()
    }

    // ---- Audio quality (M46.18): mobile data has its own setting ----

    /** What an output is, for Devices (M46.43); outputs nobody picks (the earpiece, calls) are left out. */
    private fun outputKind(type: Int): String? = when (type) {
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "speaker"
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET -> "wired"
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "bluetooth"
        AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_USB_HEADSET -> "usb"
        AudioDeviceInfo.TYPE_HDMI, AudioDeviceInfo.TYPE_HDMI_ARC -> "hdmi"
        else -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            (type == AudioDeviceInfo.TYPE_BLE_HEADSET || type == AudioDeviceInfo.TYPE_BLE_SPEAKER)
        ) "bluetooth" else null
    }

    private fun outputs(): List<AudioDeviceInfo> {
        val manager = activity.applicationContext.getSystemService(AudioManager::class.java) ?: return emptyList()
        return manager.getDevices(AudioManager.GET_DEVICES_OUTPUTS).filter { outputKind(it.type) != null }
    }

    /** The outputs sound can go to now (M46.43): speaker, wired and Bluetooth headphones, USB, HDMI. */
    @Command
    fun audioOutputs(invoke: Invoke) {
        val list = JSArray()
        for (device in outputs()) {
            list.put(
                JSObject()
                    .put("id", device.id.toString())
                    .put("name", device.productName?.toString().orEmpty())
                    .put("kind", outputKind(device.type)),
            )
        }
        val chosen = RaygunSound.output?.id?.toString()
        invoke.resolve(JSObject().put("outputs", list).put("chosen", chosen))
    }

    /** Plays through one output, or (no id) wherever Android sends sound. */
    @Command
    fun setAudioOutput(invoke: Invoke) {
        val args = invoke.parseArgs(AudioOutputArgs::class.java)
        val device = args.id?.let { id -> outputs().firstOrNull { it.id.toString() == id } }
        RaygunSound.output = device
        Handler(Looper.getMainLooper()).post { RaygunPlaybackService.instance?.applyOutput(device) }
        invoke.resolve()
    }

    /** See DeviceState. */
    @Command
    fun deviceState(invoke: Invoke) {
        main.post {
            DeviceState.start(activity)
            invoke.resolve(JSObject(DeviceState.snapshot().toString()))
        }
    }

    /** Starts capturing a stream's sound in the background (M46.49). */
    @Command
    fun captureStart(invoke: Invoke) {
        val args = invoke.parseArgs(CaptureStartArgs::class.java)
        try {
            SongCapture.start(activity.applicationContext, args)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "The capture didn't start")
        }
    }

    @Command
    fun captureStatus(invoke: Invoke) {
        val args = invoke.parseArgs(CaptureArgs::class.java)
        invoke.resolve(SongCapture.status(args.session))
    }

    @Command
    fun captureStop(invoke: Invoke) {
        val args = invoke.parseArgs(CaptureArgs::class.java)
        SongCapture.stop(args.session)
        invoke.resolve()
    }

    @Command
    fun networkKind(invoke: Invoke) {
        val connectivity = activity.applicationContext.getSystemService(ConnectivityManager::class.java)
        val capabilities = connectivity?.activeNetwork?.let { connectivity.getNetworkCapabilities(it) }
        val kind = when {
            capabilities == null -> "none"
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> "cellular"
            else -> "other"
        }
        invoke.resolve(JSObject().put("kind", kind))
    }

    @Command
    fun websiteClear(invoke: Invoke) {
        activity.startActivity(Intent(activity, WebsiteDataActivity::class.java))
        invoke.resolve()
    }

    // ---- Files outside the app (M46.1): see Storage ----

    @Command
    fun pickFolder(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
            Intent.FLAG_GRANT_READ_URI_PERMISSION or
                Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
        )
        startActivityForResult(invoke, intent, "folderPicked")
    }

    @ActivityCallback
    fun folderPicked(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject())
            return
        }
        val name = storage.keep(uri)
        invoke.resolve(JSObject().put("uri", uri.toString()).put("name", name ?: uri.lastPathSegment))
    }

    @Command
    fun folderState(invoke: Invoke) {
        val args = invoke.parseArgs(FolderArgs::class.java)
        val (granted, name) = storage.folderState(Uri.parse(args.uri))
        invoke.resolve(JSObject().put("granted", granted).put("name", name))
    }

    // ---- Local Files (M46.27): reading folders the user picked ----

    /** Asks to read the device's songs and videos (Android 13+), which Local Files plays by path. */
    @Command
    fun requestMediaAccess(invoke: Invoke) {
        val aliases = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) arrayOf("audio", "video") else arrayOf("storage")
        val granted = aliases.all { getPermissionState(it) == PermissionState.GRANTED }
        // "Not now" to Raygun's own question just before: Android isn't asked over it.
        if (granted || Permissions.recentlyDeclined("media")) {
            invoke.resolve(JSObject().put("granted", granted))
        } else {
            requestPermissionForAliases(aliases, invoke, "mediaAccessResult")
        }
    }

    @PermissionCallback
    fun mediaAccessResult(invoke: Invoke) {
        val aliases = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) arrayOf("audio", "video") else arrayOf("storage")
        invoke.resolve(JSObject().put("granted", aliases.all { getPermissionState(it) == PermissionState.GRANTED }))
    }

    /** Every file under a picked folder: its document address, path inside the folder, size and time. */
    @Command
    fun listTree(invoke: Invoke) {
        val args = invoke.parseArgs(FolderArgs::class.java)
        thread(name = "raygun-local-files") {
            try {
                val tree = Uri.parse(args.uri)
                val resolver = activity.contentResolver
                val files = JSONArray()
                val pending = ArrayDeque<Pair<String, String>>()
                pending.add(DocumentsContract.getTreeDocumentId(tree) to "")
                val columns = arrayOf(
                    DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                    DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                    DocumentsContract.Document.COLUMN_MIME_TYPE,
                    DocumentsContract.Document.COLUMN_SIZE,
                    DocumentsContract.Document.COLUMN_LAST_MODIFIED,
                )
                while (pending.isNotEmpty() && files.length() < 20000) {
                    val (parent, prefix) = pending.removeFirst()
                    val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent)
                    resolver.query(children, columns, null, null, null)?.use { cursor ->
                        while (cursor.moveToNext()) {
                            val id = cursor.getString(0) ?: continue
                            val name = cursor.getString(1) ?: continue
                            val mime = cursor.getString(2) ?: ""
                            val path = if (prefix.isEmpty()) name else "$prefix/$name"
                            if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                                if (!name.startsWith(".")) pending.add(id to path)
                            } else {
                                files.put(
                                    JSObject()
                                        .put("uri", DocumentsContract.buildDocumentUriUsingTree(tree, id).toString())
                                        .put("path", path)
                                        .put("size", if (cursor.isNull(3)) 0L else cursor.getLong(3))
                                        .put("modified", if (cursor.isNull(4)) 0L else cursor.getLong(4)),
                                )
                            }
                        }
                    }
                }
                invoke.resolve(JSObject().put("files", files))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't read the folder")
            }
        }
    }

    /** Opens a picked file for reading and hands its descriptor to Rust, which closes it. */
    @Command
    fun openInput(invoke: Invoke) {
        val args = invoke.parseArgs(FolderArgs::class.java)
        thread(name = "raygun-local-files") {
            try {
                val descriptor = activity.contentResolver.openFileDescriptor(Uri.parse(args.uri), "r")
                    ?: throw IllegalStateException("The file is gone")
                invoke.resolve(JSObject().put("fd", descriptor.detachFd()))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't open the file")
            }
        }
    }

    @Command
    fun releaseFolder(invoke: Invoke) {
        val args = invoke.parseArgs(FolderArgs::class.java)
        storage.release(Uri.parse(args.uri))
        invoke.resolve()
    }

    /** Off the main thread: the Storage Access Framework may be slow (a cloud folder). */
    @Command
    fun createOutput(invoke: Invoke) {
        val args = invoke.parseArgs(OutputArgs::class.java)
        thread(name = "raygun-output") {
            try {
                val output = storage.create(args.place, args.tree, args.path.toList(), args.name, args.mime)
                invoke.resolve(JSObject().put("uri", output.uri.toString()).put("fd", output.fd))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't make the file")
            }
        }
    }

    @Command
    fun finishOutput(invoke: Invoke) {
        val args = invoke.parseArgs(FinishArgs::class.java)
        thread(name = "raygun-output") {
            try {
                storage.finish(Uri.parse(args.uri), args.ok)
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't finish the file")
            }
        }
    }

    @Command
    fun shareFile(invoke: Invoke) {
        val args = invoke.parseArgs(ShareArgs::class.java)
        activity.runOnUiThread {
            try {
                activity.startActivity(storage.share(args.path, args.mime, args.title))
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't share the file")
            }
        }
    }

    @Command
    fun copyImage(invoke: Invoke) {
        val args = invoke.parseArgs(ShareArgs::class.java)
        activity.runOnUiThread {
            try {
                storage.copyImage(args.path, args.title)
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't copy the image")
            }
        }
    }

    // ---- Background playback: Rust's player for the media session (see MirrorPlayer) ----

    @Command
    fun mediaState(invoke: Invoke) {
        val state = MirrorState.parse(invoke.getArgs().optJSONObject("state"))
        mirrorState = state
        if (state == null && RaygunPlaybackService.instance == null) {
            // Nothing plays and nothing did: no service to start for it.
            WebAwake.set(false)
            invoke.resolve()
            return
        }
        withController {
            RaygunPlaybackService.instance?.mirror?.update(state)
            syncSession()
        }
        invoke.resolve()
    }

    // ---- Download notifications (see DownloadNotices) ----

    @Command
    fun downloadsNotify(invoke: Invoke) {
        val args = invoke.getArgs()
        val plan = args.optJSONObject("plan") ?: JSONObject()
        val finished = args.optJSONObject("finished")
        main.post { DownloadNotices.show(activity.applicationContext, plan, finished) }
        invoke.resolve()
    }

    /** A tapped notification: the screen it opens, and a button pressed on it. */
    private fun takeOpenExtras(intent: Intent?) {
        val extras = intent ?: return
        extras.getStringExtra(EXTRA_OPEN_ROUTE)?.let { route ->
            extras.removeExtra(EXTRA_OPEN_ROUTE)
            NativeBridge.open(route)
        }
        extras.getStringExtra(DownloadNotices.EXTRA_ACTION)?.let { action ->
            extras.removeExtra(DownloadNotices.EXTRA_ACTION)
            NativeBridge.notificationAction(action)
        }
    }

    override fun load(webView: WebView) {
        takeOpenExtras(activity.intent)
        DeviceState.start(activity)
    }

    override fun onNewIntent(intent: Intent) {
        takeOpenExtras(intent)
    }

    // ---- Settings → Permissions (see Permissions) ----

    private fun permissionRows(): JSObject {
        val context = activity.applicationContext
        val rows = mutableListOf<JSObject>()
        val notifications = when {
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU ->
                when (getPermissionState("notifications")) {
                    PermissionState.GRANTED -> if (Permissions.notificationsOff(context)) "blocked" else "granted"
                    PermissionState.DENIED -> "blocked"
                    else -> "denied"
                }
            Permissions.notificationsOff(context) -> "blocked"
            else -> "granted"
        }
        rows.add(Permissions.row("notifications", notifications))
        rows.add(Permissions.row("battery", Permissions.battery(context)))
        val media = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) listOf("audio", "video") else listOf("storage")
        val mediaStates = media.map { getPermissionState(it) }
        rows.add(
            Permissions.row(
                "media",
                when {
                    mediaStates.all { it == PermissionState.GRANTED } -> "granted"
                    mediaStates.any { it == PermissionState.DENIED } -> "blocked"
                    else -> "denied"
                },
            ),
        )
        rows.add(Permissions.row("folders", "granted", Permissions.folders(context)))
        if (localNetworkEnforced()) {
            val local = localNetworkAccess()
            rows.add(
                Permissions.row(
                    "localNetwork",
                    when {
                        local.optBoolean("granted") -> "granted"
                        local.optBoolean("blocked") -> "blocked"
                        else -> "denied"
                    },
                ),
            )
        }
        rows.add(Permissions.row("installApps", if (Permissions.canInstall(context)) "granted" else "blocked"))
        return Permissions.list(rows)
    }

    @Command
    fun permissionStates(invoke: Invoke) {
        invoke.resolve(permissionRows())
    }

    /**
     * Asks for a permission: Android's question where it still asks, else the permission's page in
     * the system settings; answers with every state once the user is back.
     */
    @Command
    fun requestPermission(invoke: Invoke) {
        val kind = invoke.getArgs().optString("kind")
        val context = activity.applicationContext
        when (kind) {
            "notifications" -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                getPermissionState("notifications") != PermissionState.DENIED &&
                getPermissionState("notifications") != PermissionState.GRANTED
            ) {
                requestPermissionForAlias("notifications", invoke, "permissionAnswered")
            } else {
                startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            }
            "battery" -> {
                val intent = if (Permissions.battery(context) == "optimized") {
                    Permissions.batteryQuestion(context)
                } else {
                    Permissions.settingsIntent(context, kind)
                }
                startActivityForResult(invoke, intent, "permissionPageClosed")
            }
            "media" -> {
                val aliases = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) arrayOf("audio", "video") else arrayOf("storage")
                if (aliases.any { getPermissionState(it) == PermissionState.DENIED }) {
                    startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
                } else {
                    requestPermissionForAliases(aliases, invoke, "permissionAnswered")
                }
            }
            "localNetwork" -> if (localNetworkEnforced() && getPermissionState("localNetwork") != PermissionState.DENIED) {
                requestPermissionForAlias("localNetwork", invoke, "permissionAnswered")
            } else {
                startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            }
            "installApps" -> startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            else -> invoke.resolve(permissionRows())
        }
    }

    @PermissionCallback
    fun permissionAnswered(invoke: Invoke) {
        invoke.resolve(permissionRows())
    }

    @ActivityCallback
    fun permissionPageClosed(invoke: Invoke, result: ActivityResult) {
        invoke.resolve(permissionRows())
    }

    @Command
    fun openPermissionSettings(invoke: Invoke) {
        val kind = invoke.getArgs().optString("kind")
        activity.runOnUiThread {
            try {
                activity.startActivity(Permissions.settingsIntent(activity.applicationContext, kind))
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "The settings page didn't open")
            }
        }
    }

    /** "Not now" to Raygun's own question about `kind`. */
    @Command
    fun declinePermission(invoke: Invoke) {
        Permissions.decline(invoke.getArgs().optString("kind"))
        invoke.resolve()
    }

    @Command
    fun applyTheme(invoke: Invoke) {
        theme = invoke.parseArgs(ThemeArgs::class.java).theme
        main.post { applySystemBars() }
        invoke.resolve()
    }

    override fun onResume(activity: AppCompatActivity) {
        applySystemBars()
    }

    private fun applySystemBars() {
        val night = (activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES
        val lightBackground = when (theme) {
            "light" -> true
            "system" -> !night
            else -> false
        }
        val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        controller.isAppearanceLightStatusBars = lightBackground
        controller.isAppearanceLightNavigationBars = lightBackground
    }

    /** Runs [action] on the deck ([enginePlayer]) once the controller has started the service. */
    private fun withEngine(action: (Player) -> Unit) {
        withController { action(enginePlayer() ?: it) }
    }

    /** Runs [action] on the main thread once the controller is connected to the service. */
    private fun withController(action: (MediaController) -> Unit) {
        main.post {
            val connected = controller
            if (connected != null) {
                action(connected)
                return@post
            }
            pending.add(action)
            if (connecting) return@post
            connecting = true
            // The application's context: the connection outlives the activity (a rotation, a
            // recreated activity), and the hint marks this controller's commands as the engine's.
            val context = activity.applicationContext
            val token = SessionToken(context, ComponentName(context, RaygunPlaybackService::class.java))
            val hints = android.os.Bundle().apply { putBoolean(ENGINE_CONTROLLER_HINT, true) }
            val future = MediaController.Builder(context, token)
                .setConnectionHints(hints)
                .setListener(object : MediaController.Listener {
                    override fun onDisconnected(controller: MediaController) {
                        // The service went away: the next command connects to a new one.
                        if (this@RaygunAudioPlugin.controller === controller) {
                            this@RaygunAudioPlugin.controller = null
                        }
                        main.removeCallbacks(progressTicker)
                    }
                })
                .buildAsync()
            future.addListener({
                connecting = false
                val built = runCatching { future.get() }.getOrNull() ?: return@addListener
                built.addListener(listener)
                controller = built
                pending.toList().forEach { it(built) }
                pending.clear()
            }, MoreExecutors.directExecutor())
        }
    }

    private fun reportProgress(controller: Player) {
        // The player itself: the controller's position is extrapolated, and would move over a
        // stalled output.
        val player: Player = RaygunPlaybackService.instance?.deck?.player ?: controller
        val duration = player.duration
        report(
            "progress",
            JSONObject()
                .put("positionMs", player.currentPosition.toDouble())
                .put("durationMs", if (duration == C.TIME_UNSET) JSONObject.NULL else duration.toDouble()),
        )
    }

    private fun report(type: String, fields: JSONObject = JSONObject()) {
        val id = loadId ?: return
        val event = fields.put("type", type)
        NativeBridge.send(JSONObject().put("loadId", id).put("event", event).toString())
    }
}

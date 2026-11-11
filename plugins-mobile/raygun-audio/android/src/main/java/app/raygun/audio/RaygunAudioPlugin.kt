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
import android.provider.OpenableColumns
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
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
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
    /** `url` is an HLS playlist (through Raygun's proxy): played with Media3's HLS reader. */
    var hls: Boolean = false
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
class OfflineGateArgs {
    var on: Boolean = false
}

@InvokeArg
class CastKeepAliveArgs {
    var on: Boolean = false
    var title: String = ""
    var text: String = ""
}

@InvokeArg
class SystemBarsArgs {
    var hidden: Boolean = false
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

/** A video export re-encoded with Media3's Transformer ([VideoConvert]). */
@InvokeArg
class ConvertStartArgs {
    lateinit var session: String
    lateinit var input: String
    lateinit var output: String
    var audioOnly: Boolean = false
    var stereo: Boolean = false
    /** What the source has (both when unknown). */
    var video: Boolean = true
    var audio: Boolean = true
}

@InvokeArg
class ConvertArgs {
    lateinit var session: String
}

@InvokeArg
class MediaCheckArgs {
    lateinit var path: String
}

/** Whether a hidden YouTube player being captured keeps the webview running ([WebAwake.hold]). */
@InvokeArg
class HoldArgs {
    var on: Boolean = false
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
    var pitch: Double = 1.0
    /** The book's language (BCP 47), or empty. */
    var language: String = ""
}

/** Speech a downloaded voice made: 16-bit mono PCM as base64. */
@InvokeArg
class SpeechClipArgs {
    var id: String = ""
    var sampleRate: Int = 22_050
    var pcm: String = ""
    var last: Boolean = false
}

@InvokeArg
class ReadAloudNoticeArgs {
    var title: String = ""
    var text: String = ""
    var playing: Boolean = true
}

@InvokeArg
class ThemeArgs {
    var theme: String = "system"
}

@InvokeArg
class FolderArgs {
    var uri: String = ""
}

/** A folder of shared storage for Local Files' browser (M46.51), and the kinds of file it counts. */
@InvokeArg
class BrowseMediaArgs {
    var path: String = ""
    var extensions: Array<String> = emptyArray()
}

@InvokeArg
class PickDocumentsArgs {
    var mimes: Array<String> = emptyArray()
}

@InvokeArg
class OutputArgs {
    var place: String = ""
    var tree: String? = null
    var path: Array<String> = emptyArray()
    var name: String = ""
    var mime: String = "application/octet-stream"
    var unique: Boolean = false
}

@InvokeArg
class FileArgs {
    var uri: String = ""
    var mime: String = "*/*"
}

@InvokeArg
class SavedArgs {
    var place: String = ""
    var tree: String? = null
    var contains: String = ""
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

private const val DocumentsContractDir = android.provider.DocumentsContract.Document.MIME_TYPE_DIR

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
        Permission(strings = [Manifest.permission.READ_MEDIA_IMAGES], alias = "images"),
        Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
        Permission(strings = [Manifest.permission.ACCESS_LOCAL_NETWORK], alias = "localNetwork"),
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications"),
        Permission(strings = [Manifest.permission.CAMERA], alias = "camera"),
    ],
)
class RaygunAudioPlugin(firstActivity: Activity) : Plugin(firstActivity) {
    /**
     * Raygun's activity now: a later one replaces the first when Raygun is opened again in a
     * process that carried on without one ([RaygunWindow]).
     */
    private var activity: Activity = firstActivity
    private val main = Handler(Looper.getMainLooper())
    private var controller: MediaController? = null
    private val pending = mutableListOf<(MediaController) -> Unit>()
    private var connecting = false
    /** Actions [withController] posted that haven't run yet: a later one waits behind them. */
    private val queued = java.util.concurrent.atomic.AtomicInteger(0)
    private var loadId: Int? = null
    /** The playing song's end trim, ms (M46.12), and whether it was reached. */
    private var endMs: Double? = null
    private var endReported = false
    private var theme = "system"
    private val speech by lazy { Speech(activity.applicationContext) }
    private val speechClips by lazy { SpeechClips(activity.applicationContext) }
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
                // The session's word comes late, and a report carries the load of now: the song
                // after may be in the deck already (a downloaded one loads at once, M46.53). Only
                // the deck's own end is this load's; taken as the new song's, the song before's
                // end skipped it, or ended the queue there.
                Player.STATE_ENDED -> if (holdsLoad(player) && player.playbackState == Player.STATE_ENDED) {
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
            // As with the end: only an error of the song the deck holds now is this load's (a
            // late one of the song before made a downloaded copy look broken).
            val player = enginePlayer()
            if (player != null && (!holdsLoad(player) || player.playerError == null)) return
            report("error", JSONObject().put("message", error.message ?: error.errorCodeName))
        }
    }

    /** Whether `player` holds the current load's song (each load's item is named by its id). */
    private fun holdsLoad(player: Player): Boolean =
        loadId != null && player.currentMediaItem?.mediaId == loadId.toString()

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
            // buttons are about this song from now on, and the mirror stops passing on headphone
            // changes before the deck handles them.
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
                // A song in HLS pieces (SoundCloud's): the default media source factory then
                // reads it as a playlist rather than as one file.
                .apply { if (args.hls) setMimeType(MimeTypes.APPLICATION_M3U8) }
                .build()
            val startMs = (args.startMs ?: 0.0).toLong()
            val gainDb = args.gainDb ?: 0.0
            player.volume = (args.volume ?: 1.0).toFloat()
            if (service != null && crossfading) {
                // The next song starts on the other player while this one fades out (M46.12).
                service.crossfade(item, startMs, fadeMs, gainDb, args.measure, args.autoplay)
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
            // Said again even when it's what was said last: Rust's player shows loading after a
            // Play until it hears, and a song already playing would never say so again.
            lastStatus = null
            reportStatus()
        }
        invoke.resolve()
    }

    @Command
    fun pause(invoke: Invoke) {
        withEngine {
            it.pause()
            lastStatus = null
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
            val items = args.items.map { it.id to it.text }
            if (available && speech.speak(items, args.rate.toFloat(), args.pitch.toFloat(), args.language)) {
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

    /** Plays speech a downloaded voice made, after the clips before it ([SpeechClips]). */
    @Command
    fun ttsClip(invoke: Invoke) {
        val args = invoke.parseArgs(SpeechClipArgs::class.java)
        try {
            speechClips.add(args.id, args.pcm, args.sampleRate, args.last)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "The speech couldn't be played")
        }
    }

    @Command
    fun ttsClipPause(invoke: Invoke) {
        speechClips.pause()
        invoke.resolve()
    }

    /** Goes on where the clips paused; refused when nothing is held (Rust reads the sentence again). */
    @Command
    fun ttsClipResume(invoke: Invoke) {
        if (speechClips.resume()) invoke.resolve() else invoke.reject("Nothing to go on with")
    }

    @Command
    fun ttsClipStop(invoke: Invoke) {
        speechClips.stop()
        invoke.resolve()
    }

    /** Shows or updates reading aloud's notification ([ReadAloudService]). */
    @Command
    fun ttsNotice(invoke: Invoke) {
        val args = invoke.parseArgs(ReadAloudNoticeArgs::class.java)
        ReadAloudService.show(activity.applicationContext, args.title, args.text, args.playing)
        WebAwake.reading(args.playing)
        invoke.resolve()
    }

    @Command
    fun ttsNoticeEnd(invoke: Invoke) {
        ReadAloudService.end(activity.applicationContext)
        WebAwake.reading(false)
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
        // The application's: the activity may go while the browser runs.
        androidx.core.content.ContextCompat.registerReceiver(
            activity.applicationContext,
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

    /** How the downloads the browser started stand, to its process (data only, no pages). */
    @Command
    fun websiteDownloads(invoke: Invoke) {
        val context = activity.applicationContext
        context.sendBroadcast(
            Intent(WebsiteActivity.ACTION_DOWNLOADS)
                .setPackage(context.packageName)
                .putExtra(WebsiteActivity.EXTRA_REPORT, invoke.getArgs().toString()),
        )
        invoke.resolve()
    }

    /**
     * The browser's site scripts, to its process: the list after an install (`{site, siteScripts,
     * installed}`) or why one failed (`{site, extension, error}`).
     */
    @Command
    fun websiteScripts(invoke: Invoke) {
        val context = activity.applicationContext
        context.sendBroadcast(
            Intent(WebsiteActivity.ACTION_SCRIPTS)
                .setPackage(context.packageName)
                .putExtra(WebsiteActivity.EXTRA_REPORT, invoke.getArgs().toString()),
        )
        invoke.resolve()
    }

    // ---- Browser checks passed by hand (experimental): see BrowserCheckActivity ----

    /**
     * Opens the request `{url, method, headers, body, userAgent}` that met a check, for the person
     * to pass it; answers `{cookies, userAgent, seen}` on close.
     */
    @Command
    fun browserCheck(invoke: Invoke) {
        val intent = Intent(activity, BrowserCheckActivity::class.java)
            .putExtra(BrowserCheckActivity.EXTRA_REQUEST, invoke.getArgs().toString())
        startActivityForResult(invoke, intent, "browserCheckClosed")
    }

    @ActivityCallback
    fun browserCheckClosed(invoke: Invoke, result: ActivityResult) {
        val data = result.data
        val seen = runCatching { JSONArray(data?.getStringExtra(BrowserCheckActivity.EXTRA_SEEN) ?: "[]") }
            .getOrElse { JSONArray() }
        invoke.resolve(
            JSObject()
                .put("cookies", data?.getStringExtra(BrowserCheckActivity.EXTRA_COOKIES).orEmpty())
                .put("userAgent", data?.getStringExtra(BrowserCheckActivity.EXTRA_USER_AGENT).orEmpty())
                .put("seen", seen),
        )
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

    // ---- Offline Mode: the main webview loads nothing from the network while it's on ----

    @Command
    fun setOfflineGate(invoke: Invoke) {
        OfflineGate.on = invoke.parseArgs(OfflineGateArgs::class.java).on
        invoke.resolve()
    }

    // ---- Play on TV: Raygun keeps running while a TV plays from it ----

    @Command
    fun setCastKeepAlive(invoke: Invoke) {
        val args = invoke.parseArgs(CastKeepAliveArgs::class.java)
        val context = activity.applicationContext
        if (args.on) CastKeepAliveService.start(context, args.title, args.text) else CastKeepAliveService.stop(context)
        invoke.resolve()
    }

    // ---- Audio quality (M46.18): mobile data has its own setting ----

    private fun audioManager(): AudioManager? =
        activity.applicationContext.getSystemService(AudioManager::class.java)

    private fun outputs(): List<AudioDeviceInfo> {
        val manager = audioManager() ?: return emptyList()
        return manager.getDevices(AudioManager.GET_DEVICES_OUTPUTS).filter { outputKindOf(it.type) != null }
    }

    /** Where Android sends music without a pick (Android 13 and later say; null before). */
    private fun systemOutput(): AudioDeviceInfo? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return null
        val music = android.media.AudioAttributes.Builder()
            .setUsage(android.media.AudioAttributes.USAGE_MEDIA)
            .setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC)
            .build()
        return runCatching { audioManager()?.getAudioDevicesForAttributes(music)?.firstOrNull() }.getOrNull()
    }

    /** The listed output `device` is, by id, else by kind and address, else by kind alone. */
    private fun listedId(device: AudioDeviceInfo?, listed: List<AudioDeviceInfo>): String? {
        if (device == null) return null
        val match = listed.firstOrNull { sameOutput(it, device) }
            ?: listed.firstOrNull { outputKindOf(it.type) != null && outputKindOf(it.type) == outputKindOf(device.type) }
        return match?.id?.toString()
    }

    /**
     * The outputs sound can go to now (M46.43): speaker, wired and Bluetooth headphones, USB,
     * HDMI; the one picked; where the playing song's sound goes (`routed`: the native player's
     * track as Android routes it, or Android's own choice for YouTube's player, which plays where
     * Android sends it, `web`); Android's choice without a pick (`system`); and whether Android
     * kept the song elsewhere than picked (`refused`).
     */
    @Command
    fun audioOutputs(invoke: Invoke) {
        val listed = outputs()
        val list = JSArray()
        for (device in listed) {
            list.put(
                JSObject()
                    .put("id", device.id.toString())
                    .put("name", device.productName?.toString().orEmpty())
                    .put("kind", outputKindOf(device.type)),
            )
        }
        val service = RaygunPlaybackService.instance
        val system = systemOutput()
        val native = if (loadId != null) service?.routedOutput() else null
        val state = mirrorState
        val web = native == null && loadId == null && state != null && state.web && state.status == "playing"
        val routed = native ?: if (web) system else null
        invoke.resolve(
            JSObject()
                .put("outputs", list)
                .put("chosen", RaygunSound.output?.id?.toString())
                .put("routed", listedId(routed, listed))
                .put("system", listedId(system, listed))
                .put("web", web)
                .put("refused", native != null && service?.outputRefused == true),
        )
    }

    /** Plays through one output, or (no id) wherever Android sends sound. */
    @Command
    fun setAudioOutput(invoke: Invoke) {
        val args = invoke.parseArgs(AudioOutputArgs::class.java)
        val device = args.id?.let { id -> outputs().firstOrNull { it.id.toString() == id } }
        if (args.id != null && device == null) {
            invoke.reject("That output isn't connected any more")
            return
        }
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

    /** Starts re-encoding a video export with Media3's Transformer. */
    @Command
    fun convertStart(invoke: Invoke) {
        val args = invoke.parseArgs(ConvertStartArgs::class.java)
        try {
            VideoConvert.start(activity.applicationContext, args)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "The conversion didn't start")
        }
    }

    @Command
    fun convertStatus(invoke: Invoke) {
        val args = invoke.parseArgs(ConvertArgs::class.java)
        invoke.resolve(VideoConvert.status(args.session))
    }

    @Command
    fun convertStop(invoke: Invoke) {
        val args = invoke.parseArgs(ConvertArgs::class.java)
        VideoConvert.stop(args.session)
        invoke.resolve()
    }

    /** What Android's extractor and decoders find in a converted file. */
    @Command
    fun mediaCheck(invoke: Invoke) {
        val args = invoke.parseArgs(MediaCheckArgs::class.java)
        VideoConvert.check(args.path) { found, error ->
            if (found != null) invoke.resolve(found) else invoke.reject(error ?: "The file is unreadable")
        }
    }

    /**
     * Keeps the webview running while Raygun is off screen, for a hidden YouTube player whose
     * sound is captured (M46.52: Raygun's tap reads it inside the player's frame), or lets it go.
     */
    @Command
    fun captureHoldWebview(invoke: Invoke) {
        val args = invoke.parseArgs(HoldArgs::class.java)
        WebAwake.hold(args.on)
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

    /** One file from the system picker (M46.52: fields that take a link or a file); Rust reads it with openInput. */
    @Command
    fun pickFile(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        startActivityForResult(invoke, intent, "filePicked")
    }

    @ActivityCallback
    fun filePicked(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject())
            return
        }
        val name = runCatching {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        }.getOrNull()
        invoke.resolve(JSObject().put("uri", uri.toString()).put("name", name ?: uri.lastPathSegment))
    }

    @Command
    fun folderState(invoke: Invoke) {
        val args = invoke.parseArgs(FolderArgs::class.java)
        val (granted, name) = storage.folderState(Uri.parse(args.uri))
        invoke.resolve(JSObject().put("granted", granted).put("name", name))
    }

    // ---- Local Files (M46.27): reading folders the user picked ----

    /** The media permissions: songs, videos and pictures (Android 13+), else the storage one. */
    private fun mediaAliases() =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) arrayOf("audio", "video", "images") else arrayOf("storage")

    private fun mediaGranted() = mediaAliases().all { getPermissionState(it) == PermissionState.GRANTED }

    /** Asks to read the device's songs, videos and pictures, which Local Files reads by path. */
    @Command
    fun requestMediaAccess(invoke: Invoke) {
        val granted = mediaGranted()
        // "Not now" to Raygun's own question just before: Android isn't asked over it.
        if (granted || Permissions.recentlyDeclined("media")) {
            invoke.resolve(JSObject().put("granted", granted))
        } else {
            requestPermissionForAliases(mediaAliases(), invoke, "mediaAccessResult")
        }
    }

    @PermissionCallback
    fun mediaAccessResult(invoke: Invoke) {
        invoke.resolve(JSObject().put("granted", mediaGranted()))
    }

    /**
     * What Raygun may read of shared storage (M46.51; see LocalStorage). `media` is any of the
     * media permissions: MediaStore then lists the kinds allowed.
     */
    @Command
    fun storageAccess(invoke: Invoke) {
        val media = mediaAliases().any { getPermissionState(it) == PermissionState.GRANTED }
        invoke.resolve(
            JSObject()
                .put("root", LocalStorage.root())
                .put("media", media)
                .put("allFiles", LocalStorage.allFiles(media))
                .put("canAskAllFiles", LocalStorage.canAskAllFiles()),
        )
    }

    /** A folder of shared storage as MediaStore lists it, for Local Files' browser (M46.51). */
    @Command
    fun browseMedia(invoke: Invoke) {
        val args = invoke.parseArgs(BrowseMediaArgs::class.java)
        val context = activity.applicationContext
        thread(name = "raygun-local-files") {
            try {
                val extensions = args.extensions.map { it.lowercase() }.toSet()
                invoke.resolve(LocalStorage.mediaFolders(context, args.path, extensions))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't list the phone's folders")
            }
        }
    }

    /** The system's file picker for several files at once: Local Files' Add files (M46.51). */
    @Command
    fun pickDocuments(invoke: Invoke) {
        val args = invoke.parseArgs(PickDocumentsArgs::class.java)
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        if (args.mimes.isNotEmpty()) intent.putExtra(Intent.EXTRA_MIME_TYPES, args.mimes)
        try {
            startActivityForResult(invoke, intent, "documentsPicked")
        } catch (error: android.content.ActivityNotFoundException) {
            invoke.reject("No app on this phone picks files")
        }
    }

    @ActivityCallback
    fun documentsPicked(invoke: Invoke, result: ActivityResult) {
        val data = result.data
        val uris = mutableListOf<Uri>()
        if (result.resultCode == Activity.RESULT_OK && data != null) {
            data.clipData?.let { clip ->
                for (index in 0 until clip.itemCount) clip.getItemAt(index).uri?.let(uris::add)
            }
            if (uris.isEmpty()) data.data?.let(uris::add)
        }
        val context = activity.applicationContext
        // Providers may be slow to describe their files (a cloud drive).
        thread(name = "raygun-local-files") {
            val files = JSArray()
            uris.forEach { files.put(LocalStorage.describe(context, it)) }
            invoke.resolve(JSObject().put("files", files))
        }
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
                val output = storage.create(args.place, args.tree, args.path.toList(), args.name, args.mime, args.unique)
                invoke.resolve(JSObject().put("uri", output.uri.toString()).put("fd", output.fd).put("name", output.name))
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

    /** Opens a file saved from a website (M47) in the app Android picks. */
    @Command
    fun openFile(invoke: Invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        activity.runOnUiThread {
            try {
                activity.startActivity(storage.view(Uri.parse(args.uri), args.mime))
                invoke.resolve()
            } catch (error: android.content.ActivityNotFoundException) {
                invoke.reject("No app on this phone opens this kind of file")
            } catch (error: Exception) {
                invoke.reject(error.message ?: "The file didn't open")
            }
        }
    }

    /** Shows the folder of a saved file. */
    @Command
    fun showFileFolder(invoke: Invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        activity.runOnUiThread {
            try {
                activity.startActivity(storage.folderOf(Uri.parse(args.uri)))
                invoke.resolve()
            } catch (error: Exception) {
                // Some phones have no downloads screen: the Files app's own.
                runCatching {
                    activity.startActivity(
                        Intent(Intent.ACTION_VIEW)
                            .setType(DocumentsContractDir)
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                    invoke.resolve()
                }.onFailure { invoke.reject("No app on this phone shows folders") }
            }
        }
    }

    /** Files Raygun saved in `Download/Raygun/` or a picked folder, by name (M46.51: backups). */
    @Command
    fun listSaved(invoke: Invoke) {
        val args = invoke.parseArgs(SavedArgs::class.java)
        thread(name = "raygun-output") {
            try {
                val files = JSONArray()
                for (file in storage.listSaved(args.place, args.tree, args.contains)) {
                    files.put(
                        JSObject()
                            .put("uri", file.uri.toString())
                            .put("name", file.name)
                            .put("size", file.size)
                            .put("modified", file.modified),
                    )
                }
                invoke.resolve(JSObject().put("files", files))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Couldn't read the folder")
            }
        }
    }

    /**
     * Starts Raygun again from its first screen (M46.51: after erasing everything). The WebView's
     * storage and cookies go first, and the notifications about what was erased (releases,
     * downloads); the app's other processes (the built-in browser, browser checks) end with this one.
     */
    @Command
    fun restartApp(invoke: Invoke) {
        activity.runOnUiThread {
            runCatching {
                activity.getSystemService(android.app.NotificationManager::class.java)?.cancelAll()
            }
            runCatching { android.webkit.WebStorage.getInstance().deleteAllData() }
            runCatching {
                android.webkit.CookieManager.getInstance().removeAllCookies(null)
                android.webkit.CookieManager.getInstance().flush()
            }
            val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName)?.component
            if (launch == null) {
                invoke.reject("Raygun couldn't start again; open it from the launcher")
                return@runOnUiThread
            }
            invoke.resolve()
            Handler(Looper.getMainLooper()).postDelayed({
                activity.startActivity(Intent.makeRestartActivityTask(launch))
                val manager = activity.getSystemService(android.app.ActivityManager::class.java)
                manager?.runningAppProcesses
                    ?.filter { it.uid == android.os.Process.myUid() && it.pid != android.os.Process.myPid() }
                    ?.forEach { android.os.Process.killProcess(it.pid) }
                Runtime.getRuntime().exit(0)
            }, 300)
        }
    }

    @Command
    fun deleteFile(invoke: Invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        thread(name = "raygun-output") {
            try {
                storage.delete(Uri.parse(args.uri))
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "The file couldn't be deleted")
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
        val previous = mirrorState
        mirrorState = state
        if (state == null && RaygunPlaybackService.instance == null) {
            // Nothing plays and nothing did: no service to start for it.
            WebAwake.set(false)
            invoke.resolve()
            return
        }
        withController {
            RaygunPlaybackService.instance?.mirror?.update(state)
            // The song playing here was given other details (a cover picked for it, a new
            // title): the notification and lock screen show them without loading it again.
            if (loadId != null && state != null && previous?.itemId == state.itemId &&
                (previous.title != state.title || previous.artist != state.artist ||
                    previous.artworkUrl != state.artworkUrl)
            ) {
                retitleDeck(state)
            }
            syncSession()
        }
        invoke.resolve()
    }

    /** Puts [state]'s title, artist and cover on the item the deck plays; its stream stays. */
    private fun retitleDeck(state: MirrorState) {
        val player = RaygunPlaybackService.instance?.deck?.player ?: return
        val current = player.currentMediaItem ?: return
        val metadata = current.mediaMetadata.buildUpon()
            .setTitle(state.title)
            .setArtist(state.artist)
            .setArtworkUri(state.artworkUrl?.let(Uri::parse))
            .build()
        player.replaceMediaItem(
            player.currentMediaItemIndex,
            current.buildUpon().setMediaMetadata(metadata).build(),
        )
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

    /** A notification that opens a screen when tapped (see Notices). */
    @Command
    fun notice(invoke: Invoke) {
        val notice = JSONObject(invoke.getArgs().toString())
        main.post { Notices.show(activity.applicationContext, notice) }
        invoke.resolve()
    }

    /** `{"id"}`: takes away the notice [notice] showed with it. */
    @Command
    fun noticeCancel(invoke: Invoke) {
        val id = invoke.getArgs().optInt("id")
        main.post { Notices.cancel(activity.applicationContext, id) }
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

    /**
     * Media and links another app opened with Raygun or shared to it (see OpenWith): taken from
     * the intent at once, described off the main thread (that asks the sharing app's provider).
     */
    private fun takeHandover(intent: Intent?) {
        val handover = OpenWith.take(intent) ?: return
        val context = activity.applicationContext
        thread(name = "raygun-open-with") { NativeBridge.openWith(handover.describe(context).toString()) }
    }

    /** Moves to [next], an activity made after the one this plugin started with went. */
    internal fun attach(next: Activity) {
        activity = next
    }

    override fun load(webView: WebView) {
        // Rust's commands reach this instance on their own line from now on, with or without an
        // activity (see DirectCalls).
        DirectCalls.plugin = this
        NativeBridge.attach()
        // The browser's process may outlive this one: its reports and downloads find a listener.
        listenToWebsites()
        takeOpenExtras(activity.intent)
        takeHandover(activity.intent)
        DeviceState.start(activity)
    }

    override fun onNewIntent(intent: Intent) {
        takeOpenExtras(intent)
        takeHandover(intent)
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
        val mediaStates = mediaAliases().map { getPermissionState(it) }
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
        // Optional (M46.51): only its own settings page allows it.
        if (LocalStorage.canAskAllFiles()) {
            rows.add(Permissions.row("allFiles", if (LocalStorage.allFiles(false)) "granted" else "denied"))
        }
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
        // Scanning another device's link QR code (the WebView's camera needs it granted here).
        rows.add(
            Permissions.row(
                "camera",
                when (getPermissionState("camera")) {
                    PermissionState.GRANTED -> "granted"
                    PermissionState.DENIED -> "blocked"
                    else -> "denied"
                },
            ),
        )
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
            "media" -> requestMedia(invoke)
            // Android 10 and older have no such page: the storage permission reads every file.
            "allFiles" -> if (LocalStorage.canAskAllFiles()) openAllFilesPage(invoke) else requestMedia(invoke)
            "localNetwork" -> if (localNetworkEnforced() && getPermissionState("localNetwork") != PermissionState.DENIED) {
                requestPermissionForAlias("localNetwork", invoke, "permissionAnswered")
            } else {
                startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            }
            "installApps" -> startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            "camera" -> if (getPermissionState("camera") != PermissionState.DENIED) {
                requestPermissionForAlias("camera", invoke, "permissionAnswered")
            } else {
                startActivityForResult(invoke, Permissions.settingsIntent(context, kind), "permissionPageClosed")
            }
            else -> invoke.resolve(permissionRows())
        }
    }

    private fun requestMedia(invoke: Invoke) {
        val aliases = mediaAliases()
        if (aliases.any { getPermissionState(it) == PermissionState.DENIED }) {
            startActivityForResult(
                invoke,
                Permissions.settingsIntent(activity.applicationContext, "media"),
                "permissionPageClosed",
            )
        } else {
            requestPermissionForAliases(aliases, invoke, "permissionAnswered")
        }
    }

    /** Access to all files' page (M46.51), answered with every state once the user is back. */
    private fun openAllFilesPage(invoke: Invoke) {
        for (intent in LocalStorage.allFilesIntents(activity.applicationContext)) {
            try {
                startActivityForResult(invoke, intent, "permissionPageClosed")
                return
            } catch (error: android.content.ActivityNotFoundException) {
                // Some builds have only the list of apps.
            }
        }
        invoke.resolve(permissionRows())
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

    // ---- Full screen readers: the status and navigation bars hidden, back on a swipe ----

    private var barsHidden = false

    @Command
    fun setSystemBars(invoke: Invoke) {
        barsHidden = invoke.parseArgs(SystemBarsArgs::class.java).hidden
        main.post { applySystemBars() }
        invoke.resolve()
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
        // Immersive sticky: a swipe from an edge shows the bars for a moment over the page.
        if (barsHidden) {
            controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            controller.hide(WindowInsetsCompat.Type.systemBars())
        } else {
            controller.show(WindowInsetsCompat.Type.systemBars())
        }
    }

    /** Runs [action] on the deck ([enginePlayer]) once the controller has started the service. */
    private fun withEngine(action: (Player) -> Unit) {
        withController { action(enginePlayer() ?: it) }
    }

    /** Runs [action] on the main thread once the controller is connected to the service. */
    private fun withController(action: (MediaController) -> Unit) {
        // Already on the main thread (where Rust's commands run) with the controller connected and
        // nothing ahead of it: at once, not a turn of the main loop later, so a Play or Pause
        // reaches the player as it's pressed.
        if (Looper.myLooper() == Looper.getMainLooper() && queued.get() == 0 && pending.isEmpty()) {
            val ready = controller
            if (ready != null) {
                action(ready)
                return
            }
        }
        queued.incrementAndGet()
        main.post {
            queued.decrementAndGet()
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

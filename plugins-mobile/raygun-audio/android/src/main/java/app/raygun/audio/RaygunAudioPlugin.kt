package app.raygun.audio

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.core.view.WindowCompat
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import org.json.JSONArray
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.MoreExecutors
import org.json.JSONObject

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
class SeekArgs {
    var positionMs: Double? = null
}

@InvokeArg
class VolumeArgs {
    var volume: Double? = null
}

private const val PROGRESS_INTERVAL_MS = 500L

/**
 * Native playback engine for the Rust `PlaybackService` (ARCHITECTURE §6.2). Executes engine
 * commands on a Media3 player hosted by [RaygunPlaybackService] and reports back through
 * [NativeBridge], tagging every report with the load id of the command that started the track.
 */
@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
        Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
    ],
)
class RaygunAudioPlugin(private val activity: Activity) : Plugin(activity) {
    private val main = Handler(Looper.getMainLooper())
    private var controller: MediaController? = null
    private val pending = mutableListOf<(MediaController) -> Unit>()
    private var connecting = false
    private var loadId: Int? = null
    private var theme = "system"
    private val speech by lazy { Speech(activity.applicationContext) }

    private val progressTicker = object : Runnable {
        override fun run() {
            controller?.let { reportProgress(it) }
            main.postDelayed(this, PROGRESS_INTERVAL_MS)
        }
    }

    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            val player = controller ?: return
            if (isPlaying) {
                report("playing")
                main.removeCallbacks(progressTicker)
                main.post(progressTicker)
            } else {
                main.removeCallbacks(progressTicker)
                reportProgress(player)
                if (player.playbackState == Player.STATE_READY) {
                    report("paused")
                }
            }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            val player = controller ?: return
            when (playbackState) {
                Player.STATE_BUFFERING -> report("buffering")
                Player.STATE_READY -> reportProgress(player)
                Player.STATE_ENDED -> report("ended")
                Player.STATE_IDLE -> Unit
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            report("error", JSONObject().put("message", error.message ?: error.errorCodeName))
        }
    }

    @Command
    fun load(invoke: Invoke) {
        val args = invoke.parseArgs(LoadArgs::class.java)
        withController { player ->
            loadId = args.loadId
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
            player.volume = (args.volume ?: 1.0).toFloat()
            player.setMediaItem(item, (args.startMs ?: 0.0).toLong())
            player.prepare()
            player.playWhenReady = args.autoplay
        }
        invoke.resolve()
    }

    @Command
    fun play(invoke: Invoke) {
        withController { it.play() }
        invoke.resolve()
    }

    @Command
    fun pause(invoke: Invoke) {
        withController { it.pause() }
        invoke.resolve()
    }

    @Command
    fun seek(invoke: Invoke) {
        val args = invoke.parseArgs(SeekArgs::class.java)
        withController { it.seekTo((args.positionMs ?: 0.0).toLong()) }
        invoke.resolve()
    }

    @Command
    fun setVolume(invoke: Invoke) {
        val args = invoke.parseArgs(VolumeArgs::class.java)
        withController { it.volume = (args.volume ?: 1.0).toFloat() }
        invoke.resolve()
    }

    @Command
    fun stop(invoke: Invoke) {
        withController {
            loadId = null
            main.removeCallbacks(progressTicker)
            it.stop()
            it.clearMediaItems()
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
            android.content.Intent(activity, VideoPlayerActivity::class.java)
                .putExtra(VideoPlayerActivity.EXTRA_REQUEST, request),
        )
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
            val token = SessionToken(activity, ComponentName(activity, RaygunPlaybackService::class.java))
            val future = MediaController.Builder(activity, token).buildAsync()
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

    private fun reportProgress(player: Player) {
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

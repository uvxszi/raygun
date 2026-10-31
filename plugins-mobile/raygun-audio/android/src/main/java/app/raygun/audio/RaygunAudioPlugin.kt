package app.raygun.audio

import android.app.Activity
import android.content.ComponentName
import android.os.Handler
import android.os.Looper
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.MoreExecutors
import org.json.JSONObject

@InvokeArg
class LoadArgs {
    var loadId: Int = 0
    lateinit var url: String
    var title: String = ""
    var artist: String? = null
    var startMs: Double? = null
    var autoplay: Boolean = true
    var volume: Double? = null
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
@TauriPlugin
class RaygunAudioPlugin(private val activity: Activity) : Plugin(activity) {
    private val main = Handler(Looper.getMainLooper())
    private var controller: MediaController? = null
    private val pending = mutableListOf<(MediaController) -> Unit>()
    private var connecting = false
    private var loadId: Int? = null

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

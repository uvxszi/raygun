package app.raygun.audio

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.content.res.Configuration
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

private const val PROGRESS_INTERVAL_MS = 500L
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

    /** Waits for the song's end trim while it plays. */
    private val endWatch = Runnable { controller?.let { checkEnd(it) } }

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
                scheduleEnd(player)
            } else {
                main.removeCallbacks(progressTicker)
                main.removeCallbacks(endWatch)
                reportProgress(player)
                // The pause at an end trim isn't the listener's.
                if (player.playbackState == Player.STATE_READY && !endReported) {
                    report("paused")
                }
            }
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            val player = controller ?: return
            when (playbackState) {
                Player.STATE_BUFFERING -> report("buffering")
                Player.STATE_READY -> reportProgress(player)
                Player.STATE_ENDED -> {
                    reportLoudnessIfHeard()
                    report("ended")
                }
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
            val fadeMs = (args.fadeMs ?: 0.0).toLong()
            val crossfading = fadeMs > 0 && player.isPlaying
            loadId = args.loadId
            endMs = args.endMs
            endReported = false
            main.removeCallbacks(endWatch)
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
            val service = RaygunPlaybackService.instance
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
        withController {
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
        withController {
            it.seekTo((args.positionMs ?: 0.0).toLong())
            endReported = false
            scheduleEnd(it)
        }
        invoke.resolve()
    }

    @Command
    fun setVolume(invoke: Invoke) {
        val args = invoke.parseArgs(VolumeArgs::class.java)
        withController { it.volume = (args.volume ?: 1.0).toFloat() }
        invoke.resolve()
    }

    /** Playback speed (M46.8); Media3 keeps it for the next items. */
    @Command
    fun setRate(invoke: Invoke) {
        val args = invoke.parseArgs(RateArgs::class.java)
        val rate = (args.rate ?: 1.0).coerceIn(0.5, 2.0).toFloat()
        withController { it.setPlaybackSpeed(rate) }
        invoke.resolve()
    }

    @Command
    fun stop(invoke: Invoke) {
        withController {
            loadId = null
            main.removeCallbacks(progressTicker)
            main.removeCallbacks(endWatch)
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
        if (aliases.all { getPermissionState(it) == PermissionState.GRANTED }) {
            invoke.resolve(JSObject().put("granted", true))
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

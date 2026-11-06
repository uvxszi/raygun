package app.raygun.audio

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Process
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import java.io.File
import java.io.FileOutputStream

private const val RATE = 48_000
private const val CHANNELS = 2
private const val FRAME_BYTES = CHANNELS * 2
/** Sound kept from just before a mark, for the time the mark takes to arrive, ms. */
private const val PRE_ROLL_MS = 400

/** A run of recorded sound: 16-bit interleaved samples in [file], from [startMs] in the song. */
private class RecordedPiece(val file: File, val startMs: Double) {
    @Volatile
    var frames: Long = 0
}

/**
 * One recording: Raygun's own playback (AudioPlaybackCaptureConfiguration, Raygun's own UID,
 * media usages) read on a thread of its own. Runs are placed by where the song is ([mark]):
 * samples outside a run are dropped, except the last [PRE_ROLL_MS], which open the next run.
 */
@RequiresApi(Build.VERSION_CODES.Q)
private class RecordingSession(val id: String, private val dir: File, projection: MediaProjection) {
    private val pieces = mutableListOf<RecordedPiece>()
    private var out: FileOutputStream? = null
    private var current: RecordedPiece? = null
    private val preRoll = ByteArray(RATE * FRAME_BYTES * PRE_ROLL_MS / 1000)
    private var preRollFill = 0
    private var preRollAt = 0

    @Volatile
    private var running = true

    @Volatile
    private var error: String? = null
    private val record: AudioRecord
    private val reader: Thread

    init {
        dir.mkdirs()
        val config = AudioPlaybackCaptureConfiguration.Builder(projection)
            .addMatchingUid(Process.myUid())
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(RATE)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build()
        val minimum = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT)
        record = buildRecord(config, format, maxOf(minimum, RATE * FRAME_BYTES / 4) * 2)
        record.startRecording()
        reader = Thread({ read() }, "RaygunRecording").apply { start() }
    }

    // RECORD_AUDIO was granted with the consent ([RaygunAudioPlugin.recordingConsent]).
    @SuppressLint("MissingPermission")
    private fun buildRecord(config: AudioPlaybackCaptureConfiguration, format: AudioFormat, size: Int) =
        AudioRecord.Builder()
            .setAudioFormat(format)
            .setBufferSizeInBytes(size)
            .setAudioPlaybackCaptureConfig(config)
            .build()

    private fun read() {
        val buffer = ByteArray(RATE * FRAME_BYTES / 20)
        while (running) {
            val count = record.read(buffer, 0, buffer.size)
            if (count < 0) {
                error = "Android stopped the recording ($count)"
                break
            }
            if (count > 0) take(buffer, count - count % FRAME_BYTES)
        }
    }

    @Synchronized
    private fun take(buffer: ByteArray, count: Int) {
        val piece = current
        if (piece == null) {
            keepForPreRoll(buffer, count)
            return
        }
        try {
            out?.write(buffer, 0, count)
            piece.frames += (count / FRAME_BYTES).toLong()
        } catch (failure: Exception) {
            error = failure.message ?: "The recording couldn't be written"
            closePiece()
        }
    }

    private fun keepForPreRoll(buffer: ByteArray, count: Int) {
        for (index in 0 until count) {
            preRoll[preRollAt] = buffer[index]
            preRollAt = (preRollAt + 1) % preRoll.size
        }
        preRollFill = minOf(preRoll.size, preRollFill + count)
    }

    /**
     * The song plays from [positionMs] now: a run starts there (with the sound of just before);
     * `null` holds the run (the player buffers or paused).
     */
    @Synchronized
    fun mark(positionMs: Double?) {
        closePiece()
        if (positionMs == null) return
        val kept = minOf(preRollFill, (positionMs * RATE / 1000).toInt() * FRAME_BYTES)
        val file = File(dir, "$id-${pieces.size}.pcm")
        try {
            val stream = FileOutputStream(file)
            // The ring's last `kept` bytes, oldest first.
            val start = (preRollAt - kept + preRoll.size) % preRoll.size
            val head = minOf(kept, preRoll.size - start)
            stream.write(preRoll, start, head)
            if (kept > head) stream.write(preRoll, 0, kept - head)
            val piece = RecordedPiece(file, positionMs - kept / FRAME_BYTES * 1000.0 / RATE)
            piece.frames = (kept / FRAME_BYTES).toLong()
            out = stream
            current = piece
            pieces.add(piece)
        } catch (failure: Exception) {
            error = failure.message ?: "The recording couldn't be written"
        }
        preRollFill = 0
    }

    private fun closePiece() {
        try {
            out?.close()
        } catch (_: Exception) {
        }
        out = null
        current = null
    }

    @Synchronized
    fun status(): JSObject {
        val list = JSArray()
        for (piece in pieces) {
            list.put(
                JSObject()
                    .put("file", piece.file.absolutePath)
                    .put("startMs", piece.startMs)
                    .put("rate", RATE)
                    .put("channels", CHANNELS)
                    .put("frames", piece.frames),
            )
        }
        val failure = error ?: if (PlaybackRecorder.projection == null) "The recording was stopped" else null
        val status = JSObject()
            .put("state", if (failure == null) "playing" else "failed")
            .put("pieces", list)
        failure?.let { status.put("error", it) }
        return status
    }

    fun release() {
        running = false
        runCatching { record.stop() }
        reader.join(1_000)
        record.release()
        synchronized(this) { closePiece() }
    }
}

/**
 * Records Raygun's own playback, for songs only YouTube's player plays (in the webview, where
 * neither Raygun nor its capture player can read the stream): the song plays aloud in real time,
 * and what Raygun plays is recorded. The person allows it first ([RaygunAudioPlugin.recordingConsent]:
 * RECORD_AUDIO and Android's capture question); the consent's MediaProjection is held by
 * [RecordingService] while recordings wait, and given back with [end].
 *
 * Android copies each of Raygun's tracks to the recording before it applies its volume
 * (AudioFlinger tees a track's own samples to the capture), so it records with the media volume
 * down or muted; only the player's own volume (the embed's, kept full) changes it.
 */
object PlaybackRecorder {
    @Volatile
    var projection: MediaProjection? = null
        private set

    private var session: Any? = null

    /** Whether a consent's projection is held. */
    fun ready(): Boolean = projection != null

    internal fun hold(held: MediaProjection) {
        projection = held
    }

    /** The person (or Android) stopped the projection: recordings fail until it's allowed again. */
    internal fun projectionStopped() {
        projection = null
    }

    @Synchronized
    fun start(session: String, dir: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) throw IllegalStateException("Recording needs Android 10")
        val held = projection ?: throw IllegalStateException("Recording isn't allowed")
        (this.session as? RecordingSession)?.release()
        this.session = RecordingSession(session, File(dir), held)
        // The webview plays the song: it keeps playing while Raygun is off screen.
        Handler(Looper.getMainLooper()).post { WebAwake.hold(true) }
    }

    @Synchronized
    fun mark(session: String, positionMs: Double?) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        (this.session as? RecordingSession)?.takeIf { it.id == session }?.mark(positionMs)
    }

    @Synchronized
    fun status(session: String): JSObject {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            (this.session as? RecordingSession)?.takeIf { it.id == session }?.let { return it.status() }
        }
        return JSObject().put("state", "failed").put("error", "The recording stopped")
    }

    @Synchronized
    fun stop(session: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        val running = this.session as? RecordingSession ?: return
        if (running.id != session) return
        running.release()
        this.session = null
        Handler(Looper.getMainLooper()).post { WebAwake.hold(false) }
    }

    /** No recording waits: the projection goes back and its notification with it. */
    fun end(context: Context) {
        val held = projection
        projection = null
        runCatching { held?.stop() }
        context.stopService(Intent(context, RecordingService::class.java))
    }
}

/**
 * Holds the consent's MediaProjection while songs are recorded: Android only hands one to a
 * foreground service of the mediaProjection type, started before it's asked for.
 */
class RecordingService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val code = intent?.getIntExtra(EXTRA_CODE, 0) ?: 0
        @Suppress("DEPRECATION")
        val data: Intent? = intent?.getParcelableExtra(EXTRA_DATA)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || data == null) {
            answer(false)
            stopSelf()
            return START_NOT_STICKY
        }
        try {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification(),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION,
            )
            val manager = getSystemService(MediaProjectionManager::class.java)
            val projection = manager?.getMediaProjection(code, data)
                ?: throw IllegalStateException("Android gave no projection")
            projection.registerCallback(
                object : MediaProjection.Callback() {
                    override fun onStop() {
                        PlaybackRecorder.projectionStopped()
                        stopSelf()
                    }
                },
                Handler(Looper.getMainLooper()),
            )
            PlaybackRecorder.hold(projection)
            answer(true)
        } catch (error: Exception) {
            Log.w("RaygunAudio", "recording couldn't start", error)
            answer(false)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        PlaybackRecorder.projectionStopped()
        super.onDestroy()
    }

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Recording songs", NotificationManager.IMPORTANCE_LOW),
            )
        }
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle("Recording songs as they play")
            .setContentText("Songs that play only in YouTube's player are saved as they play")
            .setOngoing(true)
            .setContentIntent(openRaygunIntent(this, "/downloads?tab=music"))
            .build()
    }

    companion object {
        const val EXTRA_CODE = "app.raygun.audio.RECORDING_CODE"
        const val EXTRA_DATA = "app.raygun.audio.RECORDING_DATA"
        private const val CHANNEL = "recording"
        private const val NOTIFICATION_ID = 7_301

        /** Called with whether the projection is held, once the service has it (or failed). */
        @Volatile
        var onAnswer: ((Boolean) -> Unit)? = null

        private fun answer(held: Boolean) {
            onAnswer?.invoke(held)
            onAnswer = null
        }
    }
}

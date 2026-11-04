package app.raygun.audio

import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import java.io.File
import java.io.FileOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.CopyOnWriteArrayList

/** A run of captured sound: little-endian 16-bit interleaved samples in [file], from [startMs]. */
private class CapturePiece(val file: File, val startMs: Double, val rate: Int, val channels: Int) {
    @Volatile
    var frames: Long = 0
}

/**
 * Copies a stream's decoded sound (M46.49). It's the first of the capture player's audio
 * processors, so it hears the stream's own samples at their own rate before the player's speed
 * applies, however fast it runs. Each run goes to a piece file the Rust side reads as it grows; a
 * flush (a new stream or format) starts the next piece where the last one ended.
 */
@UnstableApi
private class CaptureTap(private val dir: File, private val name: String, fromMs: Double) :
    BaseAudioProcessor() {
    val pieces = CopyOnWriteArrayList<CapturePiece>()
    private var out: FileOutputStream? = null
    private var current: CapturePiece? = null
    private var nextMs = fromMs
    private var scratch = ByteArray(0)

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT &&
            inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT
        ) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val size = inputBuffer.remaining()
        if (size == 0) return
        write(inputBuffer.duplicate().order(ByteOrder.LITTLE_ENDIAN))
        val output = replaceOutputBuffer(size)
        output.put(inputBuffer)
        output.flip()
    }

    @Synchronized
    private fun write(buffer: ByteBuffer) {
        val format = inputAudioFormat
        val channels = format.channelCount.coerceAtLeast(1)
        val piece = current ?: open(format) ?: return
        val bytes: Int
        if (format.encoding == C.ENCODING_PCM_FLOAT) {
            val count = buffer.remaining() / 4
            if (scratch.size < count * 2) scratch = ByteArray(count * 2)
            for (index in 0 until count) {
                val sample = (buffer.getFloat().coerceIn(-1f, 1f) * 32767f).toInt()
                scratch[index * 2] = (sample and 0xff).toByte()
                scratch[index * 2 + 1] = ((sample shr 8) and 0xff).toByte()
            }
            bytes = count * 2
        } else {
            bytes = buffer.remaining()
            if (scratch.size < bytes) scratch = ByteArray(bytes)
            buffer.get(scratch, 0, bytes)
        }
        try {
            out?.write(scratch, 0, bytes)
            piece.frames += (bytes / (2 * channels)).toLong()
        } catch (error: Exception) {
            close()
        }
    }

    @Synchronized
    private fun open(format: AudioProcessor.AudioFormat): CapturePiece? {
        if (format.sampleRate <= 0) return null
        val file = File(dir, "$name-${pieces.size}.pcm")
        return try {
            out = FileOutputStream(file)
            CapturePiece(file, nextMs, format.sampleRate, format.channelCount.coerceAtLeast(1)).also {
                current = it
                pieces.add(it)
            }
        } catch (error: Exception) {
            null
        }
    }

    /** Ends the piece being written; the next starts where it ended. */
    @Synchronized
    fun close() {
        val piece = current ?: return
        try {
            out?.close()
        } catch (_: Exception) {
        }
        out = null
        current = null
        nextMs = piece.startMs + piece.frames * 1000.0 / piece.rate
    }

    @Suppress("OVERRIDE_DEPRECATION")
    override fun onFlush() {
        close()
    }

    override fun onReset() {
        close()
    }
}

/** Puts a [CaptureTap] first in the capture player's audio sink. */
@UnstableApi
private class TapRenderersFactory(context: Context, private val tap: CaptureTap) :
    DefaultRenderersFactory(context) {
    override fun buildAudioSink(
        context: Context,
        enableFloatOutput: Boolean,
        enableAudioTrackPlaybackParams: Boolean,
    ): AudioSink =
        DefaultAudioSink.Builder(context)
            .setAudioProcessors(arrayOf(tap))
            .setEnableFloatOutput(false)
            .build()
}

/**
 * A capture player (M46.49): an ExoPlayer of its own on its own thread, never the one people
 * listen to. It plays the stream muted, without asking for audio focus, at [speed] (the copy is
 * taken before the speed applies), from [fromMs], through the stream proxy that gives playback
 * its headers.
 */
@UnstableApi
private class CaptureSession(
    context: Context,
    val id: String,
    url: String,
    mime: String?,
    fromMs: Double,
    speed: Float,
    dir: File,
) {
    private val thread = HandlerThread("RaygunCapture").apply { start() }
    private val handler = Handler(thread.looper)
    private val tap = CaptureTap(dir, id, fromMs)
    private var player: ExoPlayer? = null

    @Volatile
    private var state = "loading"

    @Volatile
    private var error: String? = null

    @Volatile
    private var durationMs = C.TIME_UNSET

    @Volatile
    private var positionMs = 0L

    private val ticker = object : Runnable {
        override fun run() {
            val player = player ?: return
            positionMs = player.currentPosition
            if (player.duration != C.TIME_UNSET) durationMs = player.duration
            handler.postDelayed(this, 250)
        }
    }

    init {
        dir.mkdirs()
        handler.post {
            try {
                val attributes = AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build()
                val player = ExoPlayer.Builder(context, TapRenderersFactory(context, tap))
                    .setLooper(thread.looper)
                    .setAudioAttributes(attributes, /* handleAudioFocus = */ false)
                    .setHandleAudioBecomingNoisy(false)
                    .setWakeMode(C.WAKE_MODE_NETWORK)
                    .build()
                this.player = player
                player.volume = 0f
                player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
                    .setTrackTypeDisabled(C.TRACK_TYPE_VIDEO, true)
                    .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
                    .build()
                player.playbackParameters = PlaybackParameters(speed)
                player.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        when (playbackState) {
                            Player.STATE_BUFFERING -> state = "buffering"
                            Player.STATE_READY -> state = "playing"
                            Player.STATE_ENDED -> {
                                tap.close()
                                state = "ended"
                            }
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        this@CaptureSession.error = error.message ?: error.errorCodeName
                        state = "failed"
                    }
                })
                val item = MediaItem.Builder().setUri(url)
                if (mime != null) item.setMimeType(mime)
                player.setMediaItem(item.build(), fromMs.toLong())
                player.prepare()
                player.playWhenReady = true
                handler.post(ticker)
            } catch (failure: Exception) {
                error = failure.message ?: "The capture player didn't start"
                state = "failed"
            }
        }
    }

    fun status(): JSObject {
        val pieces = JSArray()
        for (piece in tap.pieces) {
            pieces.put(
                JSObject()
                    .put("file", piece.file.absolutePath)
                    .put("startMs", piece.startMs)
                    .put("rate", piece.rate)
                    .put("channels", piece.channels)
                    .put("frames", piece.frames),
            )
        }
        val status = JSObject()
            .put("state", state)
            .put("positionMs", positionMs)
            .put("pieces", pieces)
        if (durationMs != C.TIME_UNSET && durationMs > 0) status.put("durationMs", durationMs)
        error?.let { status.put("error", it) }
        return status
    }

    fun release() {
        handler.removeCallbacksAndMessages(null)
        handler.post {
            player?.release()
            player = null
            tap.close()
            thread.quitSafely()
        }
    }
}

/** The one capture running (M46.49): a new one stops the last. */
@UnstableApi
object SongCapture {
    private var session: CaptureSession? = null

    @Synchronized
    fun start(context: Context, args: CaptureStartArgs) {
        session?.release()
        session = CaptureSession(
            context.applicationContext,
            args.session,
            args.url,
            args.mime,
            args.fromMs,
            args.speed.toFloat().coerceIn(1f, 8f),
            File(args.dir),
        )
    }

    @Synchronized
    fun status(id: String): JSObject =
        session?.takeIf { it.id == id }?.status()
            ?: JSObject().put("state", "failed").put("error", "The capture stopped")

    @Synchronized
    fun stop(id: String) {
        if (session?.id == id) {
            session?.release()
            session = null
        }
    }
}

package app.raygun.audio

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.os.Build
import android.util.Base64
import android.util.Log
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Plays the speech downloaded voices make (Rust synthesizes a chapter sentence by sentence, a
 * few ahead, and hands each over as 16-bit mono PCM): one clip after another through one
 * [AudioTrack], pausing and going on exactly where it was. Reports go to Rust through
 * [NativeBridge.tts]: `{"state": "speaking", "id"}` as a clip starts being heard, `ended` once the
 * last one has been, `paused` when another app took the sound, `error` when the track failed.
 */
internal class SpeechClips(private val context: Context) {
    private class Clip(val id: String, val samples: ShortArray, val rate: Int, val last: Boolean)

    /** A clip written to the track: where its frames start and end. */
    private class Written(val id: String, val start: Long, val end: Long, val last: Boolean) {
        var reported = false
    }

    private val lock = Object()
    private val queue = ArrayDeque<Clip>()
    private val written = ArrayDeque<Written>()

    /** Increases with each [stop]: a writer of an older one ends. */
    private var generation = 0
    private var writing = false
    private var paused = false
    /** 0–1, for the track playing and those made later. */
    private var volume = 1f
    private var track: AudioTrack? = null
    private var trackRate = 0
    private var framesWritten = 0L
    private var focus: AudioFocusRequest? = null
    private val audio by lazy { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }

    /** Another app took the sound: reading pauses, and Rust hears it. */
    private val focusChange = AudioManager.OnAudioFocusChangeListener { change ->
        if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
            val id = synchronized(lock) { written.firstOrNull()?.id ?: queue.firstOrNull()?.id }
            if (pause() && id != null) report("paused", id)
        }
    }

    /** Queues a clip ([pcm]: base64 16-bit little-endian mono samples at [rate] Hz). */
    fun add(id: String, pcm: String, rate: Int, last: Boolean) {
        val bytes = Base64.decode(pcm, Base64.DEFAULT)
        val samples = ShortArray(bytes.size / 2)
        ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).asShortBuffer().get(samples)
        synchronized(lock) {
            queue.addLast(Clip(id, samples, rate.coerceIn(8_000, 96_000), last))
            if (!writing) {
                writing = true
                val mine = generation
                Thread({ play(mine) }, "raygun-speech").start()
                requestFocus()
            }
            lock.notifyAll()
        }
    }

    /** Holds the clips where they are. Whether anything was playing. */
    fun pause(): Boolean = synchronized(lock) {
        if (paused || (track == null && queue.isEmpty())) return false
        paused = true
        try {
            track?.pause()
        } catch (_: IllegalStateException) {
        }
        true
    }

    /** Goes on where [pause] stopped. Whether there was anything to go on with. */
    fun resume(): Boolean = synchronized(lock) {
        val playing = track
        if (playing == null && queue.isEmpty()) return false
        if (!paused) return true
        paused = false
        requestFocus()
        try {
            playing?.play()
        } catch (_: IllegalStateException) {
            return false
        }
        lock.notifyAll()
        true
    }

    /** Plays the clips at [level] (0–1), at once. */
    fun setVolume(level: Float) = synchronized(lock) {
        volume = level.coerceIn(0f, 1f)
        try {
            track?.setVolume(volume)
        } catch (_: IllegalStateException) {
        }
        Unit
    }

    /** Stops and forgets every clip. */
    fun stop() {
        synchronized(lock) {
            generation += 1
            writing = false
            paused = false
            queue.clear()
            written.clear()
            releaseTrack()
            lock.notifyAll()
        }
        abandonFocus()
    }

    private fun play(mine: Int) {
        try {
            write(mine)
        } catch (error: Exception) {
            Log.w("RaygunAudio", "speech stopped playing", error)
            val id = synchronized(lock) {
                if (generation == mine) written.firstOrNull()?.id ?: queue.firstOrNull()?.id else null
            }
            if (id != null) {
                report("error", id)
                stop()
            }
        } finally {
            synchronized(lock) {
                if (generation == mine) writing = false
            }
        }
    }

    /** Writes the clips into the track in order, reporting as each is heard, until all were. */
    private fun write(mine: Int) {
        var clip: Clip? = null
        var offset = 0
        while (true) {
            var output: AudioTrack? = null
            synchronized(lock) {
                if (generation != mine) return
                heard()
                if (clip == null) clip = queue.removeFirstOrNull()
                val next = clip
                when {
                    next == null -> {
                        // Everything written was heard: this writer ends until the next clip.
                        if (written.isEmpty()) return
                        lock.wait(30)
                    }
                    paused -> lock.wait(30)
                    else -> {
                        output = trackFor(next.rate)
                        if (output != null && offset == 0) {
                            // Silence after the last clip isn't reported.
                            if (next.id.isNotEmpty()) {
                                written.addLast(Written(next.id, framesWritten, framesWritten + next.samples.size, next.last))
                            }
                            framesWritten += next.samples.size
                        }
                    }
                }
            }
            val into = output ?: continue
            val current = clip ?: continue
            val count = try {
                into.write(current.samples, offset, minOf(4_096, current.samples.size - offset), AudioTrack.WRITE_NON_BLOCKING)
            } catch (_: IllegalStateException) {
                -1
            }
            if (count < 0) {
                if (synchronized(lock) { generation != mine }) return
                throw IllegalStateException("the track refused speech ($count)")
            }
            if (count == 0) {
                synchronized(lock) { lock.wait(15) }
                continue
            }
            offset += count
            if (offset >= current.samples.size) {
                // The chapter's last clip is followed by a track's worth of silence, which pushes
                // it out to be heard (a track starts and plays on only with enough to play).
                clip = if (current.last) Clip("", ShortArray(maxOf(into.bufferSizeInFrames, 1)), current.rate, false) else null
                offset = 0
            }
        }
    }

    /**
     * The track for [rate] Hz, made when there is none. A clip at another rate waits (nothing
     * now) while the old track still plays what it holds. Called holding [lock].
     */
    private fun trackFor(rate: Int): AudioTrack? {
        track?.let { if (trackRate == rate) return it }
        if (track != null && written.isNotEmpty()) {
            lock.wait(30)
            return null
        }
        releaseTrack()
        return try {
            val minimum = AudioTrack.getMinBufferSize(rate, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT)
            AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build(),
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .setSampleRate(rate)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                        .build(),
                )
                // About half a second: a pause stops within it.
                .setBufferSizeInBytes(maxOf(minimum, rate))
                .setTransferMode(AudioTrack.MODE_STREAM)
                .build()
                .also {
                    // Speech starts as soon as a little of it is there.
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        try {
                            it.setStartThresholdInFrames(rate / 20)
                        } catch (_: Exception) {
                        }
                    }
                    it.setVolume(volume)
                    if (!paused) it.play()
                    track = it
                    trackRate = rate
                    framesWritten = 0
                }
        } catch (error: Exception) {
            Log.w("RaygunAudio", "speech couldn't play", error)
            throw IllegalStateException("no track for speech", error)
        }
    }

    /** Reports clips that started being heard, and the end of the last. Called holding [lock]. */
    private fun heard() {
        val output = track ?: return
        val position = try {
            output.playbackHeadPosition.toLong() and 0xffffffffL
        } catch (_: IllegalStateException) {
            return
        }
        while (true) {
            val first = written.firstOrNull() ?: return
            if (position < first.start) return
            if (!first.reported) {
                first.reported = true
                report("speaking", first.id)
            }
            if (position < first.end) return
            written.removeFirst()
            if (first.last) report("ended", first.id)
        }
    }

    /** Called holding [lock]. */
    private fun releaseTrack() {
        val old = track ?: return
        track = null
        trackRate = 0
        framesWritten = 0
        try {
            old.pause()
            old.flush()
        } catch (_: IllegalStateException) {
        }
        old.release()
    }

    private fun report(state: String, id: String) {
        NativeBridge.tts(JSONObject().put("state", state).put("id", id).toString())
    }

    private fun requestFocus() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val request = focus ?: AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build(),
                )
                .setOnAudioFocusChangeListener(focusChange)
                .build()
                .also { focus = it }
            audio.requestAudioFocus(request)
        } else {
            @Suppress("DEPRECATION")
            audio.requestAudioFocus(focusChange, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
        }
    }

    private fun abandonFocus() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            focus?.let { audio.abandonAudioFocusRequest(it) }
        } else {
            @Suppress("DEPRECATION")
            audio.abandonAudioFocus(focusChange)
        }
    }
}

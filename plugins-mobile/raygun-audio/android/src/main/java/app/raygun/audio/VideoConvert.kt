package app.raygun.audio

import android.content.Context
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.media.MediaMetadataRetriever
import android.net.Uri
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.ChannelMixingAudioProcessor
import androidx.media3.common.audio.ChannelMixingMatrix
import androidx.media3.common.util.UnstableApi
import androidx.media3.transformer.Composition
import androidx.media3.transformer.EditedMediaItem
import androidx.media3.transformer.EditedMediaItemSequence
import androidx.media3.transformer.Effects
import androidx.media3.transformer.ExportException
import androidx.media3.transformer.ExportResult
import androidx.media3.transformer.ProgressHolder
import androidx.media3.transformer.Transformer
import app.tauri.plugin.JSObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * Video exports re-encoded with the phone's own encoders: Media3's Transformer reads a downloaded
 * video (a file, or a saved HLS playlist with its keys and segments) and writes H.264 and AAC in
 * MP4, or AAC alone, copying a stream that is already that. HDR pictures are tone-mapped to SDR.
 * Rust starts a session, asks how far it is and stops it; everything here runs on the main
 * thread, which Transformer was built on.
 */
@UnstableApi
object VideoConvert {
    private class Session(val output: File) {
        var transformer: Transformer? = null

        @Volatile
        var state = "running"

        @Volatile
        var error: String? = null

        @Volatile
        var result: ExportResult? = null
    }

    private val sessions = ConcurrentHashMap<String, Session>()

    /** Starts converting `input` into `output`; the sound is mixed to stereo when `stereo`. */
    fun start(context: Context, args: ConvertStartArgs) {
        stop(args.session)
        val output = File(args.output)
        output.parentFile?.mkdirs()
        output.delete()
        val session = Session(output)
        sessions[args.session] = session
        val item = MediaItem.Builder().setUri(Uri.fromFile(File(args.input)))
        if (args.input.endsWith(".m3u8")) item.setMimeType(MimeTypes.APPLICATION_M3U8)
        val edited = EditedMediaItem.Builder(item.build()).setRemoveVideo(args.audioOnly)
        // Surround sound is mixed into stereo whenever sound is converted: Transformer has no
        // mixing matrix of its own for more than two channels, and phones' AAC encoders take two.
        if (args.audio || args.stereo) {
            edited.setEffects(Effects(listOf<AudioProcessor>(stereoMixer()), emptyList()))
        }
        val tracks = buildSet {
            if (args.audio) add(C.TRACK_TYPE_AUDIO)
            if (args.video && !args.audioOnly) add(C.TRACK_TYPE_VIDEO)
        }
        val sequence = EditedMediaItemSequence.Builder(tracks).addItem(edited.build()).build()
        val composition = Composition.Builder(sequence)
            .setHdrMode(Composition.HDR_MODE_TONE_MAP_HDR_TO_SDR_USING_OPEN_GL)
            .build()
        val transformer = Transformer.Builder(context)
            .setVideoMimeType(MimeTypes.VIDEO_H264)
            .setAudioMimeType(MimeTypes.AUDIO_AAC)
            .addListener(object : Transformer.Listener {
                override fun onCompleted(composition: Composition, exportResult: ExportResult) {
                    session.result = exportResult
                    session.state = "done"
                }

                override fun onError(
                    composition: Composition,
                    exportResult: ExportResult,
                    exportException: ExportException,
                ) {
                    session.error = describe(exportException)
                    session.state = "failed"
                    output.delete()
                }
            })
            .build()
        session.transformer = transformer
        transformer.start(composition, output.absolutePath)
    }

    /** `{state, progress, error}`: `running`, `done` or `failed`; progress 0 to 100. */
    fun status(id: String): JSObject {
        val session = sessions[id]
            ?: return JSObject().put("state", "failed").put("error", "The conversion isn't running")
        val status = JSObject().put("state", session.state)
        val transformer = session.transformer
        if (session.state == "running" && transformer != null) {
            val holder = ProgressHolder()
            if (transformer.getProgress(holder) == Transformer.PROGRESS_STATE_AVAILABLE) {
                status.put("progress", holder.progress)
            }
        }
        session.error?.let { status.put("error", it) }
        session.result?.let { result ->
            status.put("durationMs", result.approximateDurationMs)
            result.videoEncoderName?.let { status.put("videoEncoder", it) }
            result.audioEncoderName?.let { status.put("audioEncoder", it) }
        }
        if (session.state != "running") sessions.remove(id)
        return status
    }

    /** Stops a conversion and deletes what it wrote. */
    fun stop(id: String) {
        val session = sessions.remove(id) ?: return
        if (session.state == "running") {
            session.transformer?.cancel()
            session.output.delete()
        }
    }

    private fun describe(error: ExportException): String {
        val cause = error.cause?.message?.takeIf { it.isNotBlank() }
        val name = error.errorCodeName
        // A problem with the sound's processing comes as a decoding error: say what it was.
        val sound = generateSequence(error.cause) { it.cause }
            .any { it is AudioProcessor.UnhandledAudioFormatException }
        if (sound) return "The sound couldn't be converted ($name: ${cause ?: "unsupported format"})"
        return when (error.errorCode) {
            ExportException.ERROR_CODE_DECODER_INIT_FAILED,
            ExportException.ERROR_CODE_DECODING_FAILED,
            ExportException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED ->
                "This phone can't decode the video ($name)"
            ExportException.ERROR_CODE_ENCODER_INIT_FAILED,
            ExportException.ERROR_CODE_ENCODING_FAILED,
            ExportException.ERROR_CODE_ENCODING_FORMAT_UNSUPPORTED ->
                "This phone's encoder couldn't convert the video ($name)"
            else -> listOfNotNull(error.message, cause).joinToString(": ").ifEmpty { name }
        }
    }

    /** Mixes surround (and mono) sound into stereo. */
    private fun stereoMixer(): ChannelMixingAudioProcessor {
        val mixer = ChannelMixingAudioProcessor()
        for (channels in 1..8) {
            try {
                mixer.putChannelMixingMatrix(ChannelMixingMatrix.createForConstantGain(channels, 2))
            } catch (_: UnsupportedOperationException) {
            }
        }
        return mixer
    }

    /**
     * What Android's own extractor and decoders find in a file, read on a thread of its own:
     * `{durationMs, video, audio, frame}`: its length and streams as the system reads them,
     * `frame` when a picture frame decodes and `audio` when its sound decodes (with the decoders
     * Raygun's own player uses, in this process).
     */
    fun check(path: String, answer: (JSObject?, String?) -> Unit) {
        Thread {
            try {
                answer(inspect(path), null)
            } catch (error: Exception) {
                answer(null, error.message ?: "The converted file is unreadable")
            }
        }.start()
    }

    private fun inspect(path: String): JSObject {
        val retriever = MediaMetadataRetriever()
        val found = JSObject()
        try {
            retriever.setDataSource(path)
            val duration = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
                ?.toLongOrNull() ?: 0L
            val video = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_HAS_VIDEO) == "yes"
            found.put("durationMs", duration)
            found.put("video", video)
            found.put("frame", video && decodes(path, "video/"))
        } finally {
            retriever.release()
        }
        found.put("audio", decodes(path, "audio/"))
        return found
    }

    /** Whether the file's first stream of a kind (`video/`, `audio/`) decodes a frame. */
    private fun decodes(path: String, kind: String): Boolean {
        val extractor = MediaExtractor()
        var codec: MediaCodec? = null
        try {
            extractor.setDataSource(path)
            val track = (0 until extractor.trackCount).firstOrNull {
                extractor.getTrackFormat(it).getString(MediaFormat.KEY_MIME)?.startsWith(kind) == true
            } ?: return false
            val format = extractor.getTrackFormat(track)
            val mime = format.getString(MediaFormat.KEY_MIME) ?: return false
            extractor.selectTrack(track)
            val decoder = MediaCodec.createDecoderByType(mime)
            codec = decoder
            decoder.configure(format, null, null, 0)
            decoder.start()
            val info = MediaCodec.BufferInfo()
            var ended = false
            repeat(200) {
                if (!ended) {
                    val input = decoder.dequeueInputBuffer(10_000)
                    if (input >= 0) {
                        val buffer = decoder.getInputBuffer(input)
                        val size = if (buffer == null) -1 else extractor.readSampleData(buffer, 0)
                        if (size < 0) {
                            decoder.queueInputBuffer(input, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            ended = true
                        } else {
                            decoder.queueInputBuffer(input, 0, size, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }
                }
                val output = decoder.dequeueOutputBuffer(info, 10_000)
                if (output >= 0) {
                    decoder.releaseOutputBuffer(output, false)
                    if (info.size > 0) return true
                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) return false
                }
            }
            return false
        } catch (_: Exception) {
            return false
        } finally {
            try {
                codec?.stop()
            } catch (_: Exception) {
            }
            codec?.release()
            extractor.release()
        }
    }
}

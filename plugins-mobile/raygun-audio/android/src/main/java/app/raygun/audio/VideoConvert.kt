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
import androidx.media3.common.util.Clock
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.mediacodec.MediaCodecUtil
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.transformer.Composition
import androidx.media3.transformer.DefaultAssetLoaderFactory
import androidx.media3.transformer.DefaultDecoderFactory
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
        // A new composition for each try: Transformer prepares the one it's given.
        val compose = {
            val sequence = EditedMediaItemSequence.Builder(tracks).addItem(edited.build()).build()
            Composition.Builder(sequence)
                .setHdrMode(Composition.HDR_MODE_TONE_MAP_HDR_TO_SDR_USING_OPEN_GL)
                .build()
        }
        launch(context, session, compose, output, softwareFirst = false)
    }

    /**
     * Runs the conversion. A decoder that fails while it runs (some phones' hardware decoders,
     * and the emulator's) gets one more go with the software decoders first; Media3's own
     * fallback only covers decoders that fail to start.
     */
    private fun launch(
        context: Context,
        session: Session,
        compose: () -> Composition,
        output: File,
        softwareFirst: Boolean,
    ) {
        val decoders = DefaultDecoderFactory.Builder(context).setEnableDecoderFallback(true)
        if (softwareFirst) decoders.setMediaCodecSelector(SOFTWARE_FIRST)
        val transformer = Transformer.Builder(context)
            .setAssetLoaderFactory(
                DefaultAssetLoaderFactory(
                    context,
                    decoders.build(),
                    Clock.DEFAULT,
                    DefaultMediaSourceFactory(context),
                    DataSourceBitmapLoader.Builder(context).build(),
                ),
            )
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
                    output.delete()
                    if (!softwareFirst && session.state == "running" && decoding(exportException)) {
                        // After this callback returns: the failed run is still ending.
                        main.post {
                            if (sessions.containsValue(session)) {
                                launch(context, session, compose, output, softwareFirst = true)
                            }
                        }
                        return
                    }
                    session.error = describe(exportException)
                    session.state = "failed"
                }
            })
            .build()
        session.transformer = transformer
        try {
            transformer.start(compose(), output.absolutePath)
        } catch (error: RuntimeException) {
            session.error = error.message ?: error.javaClass.simpleName
            session.state = "failed"
            output.delete()
        }
    }

    private val main = android.os.Handler(android.os.Looper.getMainLooper())

    private fun decoding(error: ExportException) = error.errorCode in setOf(
        ExportException.ERROR_CODE_DECODER_INIT_FAILED,
        ExportException.ERROR_CODE_DECODING_FAILED,
        ExportException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
    )

    /** The platform's decoders with the software ones first. */
    private val SOFTWARE_FIRST = MediaCodecSelector { mimeType, secure, tunneling ->
        MediaCodecUtil.getDecoderInfos(mimeType, secure, tunneling)
            .sortedBy { it.hardwareAccelerated }
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
        for (channels in 1..2) {
            mixer.putChannelMixingMatrix(ChannelMixingMatrix.createForConstantGain(channels, 2))
        }
        for (channels in 3..8) {
            mixer.putChannelMixingMatrix(ChannelMixingMatrix(channels, 2, downmix(channels)))
        }
        return mixer
    }

    /**
     * Stereo from Android's channel orders (3: L R C; 4: L R Ls Rs; 5: L R C Ls Rs; 6: L R C LFE
     * Ls Rs; 7: L R C LFE Cs Ls Rs; 8: L R C LFE Lb Rb Ls Rs): centre and surrounds at -3 dB, the
     * LFE left out, each side scaled so it never clips. Row-major, a row per input channel.
     */
    internal fun downmix(channels: Int): FloatArray {
        val center = floatArrayOf(0.7071f, 0.7071f)
        val left = floatArrayOf(1f, 0f)
        val right = floatArrayOf(0f, 1f)
        val surroundLeft = floatArrayOf(0.7071f, 0f)
        val surroundRight = floatArrayOf(0f, 0.7071f)
        val lfe = floatArrayOf(0f, 0f)
        val rows = when (channels) {
            3 -> listOf(left, right, center)
            4 -> listOf(left, right, surroundLeft, surroundRight)
            5 -> listOf(left, right, center, surroundLeft, surroundRight)
            6 -> listOf(left, right, center, lfe, surroundLeft, surroundRight)
            7 -> listOf(left, right, center, lfe, floatArrayOf(0.5f, 0.5f), surroundLeft, surroundRight)
            else -> listOf(left, right, center, lfe, surroundLeft, surroundRight, surroundLeft, surroundRight)
        }
        val scale = maxOf(1f, rows.sumOf { it[0].toDouble() }.toFloat())
        return FloatArray(channels * 2) { index -> rows[index / 2][index % 2] / scale }
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
    /**
     * Whether any of the phone's decoders reads the file's `kind` track: the first one Android
     * offers, then the others (a hardware decoder can fail where the software one works).
     */
    private fun decodes(path: String, kind: String): Boolean {
        val mime = trackMime(path, kind) ?: return false
        val names = android.media.MediaCodecList(android.media.MediaCodecList.REGULAR_CODECS)
            .codecInfos
            .filter { !it.isEncoder && it.supportedTypes.any { type -> type.equals(mime, true) } }
            .sortedBy { if (android.os.Build.VERSION.SDK_INT >= 29) it.isSoftwareOnly else false }
            .map { it.name }
            .take(4)
        return names.any { decodesWith(path, kind, it) }
    }

    private fun trackMime(path: String, kind: String): String? {
        val extractor = MediaExtractor()
        return try {
            extractor.setDataSource(path)
            (0 until extractor.trackCount)
                .map { extractor.getTrackFormat(it).getString(MediaFormat.KEY_MIME) }
                .firstOrNull { it?.startsWith(kind) == true }
        } catch (_: Exception) {
            null
        } finally {
            extractor.release()
        }
    }

    private fun decodesWith(path: String, kind: String, name: String): Boolean {
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
            val decoder = MediaCodec.createByCodecName(name)
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

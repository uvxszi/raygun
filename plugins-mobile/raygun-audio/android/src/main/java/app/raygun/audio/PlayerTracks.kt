package app.raygun.audio

import android.content.Context
import android.net.Uri
import android.os.Looper
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.text.Cue
import androidx.media3.common.util.Util
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ForwardingRenderer
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.text.TextOutput
import androidx.media3.exoplayer.text.TextRenderer
import androidx.media3.extractor.text.DefaultSubtitleParserFactory
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.ui.SubtitleView
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import kotlin.concurrent.thread

/** Largest subtitle file read for the second track. */
private const val MAX_SUBTITLE_BYTES = 8 * 1024 * 1024

/**
 * Renders subtitles at the playback position minus a delay (CloudStream's approach, M45): a
 * positive delay shows them later. `ForwardingRenderer` passes on everything else, the default
 * methods included.
 */
internal class OffsetTextRenderer(inner: TextRenderer) : ForwardingRenderer(inner) {
    @Volatile
    var delayUs = 0L

    override fun render(positionUs: Long, elapsedRealtimeUs: Long) {
        super.render(positionUs - delayUs, elapsedRealtimeUs)
    }
}

/** The renderers: hardware decoders falling back to software, and the delayable text renderer. */
internal class PlayerRenderers(
    context: Context,
    private val onText: (OffsetTextRenderer) -> Unit,
) : DefaultRenderersFactory(context) {
    override fun buildTextRenderers(
        context: Context,
        output: TextOutput,
        outputLooper: Looper,
        extensionRendererMode: Int,
        out: ArrayList<Renderer>,
    ) {
        val renderer = OffsetTextRenderer(TextRenderer(output, outputLooper))
        onText(renderer)
        out.add(renderer)
    }
}

/** One track of a group. */
internal class TrackRef(val group: Tracks.Group, val index: Int) {
    val format: Format get() = group.getTrackFormat(index)
    val selected: Boolean get() = group.isTrackSelected(index)

    /** Its language and name, to find the same track in the next episode. */
    fun choice(): TrackChoice = TrackChoice(format.language, format.label)
}

/** A subtitle file beside the video: from the source, or added from the device. */
internal data class Sidecar(val uri: Uri, val mime: String, val language: String?, val label: String?) {
    fun choice(): TrackChoice = TrackChoice(language, label)
}

/** "en", "eng" and "en-US" are the same language, and so are "ger", "deu" and "de". */
internal fun sameLanguage(a: String?, b: String?): Boolean {
    if (a.isNullOrBlank() || b.isNullOrBlank()) return false
    return normalLanguage(a) == normalLanguage(b)
}

private fun normalLanguage(tag: String): String =
    (Util.normalizeLanguageCode(tag.replace('_', '-')) ?: tag).substringBefore('-').lowercase(Locale.ROOT)

/** A language's name in the device's language ("German" for "ger"). */
internal fun languageName(tag: String?): String? {
    if (tag.isNullOrBlank() || tag == C.LANGUAGE_UNDETERMINED) return null
    val normal = Util.normalizeLanguageCode(tag.replace('_', '-')) ?: tag
    return Locale.forLanguageTag(normal).displayName.ifBlank { null }
}

private fun hasWord(text: String?, words: List<String>): Boolean {
    val name = text?.lowercase(Locale.ROOT) ?: return false
    return words.any { name.contains(it) }
}

/** The MIME type of a subtitle file from its format or address. */
internal fun subtitleMime(format: String, url: String): String {
    val kind = format.lowercase(Locale.ROOT).ifBlank {
        url.substringBefore('?').substringAfterLast('.').lowercase(Locale.ROOT)
    }
    return when (kind) {
        "vtt", "webvtt" -> MimeTypes.TEXT_VTT
        "ass", "ssa" -> MimeTypes.TEXT_SSA
        "ttml", "dfxp", "xml" -> MimeTypes.APPLICATION_TTML
        else -> MimeTypes.APPLICATION_SUBRIP
    }
}

/**
 * Audio and subtitle tracks (M45): lists for the sheets, choosing one, and choosing by themselves
 * when a video's tracks are known: the title's remembered choice first, else the subtitle
 * languages (skipping tracks named with a skip word and preferring ones with a prefer word) and the
 * audio languages.
 */
internal class PlayerTracks(private val host: VideoPlayerActivity) {
    lateinit var text: OffsetTextRenderer

    fun list(type: Int): List<TrackRef> = host.player.currentTracks.groups
        .filter { it.type == type }
        .flatMap { group -> (0 until group.length).filter { group.isTrackSupported(it) }.map { TrackRef(group, it) } }

    fun textOff(): Boolean =
        host.player.trackSelectionParameters.disabledTrackTypes.contains(C.TRACK_TYPE_TEXT) ||
            list(C.TRACK_TYPE_TEXT).none { it.selected }

    /** Plays `track`, or turns the type off when null. */
    fun select(type: Int, track: TrackRef?) {
        val builder = host.player.trackSelectionParameters.buildUpon()
        if (track == null) {
            builder.setTrackTypeDisabled(type, true)
        } else {
            builder.setTrackTypeDisabled(type, false)
                .setOverrideForType(TrackSelectionOverride(track.group.mediaTrackGroup, track.index))
        }
        host.player.trackSelectionParameters = builder.build()
    }

    /** Before a new video: no choices from the last one, the language preferences in place. */
    fun reset(subtitleLanguages: List<String>, audioLanguages: List<String>) {
        host.player.trackSelectionParameters = host.player.trackSelectionParameters.buildUpon()
            .clearOverridesOfType(C.TRACK_TYPE_TEXT)
            .clearOverridesOfType(C.TRACK_TYPE_AUDIO)
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, false)
            .setPreferredTextLanguages(*subtitleLanguages.toTypedArray())
            .setSelectUndeterminedTextLanguage(subtitleLanguages.isNotEmpty())
            .setPreferredAudioLanguages(*audioLanguages.toTypedArray())
            .build()
    }

    /** Chooses tracks once a video's are known. */
    fun pick(state: TitleState, prefs: PlayerPrefs, subtitleLanguages: List<String>) {
        val subtitles = list(C.TRACK_TYPE_TEXT)
        val remembered = state.subtitle
        when {
            state.subtitleOff -> select(C.TRACK_TYPE_TEXT, null)
            remembered != null -> find(subtitles, remembered)?.let { select(C.TRACK_TYPE_TEXT, it) }
            subtitleLanguages.isNotEmpty() -> {
                val chosen = byLanguage(subtitles, subtitleLanguages, prefs)
                when {
                    chosen != null -> select(C.TRACK_TYPE_TEXT, chosen)
                    // Every track in the language is one to skip ("Signs"): none plays.
                    subtitles.any { track -> subtitleLanguages.any { sameLanguage(track.format.language, it) } } ->
                        select(C.TRACK_TYPE_TEXT, null)
                }
            }
        }
        state.audio?.let { choice -> find(list(C.TRACK_TYPE_AUDIO), choice)?.let { select(C.TRACK_TYPE_AUDIO, it) } }
    }

    private fun byLanguage(tracks: List<TrackRef>, languages: List<String>, prefs: PlayerPrefs): TrackRef? {
        for (language in languages) {
            val candidates = tracks.filter {
                sameLanguage(it.format.language, language) && !hasWord(it.format.label, prefs.skipWords)
            }
            if (candidates.isEmpty()) continue
            return candidates.firstOrNull { hasWord(it.format.label, prefs.preferWords) } ?: candidates.first()
        }
        return null
    }

    /** The track with the same language and name, else the same name, else the same language. */
    private fun find(tracks: List<TrackRef>, choice: TrackChoice): TrackRef? =
        tracks.firstOrNull { sameLanguage(it.format.language, choice.language) && it.format.label == choice.label }
            ?: tracks.firstOrNull { choice.label != null && it.format.label == choice.label }
            ?: tracks.firstOrNull { sameLanguage(it.format.language, choice.language) }

    /** A track's name for the sheets: its label, else its language, else "Track n". */
    fun name(track: TrackRef, ordinal: Int): String {
        val format = track.format
        val language = languageName(format.language)
        // A label that is only the language code ("eng") reads better as the language's name.
        val label = format.label?.takeIf { !sameLanguage(it, format.language) || language == null }
        return label ?: language ?: host.label("track", "Track {n}", ordinal.toString())
    }

    /** Extra facts about an audio track: channels and codec. */
    fun audioDetail(track: TrackRef): String? {
        val format = track.format
        val channels = when (format.channelCount) {
            Format.NO_VALUE -> null
            1 -> "Mono"
            2 -> "Stereo"
            6 -> "5.1"
            8 -> "7.1"
            else -> "${format.channelCount} ch"
        }
        val language = format.language?.takeIf { format.label != null && !sameLanguage(format.label, it) }
            ?.let { languageName(it) }
        return listOfNotNull(language, channels).joinToString(" · ").ifBlank { null }
    }
}

/**
 * A second subtitle track shown at the top of the picture (M45). Media3 plays one text track, so
 * this one is a subtitle file read and parsed here, its cues shown as the position moves.
 */
internal class SecondSubtitles(private val host: VideoPlayerActivity, private val view: SubtitleView) {
    private class Timed(val startUs: Long, val endUs: Long, val cues: List<Cue>)

    private var timed: List<Timed> = emptyList()
    private var generation = 0
    var current: Sidecar? = null
        private set

    val active: Boolean get() = current != null

    fun load(sidecar: Sidecar, onFailed: (String) -> Unit) {
        generation += 1
        val mine = generation
        current = sidecar
        timed = emptyList()
        view.setCues(null)
        thread(name = "raygun-subtitles") {
            val result = runCatching { parse(read(sidecar.uri), sidecar.mime) }
            host.runOnUiThread {
                if (mine != generation) return@runOnUiThread
                result.onSuccess { timed = it }.onFailure {
                    current = null
                    onFailed(it.message ?: it.javaClass.simpleName)
                }
            }
        }
    }

    fun clear() {
        generation += 1
        current = null
        timed = emptyList()
        view.setCues(null)
    }

    /** Shows the cues at `positionMs`, shifted by the subtitle delay, at the top of the picture. */
    fun update(positionMs: Long, delayMs: Long) {
        if (timed.isEmpty()) return
        val at = (positionMs - delayMs) * 1000
        view.setCues(
            timed.filter { at >= it.startUs && at < it.endUs }.flatMap { it.cues }.map { cue ->
                cue.buildUpon()
                    .setLine(0.04f, Cue.LINE_TYPE_FRACTION)
                    .setLineAnchor(Cue.ANCHOR_TYPE_START)
                    .build()
            },
        )
    }

    private fun read(uri: Uri): ByteArray {
        val stream = when (uri.scheme) {
            "http", "https" -> (URL(uri.toString()).openConnection() as HttpURLConnection).apply {
                connectTimeout = 15_000
                readTimeout = 15_000
            }.inputStream
            else -> host.contentResolver.openInputStream(uri) ?: error("unreadable")
        }
        return stream.use { input ->
            val bytes = input.readBytes()
            if (bytes.size > MAX_SUBTITLE_BYTES) error("too large")
            bytes
        }
    }

    private fun parse(bytes: ByteArray, mime: String): List<Timed> {
        val format = Format.Builder().setSampleMimeType(mime).build()
        val factory = DefaultSubtitleParserFactory()
        require(factory.supportsFormat(format)) { "unsupported subtitles" }
        val parser = factory.create(format)
        val found = mutableListOf<Pair<Long, Pair<Long, List<Cue>>>>()
        parser.parse(bytes, SubtitleParser.OutputOptions.allCues()) { cues ->
            found += cues.startTimeUs to (cues.durationUs to cues.cues.toList())
        }
        val sorted = found.sortedBy { it.first }
        return sorted.mapIndexed { index, (start, rest) ->
            val (duration, cues) = rest
            // A cue without a length lasts until the next one.
            val end = if (duration == C.TIME_UNSET) {
                sorted.getOrNull(index + 1)?.first ?: Long.MAX_VALUE
            } else {
                start + duration
            }
            Timed(start, end, cues)
        }
    }
}

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

    /** Its language as a code, from its language or else its name ("English"). */
    val language: String? get() = Languages.code(format.language) ?: Languages.code(format.label)

    /** Marked as the one to play (HLS `DEFAULT`, Matroska's default flag) or as the main track. */
    val original: Boolean
        get() = format.selectionFlags and C.SELECTION_FLAG_DEFAULT != 0 ||
            format.roleFlags and C.ROLE_FLAG_MAIN != 0 ||
            hasWord(format.label, listOf("original"))

    /** Subtitles only for parts in another language, and signs. */
    val forced: Boolean
        get() = format.selectionFlags and C.SELECTION_FLAG_FORCED != 0 || hasWord(format.label, listOf("forced"))

    /** Commentary or audio description: not what plays by itself. */
    val extra: Boolean
        get() = format.roleFlags and (C.ROLE_FLAG_COMMENTARY or C.ROLE_FLAG_DESCRIBES_VIDEO) != 0 ||
            hasWord(format.label, listOf("commentary", "description", "descriptive"))

    /** Its language and name, to find the same track in the next episode. */
    fun choice(): TrackChoice = TrackChoice(format.language ?: language, format.label)
}

/** A subtitle file beside the video: from the source, or added from the device. */
internal data class Sidecar(val uri: Uri, val mime: String, val language: String?, val label: String?) {
    fun choice(): TrackChoice = TrackChoice(language, label)
}

/** "en", "eng", "en-US" and "English" are the same language, and so are "ger", "deu" and "de". */
internal fun sameLanguage(a: String?, b: String?): Boolean {
    val first = Languages.code(a) ?: return false
    return first == Languages.code(b)
}

/** A language's name in the device's language ("German" for "ger"). */
internal fun languageName(tag: String?): String? = Languages.name(tag)

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
 * when a video's tracks are known (mpv's `alang`, `slang` and `subs-with-matching-audio`):
 * - audio: the title's remembered track, else the first audio language the video has (the
 *   device's languages, then the app's, unless Settings → Player → Audio names one), else the
 *   track marked default or original, else the first;
 * - subtitles: the title's remembered choice, else, when the audio isn't in a subtitle language
 *   (or always, as set), the first subtitle language the video has, skipping tracks named with a
 *   skip word and preferring ones with a prefer word. Audio in a subtitle language only brings
 *   its forced subtitles.
 * Media3 is given the audio languages too, so its own first choice is usually the same.
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

    /**
     * Before a new video: no choices from the last one, the audio languages in place, and
     * subtitles off until [pick] decides (Media3 would turn on a track marked default).
     */
    fun reset(languages: LanguagePrefs, maxHeight: Int?) {
        val builder = host.player.trackSelectionParameters.buildUpon()
            .clearOverridesOfType(C.TRACK_TYPE_TEXT)
            .clearOverridesOfType(C.TRACK_TYPE_AUDIO)
            .clearOverridesOfType(C.TRACK_TYPE_VIDEO)
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, false)
            .setPreferredTextLanguages()
            .setPreferredAudioLanguages(*languages.audio.toTypedArray())
        // Automatic quality stays at or below the preferred resolution; with nothing that small
        // Media3 takes the smallest above it. A picture counts by its long side against a 16:9
        // one's (1920×800 is 1080p, as `video_quality::picture_height` in raygun-app counts it).
        // The Quality sheet's overrides go past this.
        if (maxHeight == null) {
            builder.clearVideoSizeConstraints()
        } else {
            val longSide = (maxHeight * 16 + 8) / 9
            builder.setMaxVideoSize(longSide, longSide)
        }
        host.player.trackSelectionParameters = builder.build()
    }

    /** Chooses tracks once a video's are known. */
    fun pick(state: TitleState, prefs: PlayerPrefs, languages: LanguagePrefs) {
        val audio = pickAudio(state, languages)
        if (audio != null && !audio.selected) select(C.TRACK_TYPE_AUDIO, audio)
        val playing = audio ?: list(C.TRACK_TYPE_AUDIO).firstOrNull { it.selected }
        val spoken = playing?.language ?: languages.spoken
        select(C.TRACK_TYPE_TEXT, pickSubtitles(state, prefs, languages, spoken))
    }

    /** The audio track to play; null when there's no choice to make. */
    private fun pickAudio(state: TitleState, languages: LanguagePrefs): TrackRef? {
        val tracks = list(C.TRACK_TYPE_AUDIO)
        if (tracks.size < 2) return null
        state.audio?.let { choice -> find(tracks, choice)?.let { return it } }
        for (language in languages.audio) {
            val matching = tracks.filter { it.language == language }
            if (matching.isEmpty()) continue
            // Not the commentary; Media3's pick among the rest knows the channels and bit rates.
            val plain = matching.filter { !it.extra }.ifEmpty { matching }
            return plain.firstOrNull { it.selected } ?: plain.firstOrNull { it.original } ?: plain.first()
        }
        return tracks.firstOrNull { it.original && !it.extra } ?: tracks.firstOrNull { !it.extra } ?: tracks.first()
    }

    /** The subtitle track to show; null for none. `spoken` is the audio's language. */
    private fun pickSubtitles(state: TitleState, prefs: PlayerPrefs, languages: LanguagePrefs, spoken: String?): TrackRef? {
        val tracks = list(C.TRACK_TYPE_TEXT)
        if (state.subtitleOff) return null
        state.subtitle?.let { choice -> find(tracks, choice)?.let { return it } }
        val understood = spoken == null || spoken in languages.subtitles
        return when {
            languages.subtitleMode == "never" -> null
            languages.subtitleMode == "foreign" && understood ->
                // Only what's said in other languages, and signs.
                spoken?.let { language -> tracks.firstOrNull { it.forced && it.language == language } }
            else -> byLanguage(tracks, languages.subtitles, prefs)
        }
    }

    /** The best full subtitles in the first of `languages` the video has. */
    private fun byLanguage(tracks: List<TrackRef>, languages: List<String>, prefs: PlayerPrefs): TrackRef? {
        for (language in languages) {
            val candidates = tracks.filter {
                it.language == language && !hasWord(it.format.label, prefs.skipWords)
            }
            if (candidates.isEmpty()) continue
            val full = candidates.filter { !it.forced }.ifEmpty { candidates }
            return full.firstOrNull { hasWord(it.format.label, prefs.preferWords) } ?: full.first()
        }
        return null
    }

    /** The track with the same language and name, else the same name, else the same language. */
    private fun find(tracks: List<TrackRef>, choice: TrackChoice): TrackRef? {
        val language = Languages.code(choice.language)
        return tracks.firstOrNull { language != null && it.language == language && it.format.label == choice.label }
            ?: tracks.firstOrNull { choice.label != null && it.format.label == choice.label }
            ?: tracks.firstOrNull { language != null && it.language == language && !it.forced }
    }

    /** The video's qualities, highest first (HLS and DASH variants). */
    fun qualities(): List<TrackRef> = list(C.TRACK_TYPE_VIDEO)
        .filter { it.format.height > 0 || it.format.bitrate > 0 }
        .sortedWith(compareByDescending<TrackRef> { it.format.height }.thenByDescending { it.format.bitrate })

    /** Whether the player picks the quality by itself (no quality chosen). */
    fun qualityAuto(): Boolean =
        host.player.trackSelectionParameters.overrides.keys.none { it.type == C.TRACK_TYPE_VIDEO }

    /** Plays one quality, or lets the player choose by the connection when null. */
    fun selectQuality(track: TrackRef?) {
        val builder = host.player.trackSelectionParameters.buildUpon()
        if (track == null) {
            builder.clearOverridesOfType(C.TRACK_TYPE_VIDEO)
        } else {
            builder.setOverrideForType(TrackSelectionOverride(track.group.mediaTrackGroup, track.index))
        }
        host.player.trackSelectionParameters = builder.build()
    }

    /** "1080p", or the bit rate when the height isn't known. */
    fun qualityName(track: TrackRef): String {
        val format = track.format
        return if (format.height > 0) "${format.height}p" else "${format.bitrate / 1000} kb/s"
    }

    /** A track's name for the sheets: its label, else its language, else "Track n". */
    fun name(track: TrackRef, ordinal: Int): String {
        val format = track.format
        val language = languageName(format.language)
        // A label that only names the language ("eng") reads better as the language's name;
        // "English (Forced)" stays as it is.
        val label = format.label?.takeIf { !Languages.onlyNames(it, format.language) || language == null }
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

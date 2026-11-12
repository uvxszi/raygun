package app.raygun.audio

import android.content.Context
import android.graphics.Rect
import android.net.Uri
import android.os.Looper
import android.text.SpannableString
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import android.view.View
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.TrackSelectionParameters
import androidx.media3.common.Tracks
import androidx.media3.common.text.Cue
import androidx.media3.common.text.LanguageFeatureSpan
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ForwardingRenderer
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.text.TextOutput
import androidx.media3.exoplayer.text.TextRenderer
import androidx.media3.extractor.text.DefaultSubtitleParserFactory
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import kotlin.concurrent.thread
import kotlin.math.roundToInt

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

/**
 * Whether track `index` of `group` counts as chosen. Media3 applies a choice on its playback
 * thread and reports it a moment later (`onTracksChanged`), so until then `selected` still says
 * what played before: the choice asked for wins. Never while its type is turned off (`disabled`);
 * else `override` (the one asked for its type, when its group is still listed) decides; else what
 * Media3 plays.
 */
internal fun isChosen(
    group: Tracks.Group,
    index: Int,
    disabled: Boolean,
    override: TrackSelectionOverride?,
): Boolean = when {
    disabled -> false
    override != null -> override.mediaTrackGroup == group.mediaTrackGroup && index in override.trackIndices
    else -> group.isTrackSelected(index)
}

/**
 * A subtitle file beside the video: from the source, or added from the device. `source` is where
 * it comes from (the addon that found it, its file's name), shown under its name.
 */
internal data class Sidecar(
    val uri: Uri,
    val mime: String,
    val language: String?,
    val label: String?,
    val source: String? = null,
) {
    fun choice(): TrackChoice = TrackChoice(language, label)
}

/** "en", "eng", "en-US" and "English" are the same language, and so are "ger", "deu" and "de". */
internal fun sameLanguage(a: String?, b: String?): Boolean {
    val first = Languages.code(a) ?: return false
    return first == Languages.code(b)
}

/** A language's name in the device's language ("German" for "ger"). */
internal fun languageName(tag: String?): String? = Languages.name(tag)

/**
 * A track's name for the sheets, from its `label` and `language` (`languageName` is the
 * language's name): a label that only names the language reads as the language's name ("eng" is
 * "English"); one that names no language follows the language's name ("English (Forced)" for a
 * torrent's "Film.en.forced.srt", labelled "Forced"); any other stays as it is ("English
 * (Forced)"). Null when there is neither.
 */
internal fun trackName(label: String?, language: String?, languageName: String?): String? {
    val own = label?.trim()?.takeIf { it.isNotEmpty() }
    return when {
        own == null -> languageName
        languageName == null -> own
        Languages.onlyNames(own, language) -> languageName
        Languages.code(own) == null -> "$languageName ($own)"
        else -> own
    }
}

/**
 * `items` with those in the first of `languages` at the top, then the next language's, then the
 * rest; each group keeps its own order (several tracks in one language stay together).
 */
internal fun <T> byLanguages(items: List<T>, languages: List<String>, language: (T) -> String?): List<T> {
    if (languages.isEmpty()) return items
    return items.sortedBy { item ->
        languages.indexOf(Languages.code(language(item))).let { rank -> if (rank < 0) languages.size else rank }
    }
}

/** Media3's edge type for a style's edge. */
internal fun edgeType(edge: String): Int = when (edge) {
    "none" -> CaptionStyleCompat.EDGE_TYPE_NONE
    "shadow" -> CaptionStyleCompat.EDGE_TYPE_DROP_SHADOW
    "raised" -> CaptionStyleCompat.EDGE_TYPE_RAISED
    "depressed" -> CaptionStyleCompat.EDGE_TYPE_DEPRESSED
    else -> CaptionStyleCompat.EDGE_TYPE_OUTLINE
}

/** Whether the style's background is drawn here, in rounded boxes ([RoundedLineBackground]). */
internal fun SubtitleLook.roundedBackground(): Boolean = (backgroundColor ushr 24) != 0

/**
 * Cues in the viewer's style over the file's own:
 * - a text colour or a background picked wins over the colours a file gives its lines (an ASS
 *   style's, a TTML region's), white and "None" too once picked ([SubtitleLook.ownText]);
 * - "Ignore the file's styles" strips them here (Media3's own stripping would take the boxes
 *   below with it), keeping only ruby and the like;
 * - the size setting scales the sizes a file sets;
 * - lines wrap within the widest share of the picture's width ([narrowedBox]);
 * - the background is drawn behind each line in a box with rounded corners.
 * Texts are copied, so nothing of one style stays on a cue when the style changes (Media3's
 * painter adds spans to a `SpannableStringBuilder` it draws).
 */
internal fun restyled(cues: List<Cue>, look: SubtitleLook): List<Cue> {
    val ownText = look.ownText
    val ownBackground = look.ownBackground
    val scale = look.size / 100f
    val widest = look.maxWidth.coerceIn(40, 100) / 100f
    val edge = edgeType(look.edge)
    return cues.map { cue ->
        val source = cue.text
        if (cue.bitmap != null || source == null) return@map cue
        val builder = cue.buildUpon()
        var text: CharSequence = source
        if (look.ignoreEmbedded) {
            if (text is Spanned) {
                val copy = SpannableString(text)
                copy.getSpans(0, copy.length, Any::class.java)
                    .filter { it !is LanguageFeatureSpan }
                    .forEach(copy::removeSpan)
                text = copy
            }
            builder.clearWindowColor()
            builder.setTextSize(Cue.DIMEN_UNSET, Cue.TYPE_UNSET)
        } else {
            if (text is Spanned && (ownText || ownBackground)) {
                val copy = SpannableString(text)
                if (ownText) copy.getSpans(0, copy.length, ForegroundColorSpan::class.java).forEach(copy::removeSpan)
                if (ownBackground) copy.getSpans(0, copy.length, BackgroundColorSpan::class.java).forEach(copy::removeSpan)
                text = copy
            }
            if (ownBackground) builder.clearWindowColor()
            if (scale != 1f && cue.textSizeType != Cue.TYPE_UNSET && cue.textSize != Cue.DIMEN_UNSET) {
                builder.setTextSize(cue.textSize * scale, cue.textSizeType)
            }
        }
        val align = lineAlignment(cue.textAlignment)
        // Vertical text is laid out by its own rules; a cue with a window keeps that box's width.
        if (cue.verticalType == Cue.TYPE_UNSET && (!cue.windowColorSet || ownBackground || look.ignoreEmbedded)) {
            narrowedBox(cue.position, cue.positionAnchor, cue.size, align, widest)?.let { box ->
                builder.setPosition(box.position)
                builder.setPositionAnchor(box.anchor)
                builder.setSize(box.size)
            }
        }
        if (look.roundedBackground() && text.isNotEmpty()) {
            val boxed = SpannableString(text)
            boxed.setSpan(
                RoundedLineBackground(look.backgroundColor, edge, align),
                0,
                boxed.length,
                Spanned.SPAN_INCLUSIVE_INCLUSIVE,
            )
            text = boxed
        }
        builder.setText(text)
        builder.build()
    }
}

/**
 * Where subtitles go on a `width`×`height` window: the part of the picture's `frame` on screen,
 * or (no frame laid out yet, or a sliver of one mid-layout) the picture fitted to the window at
 * its own shape, at 16:9 without a picture. Never the whole window: lines are sized by the
 * area's height, and a portrait window's height would make them several times too large.
 */
internal fun subtitleArea(width: Int, height: Int, frame: View?, pictureWidth: Float, pictureHeight: Float): Rect {
    if (frame != null) {
        val visible = Rect(
            frame.left.coerceAtLeast(0),
            frame.top.coerceAtLeast(0),
            frame.right.coerceAtMost(width),
            frame.bottom.coerceAtMost(height),
        )
        if (visible.width() >= width / 8 && visible.height() >= height / 8) return visible
    }
    val ratio = if (pictureWidth > 0f && pictureHeight > 0f) pictureWidth / pictureHeight else 16f / 9f
    var fitWidth = width
    var fitHeight = (width / ratio).roundToInt()
    if (fitHeight > height) {
        fitHeight = height
        fitWidth = (height * ratio).roundToInt().coerceAtMost(width)
    }
    val left = (width - fitWidth) / 2
    val top = (height - fitHeight) / 2
    return Rect(left, top, left + fitWidth, top + fitHeight)
}

/**
 * The cues of one subtitle view as they came, shown in [look] ([restyled]): a style change shows
 * on the lines on screen at once, not from the next line or track.
 */
internal class StyledCues(private val view: SubtitleView) {
    private var raw: List<Cue> = emptyList()

    var look: SubtitleLook = SubtitleLook()
        set(value) {
            if (field == value) return
            field = value
            view.setCues(restyled(raw, value))
        }

    fun set(cues: List<Cue>?) {
        val next = cues ?: emptyList()
        // The second subtitles ask every tick; the same lines aren't drawn again.
        if (next == raw) return
        raw = next
        view.setCues(restyled(next, look))
    }
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

    /**
     * Whether `track` plays or was just asked for ([isChosen]): a sheet drawn again right after a
     * tap shows the tap's choice, not the one Media3 hasn't let go of yet.
     */
    fun chosen(track: TrackRef): Boolean {
        val parameters = host.player.trackSelectionParameters
        val type = track.group.type
        val listed = host.player.currentTracks.groups
        val override = parameters.overrides.values.firstOrNull { asked ->
            asked.type == type && listed.any { it.mediaTrackGroup == asked.mediaTrackGroup }
        }
        return isChosen(track.group, track.index, parameters.disabledTrackTypes.contains(type), override)
    }

    /** No subtitle track plays or was asked for. */
    fun textOff(): Boolean = list(C.TRACK_TYPE_TEXT).none { chosen(it) }

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
     * subtitles off until [pick] decides (Media3 would turn on a track marked default). With
     * [preferH264] (Settings → Video player → Advanced → Video codec), a stream offering H.264
     * plays it over VP9 and AV1, which some devices decode slowly or not at all; otherwise Media3
     * picks, preferring formats with a hardware decoder.
     */
    fun reset(languages: LanguagePrefs, maxHeight: Int?, preferH264: Boolean) {
        val builder = host.player.trackSelectionParameters.buildUpon()
            .clearOverridesOfType(C.TRACK_TYPE_TEXT)
            .clearOverridesOfType(C.TRACK_TYPE_AUDIO)
            .clearOverridesOfType(C.TRACK_TYPE_VIDEO)
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, false)
            .setPreferredTextLanguages()
            .setPreferredAudioLanguages(*languages.audio.toTypedArray())
        if (preferH264) {
            builder.setPreferredVideoMimeType(MimeTypes.VIDEO_H264)
        } else {
            builder.setPreferredVideoMimeTypes()
        }
        // Automatic quality stays at or below the preferred resolution; with nothing that small
        // Media3 takes the smallest above it. A picture counts by its long side against a 16:9
        // one's (1920×800 is 1080p, as `video_quality::picture_height` in raygun-app counts it).
        // The Quality sheet's overrides go past this.
        limitSize(builder, maxHeight)
        host.player.trackSelectionParameters = builder.build()
    }

    /** A new preferred resolution, for automatic quality from now on. */
    fun limit(maxHeight: Int?) {
        val builder = host.player.trackSelectionParameters.buildUpon()
        limitSize(builder, maxHeight)
        host.player.trackSelectionParameters = builder.build()
    }

    private fun limitSize(builder: TrackSelectionParameters.Builder, maxHeight: Int?) {
        if (maxHeight == null) {
            builder.clearVideoSizeConstraints()
        } else {
            val longSide = (maxHeight * 16 + 8) / 9
            builder.setMaxVideoSize(longSide, longSide)
        }
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

    /** A track's name for the sheets ([trackName]), else "Track n". */
    fun name(track: TrackRef, ordinal: Int): String {
        val format = track.format
        return trackName(format.label, format.language, languageName(format.language))
            ?: host.label("track", "Track {n}", ordinal.toString())
    }

    /**
     * What a sheet knows of `track` to name it apart from the others ([trackFacts], then
     * [distinctTracks]): its name with what its flags add ("Forced", "SDH"), and `detail` under
     * it; "Track n" when it has no name at all.
     */
    fun facts(track: TrackRef, ordinal: Int, detail: String?): TrackFacts {
        val format = track.format
        return trackFacts(format.label, format.language, languageName(format.language), marksOf(format), ::markWord, detail)
            ?: TrackFacts(host.label("track", "Track {n}", ordinal.toString()), detail = detail)
    }

    private fun markWord(mark: TrackMark): String = host.label(mark.key, mark.fallback)

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

/** Cues showing from `startUs` to `endUs` in a subtitle file. */
internal class TimedCues(val startUs: Long, val endUs: Long, val cues: List<Cue>)

/** Subtitle files read and parsed here (Media3's parsers: SubRip, WebVTT, SSA/ASS, TTML). */
internal object SubtitleFiles {
    /** The file at `uri`: a web address (the proxy's) or one picked on the device. */
    fun read(host: VideoPlayerActivity, uri: Uri): ByteArray {
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

    fun parse(bytes: ByteArray, mime: String): List<TimedCues> {
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
            TimedCues(start, end, cues)
        }
    }

    /** A file's lines as `{startMs, endMs, text}` for Rust: what's said, without styling. */
    fun lines(timed: List<TimedCues>): JSONArray {
        val out = JSONArray()
        for (entry in timed) {
            val text = entry.cues.mapNotNull { it.text?.toString()?.trim() }.filter { it.isNotEmpty() }
                .joinToString("\n")
            if (text.isBlank()) continue
            val startMs = (entry.startUs / 1000).coerceAtLeast(0)
            // The last cue without a length shows a few seconds.
            val endMs = if (entry.endUs == Long.MAX_VALUE) startMs + 4_000 else (entry.endUs / 1000).coerceAtLeast(startMs)
            out.put(JSONObject().put("startMs", startMs).put("endMs", endMs).put("text", text))
        }
        return out
    }
}

/** Where the second subtitles' lines start, as a share of the picture's height from its top. */
internal const val SECOND_LINE_TOP = 0.04f

/** A cue at the top of the picture, where the second subtitles show. */
private fun Cue.atTop(): Cue = buildUpon()
    .setLine(SECOND_LINE_TOP, Cue.LINE_TYPE_FRACTION)
    .setLineAnchor(Cue.ANCHOR_TYPE_START)
    .build()

/**
 * A second subtitle track shown at the top of the picture (M45). Media3 plays one text track, so
 * this one is a subtitle file read and parsed here, or one of the [LiveSubtitles] tracks, its
 * cues shown as the position moves.
 */
internal class SecondSubtitles(private val host: VideoPlayerActivity, private val view: StyledCues) {
    private var timed: List<TimedCues> = emptyList()
    private var generation = 0
    var current: Sidecar? = null
        private set
    /** The live track shown second (AI subtitles or a translation), by its id. */
    var liveId: String? = null
        private set

    val active: Boolean get() = current != null || liveId != null

    fun load(sidecar: Sidecar, onFailed: (String) -> Unit) {
        generation += 1
        val mine = generation
        current = sidecar
        liveId = null
        timed = emptyList()
        view.set(null)
        thread(name = "raygun-subtitles") {
            val result = runCatching { SubtitleFiles.parse(SubtitleFiles.read(host, sidecar.uri), sidecar.mime) }
            host.runOnUiThread {
                if (mine != generation) return@runOnUiThread
                result.onSuccess { timed = it }.onFailure {
                    current = null
                    onFailed(it.message ?: it.javaClass.simpleName)
                }
            }
        }
    }

    /** Shows a live track second; its cues come as they're made. */
    fun showLive(id: String) {
        generation += 1
        current = null
        timed = emptyList()
        liveId = id
        view.set(null)
    }

    fun clear() {
        generation += 1
        current = null
        liveId = null
        timed = emptyList()
        view.set(null)
    }

    /** Shows the cues at `positionMs`, shifted by the subtitle delay, at the top of the picture. */
    fun update(positionMs: Long, delayMs: Long) {
        liveId?.let { id ->
            view.set(host.liveTracks.cuesAt(id, positionMs - delayMs).map { it.atTop() })
            return
        }
        if (timed.isEmpty()) return
        val at = (positionMs - delayMs) * 1000
        view.set(together(timed.filter { at >= it.startUs && at < it.endUs }.flatMap { it.cues }).map { it.atTop() })
    }

    /**
     * Cues showing at once as one, their lines one under the other: each is put on the same top
     * line, where they would be drawn one over the other.
     */
    private fun together(cues: List<Cue>): List<Cue> {
        val texts = cues.mapNotNull { cue -> cue.text?.takeIf { it.isNotEmpty() } }
        if (texts.size < 2) return cues
        val joined = SpannableStringBuilder()
        texts.forEachIndexed { index, text ->
            if (index > 0) joined.append('\n')
            joined.append(text)
        }
        return listOf(cues.first { !it.text.isNullOrEmpty() }.buildUpon().setText(joined).build())
    }
}

/** A cue of a live track, in milliseconds. */
internal class LiveCue(val startMs: Long, val endMs: Long, val text: String)

/** Where making a live track stands, as Rust says. */
internal class LiveStatus(json: JSONObject?) {
    val running = json?.optBoolean("running") == true
    val preparing = json?.optBoolean("preparing") == true
    val complete = json?.optBoolean("complete") == true
    /** How far past the viewer it's ready; null once it is to the end. */
    val aheadMs: Long? = json?.number("aheadMs")
    /** Until when it's ready from where the viewer was, to say as the video moves on. */
    private val readyUntilMs: Long? = json?.number("readyUntilMs")
    val done = json?.optInt("done") ?: 0

    /** How far past `positionMs` it's ready; null once it is to the end. */
    fun aheadOf(positionMs: Long): Long? = readyUntilMs?.let { (it - positionMs).coerceAtLeast(0) } ?: aheadMs
    val total = json?.optInt("total") ?: 0
    /** Downloading this device's models first, in percent. */
    val downloading: Int? = json?.number("downloading")?.toInt()
    val error: String? = json?.text("error")
    /** Who translates (an AI provider's name). */
    val by: String? = json?.text("by")
}

/** A number that may be missing or null. */
internal fun JSONObject.number(key: String): Long? = if (has(key) && !isNull(key)) optLong(key) else null

/** A subtitle track Rust makes as the video plays: the video's AI subtitles, or a translation. */
internal class LiveTrack(val id: String, val kind: String) {
    var language: String? = null
    var sourceLabel: String? = null
    var status = LiveStatus(null)
    /** In time order. */
    val cues = ArrayList<LiveCue>()

    /** Listed with the subtitles once it has lines or is being made, not before. */
    val shown: Boolean
        get() = cues.isNotEmpty() || status.running || status.preparing || status.downloading != null

    fun merge(list: JSONArray?, replace: Boolean) {
        if (replace) cues.clear()
        val known = cues.mapTo(HashSet()) { it.startMs to it.text }
        for (index in 0 until (list?.length() ?: 0)) {
            val cue = list?.optJSONObject(index) ?: continue
            val text = cue.optString("text").trim()
            val start = cue.optLong("startMs")
            if (text.isEmpty() || !known.add(start to text)) continue
            cues += LiveCue(start, cue.optLong("endMs").coerceAtLeast(start), text)
        }
        cues.sortWith(compareBy({ it.startMs }, { it.endMs }))
    }

    /**
     * The cue showing at `positionMs`: the last one to start, until it ends. One shows at a
     * time: a stretch heard again (the viewer went back, a window met a line cut at the end of
     * the one before) brings a cue over one already here, and both at once would be drawn one
     * on the other.
     */
    fun at(positionMs: Long): LiveCue? {
        // The first cue starting after the position; the one before it is the last to start.
        var low = 0
        var high = cues.size
        while (low < high) {
            val middle = (low + high) / 2
            if (cues[middle].startMs <= positionMs) low = middle + 1 else high = middle
        }
        return cues.getOrNull(low - 1)?.takeIf { positionMs < it.endMs }
    }
}

/**
 * AI subtitles in the player: the video transcribed as it plays, and subtitles translated into
 * another language ahead of the viewer. Rust makes them (`video_subtitles` in raygun-app) and
 * sends their cues as they're done (`subtitleTrack` updates), so the player shows these tracks
 * itself rather than through Media3, which only reads a file once: as the main subtitles (Media3's
 * text track off) or as the second ones ([SecondSubtitles]). `subtitleTools` says whether
 * transcription is set up and who translates into what.
 */
internal class LiveSubtitles(private val host: VideoPlayerActivity) {
    /** AI subtitles first, then translations, as they came. */
    val tracks = LinkedHashMap<String, LiveTrack>()
    /** Rust's `subtitleTools`: `transcription` and `translation`. */
    var tools: JSONObject? = null
        private set
    /** The live track shown as the main subtitles, by id. */
    var primary: String? = null
        private set
    /** The language translations go into (this video's choice, else the subtitle language). */
    var language: String? = null
    /** The last translation asked for, sent again once its models may be downloaded. */
    var lastTranslation: JSONObject? = null
    private var videoId = ""
    /** Shown first when its cues come: what the viewer just asked for. */
    private var wantAi = false
    private var wantLanguage: String? = null
    private var shown: LiveCue? = null

    private val ticker = object : Runnable {
        override fun run() {
            render()
            if (primary != null) host.main.postDelayed(this, RENDER_MS)
        }
    }

    val transcription: JSONObject? get() = tools?.optJSONObject("transcription")
    val translation: JSONObject? get() = tools?.optJSONObject("translation")

    /** A video loaded: another one's tracks go, and Rust says what this one has. */
    fun onLoad(id: String) {
        if (id != videoId) {
            tracks.clear()
            show(null)
            if (host.second.liveId != null) host.second.clear()
            tools = null
            language = null
            lastTranslation = null
            wantAi = false
            wantLanguage = null
            videoId = id
        }
        if (id.isNotEmpty()) host.request("subtitleTools")
    }

    fun onUpdate(update: JSONObject) {
        if (update.optString("id") != videoId || videoId.isEmpty()) return
        when (update.optString("type")) {
            "subtitleTools" -> tools = update
            "subtitleTrack" -> {
                val id = update.optString("track")
                val track = tracks.getOrPut(id) { LiveTrack(id, update.optString("kind")) }
                track.language = update.text("language") ?: track.language
                track.sourceLabel = update.text("sourceLabel") ?: track.sourceLabel
                track.status = LiveStatus(update.optJSONObject("status"))
                track.merge(update.optJSONArray("cues"), update.optBoolean("replace"))
                if (track.kind == "ai" && wantAi) {
                    wantAi = false
                    show(id)
                } else if (track.kind == "translation" && wantLanguage != null && track.language == wantLanguage) {
                    wantLanguage = null
                    show(id)
                }
            }
            "subtitleAsk" -> host.sheets.askDownload(update.optDouble("downloadBytes").toLong())
            "subtitleNotice" -> {
                // What was asked for didn't start: nothing new is shown by itself.
                wantAi = false
                wantLanguage = null
                host.showHint(update.optString("text"), 4_000)
            }
        }
        host.sheets.refreshAi()
    }

    /** Shows `id` as the main subtitles (Media3's text track off), or none. */
    fun show(id: String?) {
        primary = id
        shown = null
        host.main.removeCallbacks(ticker)
        host.mainCues.set(null)
        if (id != null) {
            host.tracks.select(C.TRACK_TYPE_TEXT, null)
            host.main.post(ticker)
        }
    }

    private fun render() {
        val id = primary ?: return
        val track = tracks[id]
        // Media3's own track turned on again (picked after a reload): it shows instead.
        if (track == null || !host.tracks.textOff()) {
            primary = null
            if (track == null) host.mainCues.set(null)
            return
        }
        // A live stream's cues are timed by its own clock (`PlayerLive.clockMs`).
        val now = track.at(host.liveStream.clockMs() - host.state.subtitleDelayMs)
        if (now === shown) return
        shown = now
        host.mainCues.set(listOfNotNull(now).map { Cue.Builder().setText(it.text).build() })
    }

    /** The cue of live track `id` at `positionMs` (one at a time), for the second subtitles. */
    fun cuesAt(id: String, positionMs: Long): List<Cue> =
        listOfNotNull(tracks[id]?.at(positionMs)).map { Cue.Builder().setText(it.text).build() }

    /** Asks Rust to transcribe from here; the AI subtitles show as they come. */
    fun transcribe(confirmed: Boolean = false) {
        wantAi = true
        host.request("transcribe", JSONObject().put("confirmed", confirmed))
    }

    /** Downloads the speech model, sets transcription up with it, then transcribes. */
    fun setUpTranscription() {
        wantAi = true
        host.request("transcribeSetup")
    }

    /** Stops the speech model's download that [setUpTranscription] started. */
    fun cancelTranscriptionSetUp() {
        wantAi = false
        host.request("transcribeSetupCancel")
    }

    /** Asks Rust to translate `source` into `language` with `translator`. */
    fun translate(source: JSONObject, language: String, translator: String, download: Boolean = false) {
        val ask = JSONObject()
            .put("source", source)
            .put("language", language)
            .put("translator", translator)
            .put("download", download)
        lastTranslation = ask
        wantLanguage = language
        host.request("translateSubtitles", ask)
    }

    /** The last translation again, its models downloaded first. */
    fun translateWithDownload() {
        val ask = lastTranslation ?: return
        ask.put("download", true)
        wantLanguage = ask.optString("language")
        host.request("translateSubtitles", ask)
    }

    fun stop(track: LiveTrack) {
        if (track.kind == "ai") {
            host.request("transcribeStop")
        } else {
            host.request("translateStop", JSONObject().put("track", track.id))
        }
    }

    /** The saved AI subtitles file of this video, which the live AI track replaces while shown. */
    fun isAiFile(sidecar: Sidecar): Boolean = sidecar.uri.toString().contains("/local/subtitles/ai/")

    /** Whether the live AI subtitles are listed (lines, or being made). */
    val aiListed: Boolean get() = tracks[AI_TRACK]?.shown == true

    /** Whether Media3's `track` is the saved AI subtitles file, hidden while the live one shows. */
    fun hides(track: TrackRef): Boolean {
        if (!aiListed) return false
        val file = host.sidecars.firstOrNull { isAiFile(it) } ?: return false
        return track.format.label == file.label
    }

    companion object {
        const val AI_TRACK = "ai"
        /** How often the main live subtitles follow the position. */
        private const val RENDER_MS = 100L
    }
}

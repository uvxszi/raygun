package app.raygun.audio

import android.graphics.Typeface
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import java.util.Locale

// Subtitle fonts, and names that tell a video's tracks apart in the player's sheets.

/**
 * The subtitle fonts offered (`SubtitleFont` in raygun-model, `SUBTITLE_FONTS` in the app's
 * `settings/playerOptions.ts`), in the order Android's own caption settings list them.
 */
internal val SUBTITLE_FONTS = listOf(
    "default", "sans", "condensed", "mono", "serif", "serifMono", "casual", "cursive", "smallCaps",
)

/**
 * The system family a subtitle font is drawn in: the names of the system's font configuration
 * that Android's own caption settings use. Null for the default typeface; a family a device
 * lacks draws as its default sans serif.
 */
internal fun subtitleFamily(font: String): String? = when (font) {
    "sans" -> "sans-serif"
    "condensed" -> "sans-serif-condensed"
    "mono" -> "monospace"
    "serif" -> "serif"
    "serifMono" -> "serif-monospace"
    "casual" -> "casual"
    "cursive" -> "cursive"
    "smallCaps" -> "sans-serif-smallcaps"
    else -> null
}

/** The typeface for subtitles in `font`, with `style` (`Typeface.BOLD`, `ITALIC`) applied. */
internal fun subtitleTypeface(font: String, style: Int): Typeface = Typeface.create(subtitleFamily(font), style)

/** A font's name in the player's words. */
internal fun VideoPlayerActivity.fontName(font: String): String = when (font) {
    "sans" -> label("fontSans", "Sans serif")
    "condensed" -> label("fontCondensed", "Condensed")
    "mono" -> label("fontMono", "Monospace")
    "serif" -> label("fontSerif", "Serif")
    "serifMono" -> label("fontSerifMono", "Serif monospace")
    "casual" -> label("fontCasual", "Casual")
    "cursive" -> label("fontCursive", "Cursive")
    "smallCaps" -> label("fontSmallCaps", "Small capitals")
    else -> label("fontDefault", "Default")
}

/**
 * What a track is besides its language, said after its name ("English (Forced)"): its key and
 * English word in the player's words, and the words a name says it with already.
 */
internal enum class TrackMark(val key: String, val fallback: String, private val words: List<String>) {
    FORCED("markForced", "Forced", listOf("forced")),
    SDH("markSdh", "SDH", listOf("sdh", "cc", "hi", "hoh")),
    CC("markCc", "CC", listOf("cc", "sdh", "hi", "hoh")),
    COMMENTARY("markCommentary", "Commentary", listOf("commentary")),
    DESCRIPTION("markDescription", "Audio description", listOf("description", "descriptive", "ad"));

    /** Whether `name` says it already: in English, or in the viewer's `word`. */
    fun saidIn(name: String, word: String = fallback): Boolean = (words + word).any { mentions(name, it) }
}

/** Whether `text` holds `word` as a word of its own, in any case. */
internal fun mentions(text: String, word: String): Boolean {
    val plain = word.trim()
    if (plain.isEmpty()) return false
    val pattern = Regex("(?<![\\p{L}\\p{N}])" + Regex.escape(plain) + "(?![\\p{L}\\p{N}])", RegexOption.IGNORE_CASE)
    return pattern.containsMatchIn(text)
}

/** Closed captions carried in the picture's own stream (CEA-608 and 708). */
private val CLOSED_CAPTIONS = setOf(MimeTypes.APPLICATION_CEA608, MimeTypes.APPLICATION_CEA708)

/**
 * What a track's flags say it is: forced (only what's said in other languages, and signs),
 * subtitles for the deaf and hard of hearing (closed captions when the picture carries them),
 * commentary, audio description.
 */
internal fun marksOf(format: Format): List<TrackMark> {
    val marks = mutableListOf<TrackMark>()
    if (format.selectionFlags and C.SELECTION_FLAG_FORCED != 0) marks += TrackMark.FORCED
    // Subtitles parsed as they load keep their own type as the codec.
    if (format.sampleMimeType in CLOSED_CAPTIONS || format.codecs in CLOSED_CAPTIONS) {
        marks += TrackMark.CC
    } else if (format.roleFlags and C.ROLE_FLAG_DESCRIBES_MUSIC_AND_SOUND != 0) {
        marks += TrackMark.SDH
    }
    if (format.roleFlags and C.ROLE_FLAG_COMMENTARY != 0) marks += TrackMark.COMMENTARY
    if (format.roleFlags and C.ROLE_FLAG_DESCRIBES_VIDEO != 0) marks += TrackMark.DESCRIPTION
    return marks
}

/** What a sheet knows of a track to name it. */
internal data class TrackFacts(
    /** Its name so far ([trackName]): "English", "English (Forced)", "Sintel.srt". */
    val name: String,
    /** Words for what else it is, said after the name unless it says them ([TrackMark]). */
    val marks: List<String> = emptyList(),
    /** What goes under the name: where it comes from (an addon, a file, the video), or an audio track's channels. */
    val detail: String? = null,
)

/** A track as a sheet lists it. */
internal data class ShownTrack(val name: String, val detail: String?)

/** A subtitle file's own name ("Film.en.srt"), rather than a track's name. */
private val FILE_NAME = Regex("\\.(srt|vtt|webvtt|ass|ssa|ttml|dfxp|sub|sbv|smi|txt)$", RegexOption.IGNORE_CASE)

/**
 * A track's facts from its `label`, `language` (with that language's name), the `marks` its
 * flags give and where it comes from (`source`): a label that is a file's name ("Film.en.srt",
 * a file added here) goes under the language's name as where it comes from. `word` gives a
 * mark's word in the viewer's language. Null when it has no name at all.
 */
internal fun trackFacts(
    label: String?,
    language: String?,
    languageName: String?,
    marks: List<TrackMark>,
    word: (TrackMark) -> String,
    source: String?,
): TrackFacts? {
    val file = label?.trim()?.takeIf { FILE_NAME.containsMatchIn(it) }
    val name = trackName(if (file != null) null else label, language, languageName) ?: file ?: return null
    val unsaid = marks.distinct().filter { mark -> !mark.saidIn(name, word(mark)) }.map(word)
    return TrackFacts(name, unsaid, source ?: file)
}

/** `name` with the `marks` it doesn't say yet, in brackets: "English (Forced)", "English (Signs, SDH)". */
internal fun withMarks(name: String, marks: List<String>): String {
    val unsaid = marks.map { it.trim() }.filter { it.isNotEmpty() && !mentions(name, it) }.distinct()
    if (unsaid.isEmpty()) return name
    val words = unsaid.joinToString(", ")
    return if (name.endsWith(")") && name.contains(" (")) "${name.dropLast(1)}, $words)" else "$name ($words)"
}

/**
 * Names that tell `tracks` apart, in their order:
 * - each name gets the marks it doesn't say yet ([withMarks]);
 * - names still alike are numbered, the first kept: "English", "English 2", "English 3";
 * - what goes under a name stays, unless it is that name again (a file named only by its file).
 * `numbered` words a name with its number ("{n} {of}" in the player's words).
 */
internal fun distinctTracks(tracks: List<TrackFacts>, numbered: (String, Int) -> String): List<ShownTrack> {
    val named = tracks.map { withMarks(it.name, it.marks) }
    val taken = named.mapTo(HashSet()) { it.lowercase(Locale.ROOT) }
    val seen = HashMap<String, Int>()
    return tracks.mapIndexed { index, track ->
        val base = named[index]
        val key = base.lowercase(Locale.ROOT)
        val count = (seen[key] ?: 0) + 1
        seen[key] = count
        var name = base
        if (count > 1) {
            var number = count
            name = numbered(base, number)
            while (name.lowercase(Locale.ROOT) in taken) {
                number += 1
                name = numbered(base, number)
            }
            taken += name.lowercase(Locale.ROOT)
        }
        val detail = track.detail?.trim()?.takeIf { it.isNotEmpty() && !it.equals(track.name, ignoreCase = true) }
        ShownTrack(name, detail)
    }
}

/** The id a subtitle file's track has in Media3, from its place among the video's files. */
internal fun sidecarId(index: Int): String = "raygun-file-$index"

/**
 * The place among the video's files of the subtitle file a track came from, by its id; null for
 * the video's own tracks. Media3 puts the source's place in front of a merged track's id
 * ("1:raygun-file-0").
 */
internal fun sidecarIndex(id: String?): Int? =
    id?.let { Regex("(?:^|:)raygun-file-(\\d+)$").find(it) }?.groupValues?.get(1)?.toIntOrNull()

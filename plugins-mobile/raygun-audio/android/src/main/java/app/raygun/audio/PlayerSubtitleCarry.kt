package app.raygun.audio

import org.json.JSONObject

// The subtitle choice carried from one title to the next: a title remembers its own choice
// (`TitleState`); one without a choice of its own starts from the last choice made in any title
// (`video.lastSubtitles` in raygun-app), matched by language and kind rather than by name, as
// another title's tracks are named its own way.

/** What a subtitle track is besides its language: `TrackChoice.kind`. */
internal object SubtitleKind {
    const val PLAIN = "plain"
    /** Only what's said in other languages, and signs. */
    const val FORCED = "forced"
    /** For the deaf and hard of hearing (closed captions too): the sounds said as well. */
    const val SDH = "sdh"

    /** A track's kind from its flags (`forced`, `hearing`) and its name's words. */
    fun of(forced: Boolean, hearing: Boolean, label: String?): String = when {
        forced || (label != null && TrackMark.FORCED.saidIn(label)) -> FORCED
        hearing || (label != null && TrackMark.SDH.saidIn(label)) -> SDH
        else -> PLAIN
    }

    /** A saved kind as one of the three; choices saved before kinds were kept read as plain. */
    fun read(kind: String?): String = when (kind) {
        FORCED, SDH -> kind
        else -> PLAIN
    }
}

/** A subtitle track as the carried choice compares it: its language code, name and kind. */
internal data class SubtitleCandidate(val language: String?, val label: String?, val kind: String)

/**
 * The track among `tracks` matching a choice made in another title: the same language (it must
 * have one) and, within it, the same kind, the same name first. Full subtitles and SDH ones stand
 * in for each other when the video lacks the kind chosen; forced ones never stand in for full
 * ones, nor full ones for forced. Null when nothing matches.
 */
internal fun carriedMatch(tracks: List<SubtitleCandidate>, choice: TrackChoice): Int? {
    val language = Languages.code(choice.language) ?: return null
    val kind = SubtitleKind.read(choice.kind)
    val same = tracks.withIndex().filter { (_, track) -> Languages.code(track.language) == language }
    fun of(wanted: String) = same.filter { (_, track) -> track.kind == wanted }
    val stand = when (kind) {
        SubtitleKind.PLAIN -> SubtitleKind.SDH
        SubtitleKind.SDH -> SubtitleKind.PLAIN
        else -> null
    }
    val ordered = of(kind) + (stand?.let(::of) ?: emptyList())
    return (ordered.firstOrNull { (_, track) -> choice.label != null && track.label == choice.label } ?: ordered.firstOrNull())
        ?.index
}

/**
 * The last subtitle choice a title without its own starts from: `off` when subtitles were turned
 * off, else the track chosen (null: none known); `secondary` for the second subtitles. Each part
 * applies only when the title has no choice of that part (`own`).
 */
internal class CarriedSubtitles(own: JSONObject?, last: JSONObject?) {
    private val ownSubtitle = own?.has("subtitle") == true
    private val ownSecondary = own?.has("secondarySubtitle") == true

    val off: Boolean = !ownSubtitle && last?.optJSONObject("subtitle")?.optBoolean("off") == true
    val subtitle: TrackChoice? = if (ownSubtitle || off) null else TrackChoice.from(last?.optJSONObject("subtitle"))
    val secondary: TrackChoice? = if (ownSecondary) null else TrackChoice.from(last?.optJSONObject("secondarySubtitle"))
}

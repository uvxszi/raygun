package app.raygun.audio

import android.content.res.Resources
import java.util.Locale
import java.util.MissingResourceException

/** `audioLanguage` for the video's own track (`ORIGINAL_AUDIO` in raygun-model). */
internal const val ORIGINAL_AUDIO = "original"

/**
 * Languages as streams and sources write them, brought to one code each (`languageCode`): ISO
 * 639-1, 639-2/T and /B and 639-3 codes, BCP 47 tags with a region or script ("en-US", "pt_BR",
 * "zh-Hant"), OpenSubtitles' own ("pob", "spn"), and names or labels ("English",
 * "Español (Latinoamérica)", "Japanese - Original", "ENG SDH", "日本語").
 */
internal object Languages {
    /** A whole value that is a code or a tag: "en", "eng", "en-us", "zh-hant-tw". */
    private val CODE_SHAPE = Regex("^[a-z]{2,3}(-[a-z0-9]{1,8})*$")
    private val BRACKETS = Regex("[(\\[{][^)\\]}]*[)\\]}]")
    private val WORD = Regex("\\p{L}+")
    /** A code written in capitals inside a label: "ENG SDH", "[JPN]". */
    private val CAPITAL_CODE = Regex("(?<![\\p{L}])[A-Z]{2,3}(?![\\p{L}])")
    /** Capital words in labels that are codes of languages but rarely mean them (SDH, HI, SD). */
    private val NOT_LANGUAGES = setOf("sd", "hi", "sdh", "ad", "cc", "tv")

    /** No language, several, or not said. */
    private val UNDETERMINED = setOf("und", "zxx", "mul", "mis", "qaa", "xx", "unknown", "none")

    /** Bibliographic ISO 639-2 codes and older or regional codes, to the code used here. */
    private val ALIASES = mapOf(
        "alb" to "sq", "arm" to "hy", "baq" to "eu", "bur" to "my", "chi" to "zh", "cze" to "cs",
        "dut" to "nl", "fre" to "fr", "geo" to "ka", "ger" to "de", "gre" to "el", "ice" to "is",
        "mac" to "mk", "mao" to "mi", "may" to "ms", "per" to "fa", "rum" to "ro", "slo" to "sk",
        "tib" to "bo", "wel" to "cy",
        // Codes Java and old streams still use.
        "iw" to "he", "in" to "id", "ji" to "yi", "jw" to "jv", "mo" to "ro",
        // Bokmål and Nynorsk read as Norwegian; Tagalog as Filipino.
        "nb" to "no", "nn" to "no", "nob" to "no", "nno" to "no", "tl" to "fil", "tgl" to "fil",
        "sh" to "sr", "scc" to "sr", "scr" to "hr",
        // Chinese spoken varieties that are written as Chinese, and OpenSubtitles' codes.
        "cmn" to "zh", "zht" to "zh", "zhs" to "zh", "zhe" to "zh", "ze" to "zh",
        "pob" to "pt", "pom" to "pt", "spn" to "es", "spl" to "es", "sea" to "es",
    )

    /** Names that Locale doesn't give but labels use. */
    private val EXTRA_NAMES = mapOf(
        "castilian" to "es", "castellano" to "es", "latino" to "es", "brazilian" to "pt",
        "mandarin" to "zh", "cantonese" to "yue", "farsi" to "fa", "flemish" to "nl",
        "filipino" to "fil", "tagalog" to "fil", "bokmål" to "no", "nynorsk" to "no",
    )

    private val twoLetter: Set<String> by lazy {
        Locale.getISOLanguages().map { ALIASES[it] ?: it }.toSet() + setOf("fil", "yue")
    }

    /** ISO 639-2/T codes to the two-letter one ("deu" → "de"). */
    private val threeLetter: Map<String, String> by lazy {
        buildMap {
            for (code in Locale.getISOLanguages()) {
                val three = try {
                    Locale.forLanguageTag(code).isO3Language
                } catch (_: MissingResourceException) {
                    ""
                }
                if (three.length == 3) put(three, ALIASES[code] ?: code)
            }
        }
    }

    /** Lower-case language names in English, in the device's language and in their own. */
    private val names: Map<String, String> by lazy {
        buildMap {
            putAll(EXTRA_NAMES)
            val device = Locale.getDefault()
            for (raw in Locale.getISOLanguages().toList() + listOf("fil", "yue")) {
                val code = ALIASES[raw] ?: raw
                val locale = Locale.forLanguageTag(raw)
                for (inLanguage in listOf(Locale.ENGLISH, device, locale)) {
                    val name = locale.getDisplayLanguage(inLanguage).lowercase(inLanguage)
                    if (name.length > 1 && name != raw && name !in this) put(name, code)
                }
            }
        }
    }

    /** Builds the tables off the main thread before the first video's tracks need them. */
    fun warm() {
        names.size
        threeLetter.size
        twoLetter.size
    }

    /** The language `text` names, as a primary code ("en", "pt", "fil"); null when unknown. */
    fun code(text: String?): String? {
        val raw = text?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val lower = raw.lowercase(Locale.ROOT).replace('_', '-')
        if (lower in UNDETERMINED) return null
        // A code or tag of a known language; "Dub" or "SDH" as a whole is none.
        if (CODE_SHAPE.matches(lower)) return canonical(lower.substringBefore('-'))?.takeIf { it in twoLetter }
        // A name or a label: the whole of it without what's in brackets, then word by word.
        val plain = lower.replace(BRACKETS, " ").trim()
        names[plain]?.let { return it }
        val words = WORD.findAll(plain).map { it.value }.toList()
        words.firstNotNullOfOrNull { names[it] }?.let { return it }
        // Names in scripts without spaces ("简体中文").
        if (words.any { word -> word.any { it.code > 0x2E80 } }) {
            names.entries.firstOrNull { (name, _) -> name.length > 1 && name.any { it.code > 0x2E80 } && plain.contains(name) }
                ?.let { return it.value }
        }
        return CAPITAL_CODE.findAll(raw)
            .map { it.value.lowercase(Locale.ROOT) }
            .filter { it !in NOT_LANGUAGES }
            .mapNotNull { canonical(it) }
            .firstOrNull { it in twoLetter }
    }

    private fun canonical(primary: String): String? {
        if (primary in UNDETERMINED) return null
        val code = ALIASES[primary] ?: if (primary.length == 3) threeLetter[primary] ?: primary else primary
        return ALIASES[code] ?: code
    }

    /** A label that only names its track's language ("eng", "en-US", "English"), nothing more. */
    fun onlyNames(label: String, language: String?): Boolean {
        val lower = label.trim().lowercase(Locale.ROOT).replace('_', '-')
        val bare = CODE_SHAPE.matches(lower) || names[lower] != null
        return bare && code(label) != null && code(label) == code(language)
    }

    /** The device's languages, in the order chosen in the system settings. */
    fun device(): List<String> {
        val locales = Resources.getSystem().configuration.locales
        return (0 until locales.size()).mapNotNull { code(locales[it].toLanguageTag()) }.distinct()
    }

    /** A language's name in the device's language ("German" for "ger", "English" for "English"). */
    fun name(text: String?): String? {
        val code = code(text) ?: return null
        return Locale.forLanguageTag(code).getDisplayLanguage(Locale.getDefault())
            .takeIf { it.isNotBlank() && !it.equals(code, ignoreCase = true) }
            ?.replaceFirstChar { it.titlecase(Locale.getDefault()) }
    }
}

/**
 * Which languages tracks are chosen in: the audio's in order (empty plays the video's own), the
 * subtitles', and when subtitles turn on by themselves (`SubtitleMode` in raygun-model).
 */
internal class LanguagePrefs(
    val audio: List<String>,
    val subtitles: List<String>,
    /** `foreign`, `always` or `never`. */
    val subtitleMode: String,
    /** The audio's language when a stream doesn't say (Japanese for anime). */
    val spoken: String?,
) {
    companion object {
        fun from(
            prefs: PlayerPrefs,
            appLanguage: String,
            subtitleLanguage: String,
            subtitleMode: String,
            spoken: String?,
        ): LanguagePrefs {
            // The device's languages, then the app's.
            val yours = (Languages.device() + listOfNotNull(Languages.code(appLanguage))).distinct()
            val audio = when (prefs.audioLanguage) {
                "" -> yours + listOfNotNull(Languages.code(prefs.audioFallback))
                ORIGINAL_AUDIO -> emptyList()
                else -> listOfNotNull(Languages.code(prefs.audioLanguage), Languages.code(prefs.audioFallback))
            }
            val firstSubtitles = if (subtitleLanguage.isBlank()) yours else listOfNotNull(Languages.code(subtitleLanguage))
            val subtitles = firstSubtitles + listOfNotNull(Languages.code(prefs.subtitleFallback))
            return LanguagePrefs(audio.distinct(), subtitles.distinct(), subtitleMode, Languages.code(spoken))
        }
    }
}

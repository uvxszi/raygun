package app.raygun.audio

import org.json.JSONObject

/** Smart Subtitles' word for a track: like the others of its language, or unlike all of them. */
internal enum class SubtitleFit { MATCHING, MIGHT_NOT_MATCH }

/**
 * Smart Subtitles' order and marks for a video's subtitle files (Rust's `subtitleRanks`), by the
 * address the player loads each from: its place among its language's files (0 first) and its
 * mark. Empty while Smart Subtitles is off.
 */
internal class SmartRanks(private val ranks: Map<String, Pair<Int, SubtitleFit?>>) {
    fun rank(url: String): Int? = ranks[url]?.first

    fun fit(url: String): SubtitleFit? = ranks[url]?.second

    val isEmpty: Boolean get() = ranks.isEmpty()

    companion object {
        val NONE = SmartRanks(emptyMap())

        fun from(update: JSONObject): SmartRanks {
            if (!update.optBoolean("on")) return NONE
            val items = update.optJSONArray("items") ?: return NONE
            val ranks = mutableMapOf<String, Pair<Int, SubtitleFit?>>()
            for (index in 0 until items.length()) {
                val item = items.optJSONObject(index) ?: continue
                val url = item.optString("url").ifBlank { null } ?: continue
                val fit = when (item.optString("fit")) {
                    "matching" -> SubtitleFit.MATCHING
                    "mightNotMatch" -> SubtitleFit.MIGHT_NOT_MATCH
                    else -> null
                }
                ranks[url] = item.optInt("rank", Int.MAX_VALUE) to fit
            }
            return SmartRanks(ranks)
        }
    }
}

/**
 * `items` with Smart Subtitles' order inside each language (the most like the others first):
 * languages keep their places (each language's first item sets it), and items without a rank stay
 * after the ranked ones, in the order they had.
 */
internal fun <T> smartOrder(items: List<T>, language: (T) -> String?, rank: (T) -> Int?): List<T> {
    if (items.none { rank(it) != null }) return items
    val languageAt = mutableMapOf<String, Int>()
    items.forEachIndexed { index, item -> languageAt.putIfAbsent(language(item).orEmpty(), index) }
    return items.withIndex()
        .sortedWith(
            compareBy<IndexedValue<T>>(
                { languageAt[language(it.value).orEmpty()] ?: 0 },
                { rank(it.value) ?: Int.MAX_VALUE },
                { it.index },
            ),
        )
        .map { it.value }
}

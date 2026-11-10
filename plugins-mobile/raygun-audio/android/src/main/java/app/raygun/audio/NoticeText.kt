package app.raygun.audio

/**
 * The first of `lines` that says something, trimmed; null when none does. Android shows a
 * notification without a title or text as an empty one, so every notification Raygun posts takes
 * its lines through this, with a fallback that says what's running.
 */
internal fun firstText(vararg lines: CharSequence?): String? =
    lines.firstNotNullOfOrNull { line -> line?.toString()?.trim()?.takeIf { it.isNotEmpty() } }

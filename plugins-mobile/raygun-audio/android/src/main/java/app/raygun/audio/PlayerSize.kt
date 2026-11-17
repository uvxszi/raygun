package app.raygun.audio

import android.content.Context
import android.content.res.Configuration
import org.json.JSONObject

/**
 * The app's Interface size and Text size (Settings → Appearance) for the video player's own
 * views: every dp (icons, buttons, bars, sheets) scales with the interface, text with both, as
 * the app's pages do. Rust sends them with each request (`uiScale`, `textScale`); the plugin
 * keeps them here before it starts the player, whose base context then carries them
 * ([VideoPlayerActivity.attachBaseContext]).
 */
internal object PlayerSize {
    /** Interface size as a factor, as the app's phone range allows (70–150 %). */
    @Volatile
    var interfaceScale: Float = 1f
        private set

    /** Text size as a factor over the interface's (−30 to +100 %). */
    @Volatile
    var textScale: Float = 1f
        private set

    /** Takes the sizes a request carries; requests without them leave the last ones. */
    fun take(request: JSONObject) {
        if (request.has("uiScale")) interfaceScale = clampInterface(request.optDouble("uiScale", 1.0).toFloat())
        if (request.has("textScale")) textScale = clampText(request.optDouble("textScale", 1.0).toFloat())
    }

    fun clampInterface(value: Float): Float = if (value.isFinite()) value.coerceIn(0.7f, 1.5f) else 1f

    fun clampText(value: Float): Float = if (value.isFinite()) value.coerceIn(0.7f, 2f) else 1f

    /** The density a screen of `densityDpi` has at Interface size `scale` (whole dpi). */
    fun scaledDensityDpi(densityDpi: Int, scale: Float): Int = Math.round(densityDpi * scale)

    /** The font scale: the system's times Text size (sp already follow the scaled density). */
    fun scaledFontScale(systemFontScale: Float, text: Float): Float = systemFontScale * text

    /** `base` with the sizes applied; `base` itself at 100 % and no text change. */
    fun wrap(base: Context): Context {
        val scale = interfaceScale
        val text = textScale
        if (scale == 1f && text == 1f) return base
        val current = base.resources.configuration
        // Only these two: everything left undefined (orientation, sizes) follows the system as the
        // window turns or resizes.
        val config = Configuration().apply {
            densityDpi = scaledDensityDpi(current.densityDpi, scale)
            fontScale = scaledFontScale(current.fontScale, text)
        }
        return base.createConfigurationContext(config)
    }
}

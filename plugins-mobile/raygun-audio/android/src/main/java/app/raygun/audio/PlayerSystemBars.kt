package app.raygun.audio

import android.graphics.Color
import android.os.Build
import android.view.Window
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

/**
 * The status and navigation bars over the video player: shown with the controls (or a sheet),
 * hidden for the picture otherwise, a swipe from an edge bringing them in for a moment.
 *
 * The window lays out under the bars at all times (edge to edge, cutout `SHORT_EDGES`), so their
 * coming and going never resizes or moves the picture or the subtitles; the controls and sheets
 * keep the bars' room whether they show or not (`SafeInsets.pad(hiddenBars = true)`).
 */
internal class PlayerSystemBars(private val window: Window) {
    private val controller = WindowInsetsControllerCompat(window, window.decorView)
    private var shown: Boolean? = null

    init {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.VANILLA_ICE_CREAM) {
            @Suppress("DEPRECATION")
            window.statusBarColor = Color.TRANSPARENT
            @Suppress("DEPRECATION")
            window.navigationBarColor = Color.TRANSPARENT
        }
        controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        // Light icons over the dark picture and shade.
        controller.isAppearanceLightStatusBars = false
        controller.isAppearanceLightNavigationBars = false
    }

    /** Shows or hides the bars; `again` re-applies the same state (after the window regains focus). */
    fun apply(wanted: Boolean, again: Boolean = false) {
        if (!again && shown == wanted) return
        shown = wanted
        if (wanted) {
            controller.show(WindowInsetsCompat.Type.systemBars())
        } else {
            controller.hide(WindowInsetsCompat.Type.systemBars())
        }
    }

    companion object {
        /**
         * Whether the bars show: while the controls show (not locked, where only the unlock
         * button is up) or a sheet is open, never in picture-in-picture.
         */
        fun wanted(controlsShown: Boolean, locked: Boolean, sheetOpen: Boolean, pip: Boolean): Boolean =
            !pip && (sheetOpen || (controlsShown && !locked))
    }
}

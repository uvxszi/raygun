package app.raygun.audio

import android.view.View
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Keeps controls clear of what the system draws over a window: the status bar, the navigation
 * bar or gesture area and a camera cutout, in every orientation (in landscape the cutout and a
 * button navigation bar sit at the sides).
 */
internal object SafeInsets {
    /**
     * Pads [view] by what the system bars and a cutout cover of it. With [hiddenBars], bars hidden
     * for a full screen keep their room, so controls stay clear when they swipe in; with
     * [keyboard], the on-screen keyboard is kept clear too.
     */
    fun pad(view: View, hiddenBars: Boolean = false, keyboard: Boolean = false) {
        ViewCompat.setOnApplyWindowInsetsListener(view) { padded, insets ->
            val area = of(insets, hiddenBars, keyboard)
            padded.setPadding(area.left, area.top, area.right, area.bottom)
            insets
        }
        ViewCompat.requestApplyInsets(view)
    }

    /** What the system bars and a cutout (with [keyboard], the keyboard too) cover, px. */
    fun of(insets: WindowInsetsCompat, hiddenBars: Boolean = false, keyboard: Boolean = false): Insets {
        val barTypes = WindowInsetsCompat.Type.systemBars()
        val bars = if (hiddenBars) insets.getInsetsIgnoringVisibility(barTypes) else insets.getInsets(barTypes)
        val area = Insets.max(bars, insets.getInsets(WindowInsetsCompat.Type.displayCutout()))
        return if (keyboard) Insets.max(area, insets.getInsets(WindowInsetsCompat.Type.ime())) else area
    }
}

/**
 * The video player's controls: clear of the bars (hidden while it plays, their room kept) and a
 * cutout, and each button names itself on a hold. Called once its views are in place.
 */
internal fun VideoPlayerActivity.fitControls(root: View, controls: View) {
    SafeInsets.pad(controls, hiddenBars = true)
    ControlTooltips.follow(root)
}

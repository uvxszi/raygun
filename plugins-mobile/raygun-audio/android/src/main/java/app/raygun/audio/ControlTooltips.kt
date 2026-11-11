package app.raygun.audio

import android.os.Build
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.TextView
import androidx.appcompat.widget.TooltipCompat
import androidx.media3.ui.TimeBar
import java.util.WeakHashMap

/**
 * Names for icon buttons, as the page's have them (shell/ControlLabels): holding a button (or
 * resting a mouse on it) shows its content description in the system's tooltip, and the release
 * after the name showed doesn't press it. Buttons with a long press of their own keep it, and a
 * button without a description, or one whose text says it, shows nothing.
 */
internal object ControlTooltips {
    /** The buttons named so far, with the name each was last given. */
    private val named = WeakHashMap<View, String?>()

    /** Names the buttons under [root], now and as views come and go or change descriptions. */
    fun follow(root: View) {
        name(root)
        root.viewTreeObserver.addOnGlobalLayoutListener { name(root) }
    }

    private fun name(view: View) {
        // A page's own presses are its own.
        if (view is WebView) return
        if (view is ViewGroup) for (index in 0 until view.childCount) name(view.getChildAt(index))
        if (!view.isClickable || view is TimeBar) return
        val name = nameOf(view)?.toString()
        if (!named.containsKey(view)) {
            // Nothing to say (yet), or a long press of its own (a menu, a hold): left as it is.
            if (name == null || view.isLongClickable) return
            named[view] = null
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                // Read again as the press becomes a hold (play and pause swap descriptions); not
                // handled here, so the system shows the tooltip and the release clicks nothing.
                view.setOnLongClickListener { held ->
                    held.tooltipText = nameOf(held)
                    false
                }
            }
        }
        if (named[view] == name) return
        named[view] = name
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            view.tooltipText = name
        } else {
            TooltipCompat.setTooltipText(view, name)
        }
    }

    private fun nameOf(view: View): CharSequence? {
        val description = view.contentDescription?.takeIf { it.isNotBlank() } ?: return null
        val text = (view as? TextView)?.text?.toString()?.trim()
        return if (text == description.toString().trim()) null else description
    }
}

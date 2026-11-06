package app.raygun.audio

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import java.lang.ref.WeakReference

/**
 * Keeps Raygun's webview running while its YouTube player plays (or the next song is being found)
 * and Raygun isn't on screen.
 *
 * When the activity pauses, wry calls `WebView.onPause()`, and when it stops, the window turns
 * invisible: either way Chromium hides the page and pauses its video elements, so YouTube's
 * embed stops a moment after the screen goes off. While [keepAwake] holds, the webview isn't
 * paused (`MainActivity` keeps wry's `onPause` from reaching it: even a moment hidden stops
 * YouTube's player for good) or is resumed again, and is told its window is still visible; the
 * media session service keeps the process in the foreground meanwhile. Once it's let go, the
 * webview gets the real state back.
 *
 * The app's `MainActivity` wraps the webview in a [WebAwakeLayout] (`setContentView`), reads
 * [keepAwake] as it pauses, and calls [paused] and [resumed]; everything else is decided here.
 */
object WebAwake {
    private val main = Handler(Looper.getMainLooper())
    private var layout: WeakReference<WebAwakeLayout>? = null
    private var activityPaused = false

    /** The webview must keep playing while Raygun isn't on screen. Main thread only. */
    var keepAwake = false
        private set

    /** What Rust's player asked for ([set]), and whether a recording plays in the webview. */
    private var player = false
    private var recording = false

    /** A song recorded as it plays in the webview (PlaybackRecorder) keeps it awake too. */
    fun hold(on: Boolean) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            main.post { hold(on) }
            return
        }
        recording = on
        apply(player || recording)
    }

    /** Wraps the activity's content (the webview) so its window's visibility can be held. */
    @JvmStatic
    fun host(context: Context, content: View): View {
        val host = WebAwakeLayout(context)
        host.addView(
            content,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        layout = WeakReference(host)
        return host
    }

    /** After the activity's `onPause` (wry paused the webview): undone while it must keep playing. */
    @JvmStatic
    fun paused() {
        activityPaused = true
        if (keepAwake) webView()?.onResume()
    }

    @JvmStatic
    fun resumed() {
        activityPaused = false
    }

    /** Called with every state Rust's player reports. */
    fun set(awake: Boolean) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            main.post { set(awake) }
            return
        }
        player = awake
        apply(player || recording)
    }

    private fun apply(awake: Boolean) {
        if (awake == keepAwake) return
        keepAwake = awake
        val webView = webView()
        if (activityPaused) {
            if (awake) webView?.onResume() else webView?.onPause()
        }
        layout?.get()?.refresh()
    }

    private fun webView(): WebView? = layout?.get()?.let { host ->
        (0 until host.childCount).map(host::getChildAt).firstOrNull { it is WebView } as WebView?
    }
}

/** The webview's parent: passes its window's visibility on, or "visible" while [WebAwake] holds. */
class WebAwakeLayout(context: Context) : FrameLayout(context) {
    private var windowVisibility = View.VISIBLE

    override fun dispatchWindowVisibilityChanged(visibility: Int) {
        windowVisibility = visibility
        super.dispatchWindowVisibilityChanged(shown())
    }

    /** Hands the children the visibility that applies now. */
    fun refresh() {
        super.dispatchWindowVisibilityChanged(shown())
    }

    private fun shown(): Int = if (WebAwake.keepAwake) View.VISIBLE else windowVisibility
}

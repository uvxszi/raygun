package app.raygun.player

import android.app.Activity
import android.webkit.WebView
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.ScriptHandler
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.util.Locale

/**
 * Tells the page its safe area: how far the status bar, the navigation bar or gesture area and a
 * camera cutout reach into the webview from each edge (in landscape the cutout and a button
 * navigation bar sit at the sides), as `--safe-top`, `--safe-right`, `--safe-bottom` and
 * `--safe-left` (CSS px) on the page's root, which styles.css lays the shell out by. The webview's
 * own `env(safe-area-inset-*)` doesn't count the system bars in every version. The page hears it
 * on every change, and before its own scripts run on each load.
 */
internal class SafeArea(private val activity: Activity) {
    private var webView: WebView? = null
    private var startScript: ScriptHandler? = null
    private var told: List<Float>? = null
    private var latest: WindowInsetsCompat? = null

    /** The webview wry hands over: its pages hear of the safe area from now on. */
    fun attach(view: WebView) {
        detach()
        webView = view
        latest?.let(::update)
    }

    fun detach() {
        startScript?.remove()
        startScript = null
        webView = null
        told = null
    }

    /** The window's insets, as they change. */
    fun update(insets: WindowInsetsCompat) {
        latest = insets
        val view = webView ?: return
        val covered = insets.getInsets(
            WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
        )
        // The keyboard lifts the webview above it (MainActivity.resizeForKeyboard): the navigation
        // bar under the keyboard is no longer the page's.
        val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
        val density = activity.resources.displayMetrics.density
        val area = listOf(
            covered.top,
            covered.right,
            (covered.bottom - keyboard).coerceAtLeast(0),
            covered.left,
        ).map { it / density }
        if (area == told) return
        told = area
        val script = script(area)
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            startScript?.remove()
            startScript = WebViewCompat.addDocumentStartJavaScript(view, script, setOf("*"))
        }
        view.evaluateJavascript(script, null)
    }

    private fun script(area: List<Float>): String {
        val (top, right, bottom, left) = area.map { String.format(Locale.ROOT, "%.1f", it) }
        // Only Raygun's own page (frames inside it, such as YouTube's player, have their own);
        // at a load's start the root may not be there yet.
        return """
            (function () {
              if (window.top !== window) return;
              var area = { top: $top, right: $right, bottom: $bottom, left: $left };
              window.__raygunSafeArea = area;
              var apply = function () {
                var root = document.documentElement;
                if (!root) return false;
                for (var side in area) root.style.setProperty('--safe-' + side, area[side] + 'px');
                return true;
              };
              if (!apply()) document.addEventListener('readystatechange', apply, { once: true });
            })();
        """.trimIndent()
    }
}

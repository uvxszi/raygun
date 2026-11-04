package app.raygun.audio

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.TypedValue
import android.view.Gravity
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity

/**
 * A visible browser for passing a source's browser check by hand (experimental, off unless the
 * person turns it on; the flow is described in `raygun_ext_host::browser_check`). Raygun never
 * touches the page: it only shows it, with the User-Agent sources send. It runs in its own
 * process (`:check`) with its own WebView data, and finishes when the person closes it, or once a
 * page that isn't a check follows one, answering the site's cookies to the plugin's
 * `browserCheck` command.
 */
class BrowserCheckActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_URL = "app.raygun.audio.CHECK_URL"
        const val EXTRA_USER_AGENT = "app.raygun.audio.CHECK_USER_AGENT"
        const val EXTRA_COOKIES = "app.raygun.audio.CHECK_COOKIES"

        /** The same titles as `raygun_ext_host::browser_check::looks_like_challenge`. */
        private val CHALLENGE_TITLES = listOf(
            "just a moment",
            "attention required",
            "checking your browser",
            "checking if the site connection is secure",
            "verify you are human",
            "verifying you are human",
            "security check",
            "ddos-guard",
            "please wait",
            "one more step",
            "captcha",
            "making sure you're not a bot",
        )

        fun looksLikeChallenge(title: String?): Boolean {
            val lower = title?.trim()?.lowercase() ?: return false
            return CHALLENGE_TITLES.any { lower.contains(it) }
        }

        private var dataReady = false
    }

    private val main = Handler(Looper.getMainLooper())
    private lateinit var webView: WebView
    private var url = ""
    private var sawCheck = false
    private var done = false

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        if (!dataReady && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            dataReady = true
            runCatching { WebView.setDataDirectorySuffix("check") }
        }
        super.onCreate(savedInstanceState)
        url = intent.getStringExtra(EXTRA_URL).orEmpty()
        val host = runCatching { Uri.parse(url).host }.getOrNull().orEmpty()

        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setBackgroundColor(PANEL)
        }
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            val pad = dp(12)
            setPadding(pad, dp(8), pad, dp(8))
        }
        val title = TextView(this).apply {
            text = if (host.isEmpty()) "Pass the check, then tap Done" else "$host: pass the check, then tap Done"
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            maxLines = 2
        }
        val close = TextView(this).apply {
            text = "Done"
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(16), dp(8), dp(4), dp(8))
            isClickable = true
            setOnClickListener { finishWithCookies() }
        }
        bar.addView(title, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        bar.addView(close)
        webView = WebView(this)
        column.addView(bar)
        column.addView(webView, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        setContentView(column)

        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            intent.getStringExtra(EXTRA_USER_AGENT)?.takeIf { it.isNotBlank() }?.let { userAgentString = it }
        }
        CookieManager.getInstance().apply {
            setAcceptCookie(true)
            setAcceptThirdPartyCookies(webView, true)
        }
        webView.webChromeClient = object : WebChromeClient() {
            override fun onReceivedTitle(view: WebView, title: String?) = follow(title)
        }
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView, url: String?) = follow(view.title)
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (webView.canGoBack()) webView.goBack() else finishWithCookies()
            }
        })
        if (url.startsWith("https://") || url.startsWith("http://")) {
            webView.loadUrl(url)
        } else {
            finishWithCookies()
        }
    }

    /** A check's page, then one that isn't: the check was passed. */
    private fun follow(title: String?) {
        if (looksLikeChallenge(title)) {
            sawCheck = true
        } else if (sawCheck && !title.isNullOrBlank()) {
            main.postDelayed({ finishWithCookies() }, 800)
        }
    }

    private fun finishWithCookies() {
        if (done) return
        done = true
        val cookies = CookieManager.getInstance().run {
            flush()
            getCookie(url).orEmpty()
        }
        setResult(RESULT_OK, Intent().putExtra(EXTRA_COOKIES, cookies))
        finish()
    }

    private fun dp(value: Int): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value.toFloat(), resources.displayMetrics).toInt()

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        if (::webView.isInitialized) webView.destroy()
        super.onDestroy()
    }
}

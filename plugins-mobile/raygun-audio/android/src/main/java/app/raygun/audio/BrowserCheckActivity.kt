package app.raygun.audio

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Bitmap
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
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONArray
import org.json.JSONObject

/**
 * A visible browser for passing a source's browser check by hand (experimental, off unless the
 * person turns it on; the flow is described in `raygun_ext_host::browser_check`). Raygun never
 * touches the page: it opens the request that met the check as the source sent it (its address,
 * headers, a POST's body) with the source's User-Agent, so the same check shows, and only watches
 * what the pages are. It runs in its own process (`:check`) with its own WebView data, and
 * finishes when the person closes it, or once a page that isn't a check follows one. It answers
 * the address's cookies, its User-Agent and what it saw of its pages (`Seen` in
 * `raygun_ext_host::browser_check`) to the plugin's `browserCheck` command.
 */
class BrowserCheckActivity : AppCompatActivity() {
    companion object {
        /** The request to open, as JSON: `{url, method, headers, body, userAgent}`. */
        const val EXTRA_REQUEST = "app.raygun.audio.CHECK_REQUEST"
        const val EXTRA_COOKIES = "app.raygun.audio.CHECK_COOKIES"
        /** The User-Agent the browser used. */
        const val EXTRA_USER_AGENT = "app.raygun.audio.CHECK_USER_AGENT"
        /** What the browser saw of its pages, in order, as a JSON array. */
        const val EXTRA_SEEN = "app.raygun.audio.CHECK_SEEN"

        /** Reports kept: a check that reloads itself again and again stays bounded. */
        private const val MAX_SEEN = 200

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
    private lateinit var hint: TextView
    private var url = ""
    private var host = ""
    private val seen = JSONArray()

    // The page showing now, and whether it's a check: its answer said so (`cf-mitigated`, in any
    // language), or its title is a check's.
    private var pageUrl: String? = null
    private var pageCheck = false
    private var pageFinished = false
    private var pageFailed = false
    /** A page's answer that said it's a check before the page started. */
    private var earlyCheck: String? = null
    private var sawCheck = false
    private var done = false

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        CrashLog.install(this)
        if (!dataReady && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            dataReady = true
            runCatching { WebView.setDataDirectorySuffix("check") }
        }
        super.onCreate(savedInstanceState)
        val request = runCatching { JSONObject(intent.getStringExtra(EXTRA_REQUEST).orEmpty()) }
            .getOrElse { JSONObject() }
        url = request.optString("url")
        host = runCatching { Uri.parse(url).host }.getOrNull().orEmpty()

        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(PANEL)
        }
        // Clear of the system bars, a camera cutout (at a side in landscape) and the keyboard.
        SafeInsets.pad(column, keyboard = true)
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            val pad = dp(12)
            setPadding(pad, dp(8), pad, dp(8))
        }
        hint = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            maxLines = 2
        }
        showHint()
        val close = TextView(this).apply {
            text = "Done"
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(16), dp(8), dp(4), dp(8))
            isClickable = true
            setOnClickListener { finishWithCookies() }
        }
        bar.addView(hint, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        bar.addView(close)
        webView = WebView(this)
        column.addView(bar)
        column.addView(webView, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        setContentView(column)

        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            request.optString("userAgent").takeIf { it.isNotBlank() }?.let { userAgentString = it }
        }
        CookieManager.getInstance().apply {
            setAcceptCookie(true)
            setAcceptThirdPartyCookies(webView, true)
        }
        webView.webChromeClient = object : WebChromeClient() {
            override fun onReceivedTitle(view: WebView, title: String?) = titled(title)
        }
        webView.webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) = started(url.orEmpty())

            override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, response: WebResourceResponse) {
                if (request.isForMainFrame) answered(request.url.toString(), response)
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (request.isForMainFrame) failed(request.url.toString())
            }

            override fun onPageFinished(view: WebView, url: String?) = finished(url.orEmpty(), view.title)
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (webView.canGoBack()) webView.goBack() else finishWithCookies()
            }
        })
        if (!url.startsWith("https://") && !url.startsWith("http://")) {
            finishWithCookies()
            return
        }
        if (request.optString("method") == "POST") {
            // A POST stays one, with its body; WebView posts it as a form and adds no headers.
            val body = if (request.isNull("body")) "" else request.optString("body")
            webView.postUrl(url, body.toByteArray(Charsets.UTF_8))
        } else {
            val headers = mutableMapOf<String, String>()
            request.optJSONObject("headers")?.let { given ->
                given.keys().forEach { name -> headers[name] = given.optString(name) }
            }
            webView.loadUrl(url, headers)
        }
    }

    private fun record(kind: String, vararg fields: Pair<String, Any>) {
        if (seen.length() >= MAX_SEEN) seen.remove(0)
        seen.put(JSONObject().put("kind", kind).apply { fields.forEach { (name, value) -> put(name, value) } })
    }

    private fun started(address: String) {
        record("start", "url" to address)
        pageUrl = address
        pageFinished = false
        pageFailed = false
        pageCheck = earlyCheck == address
        earlyCheck = null
        if (pageCheck) sawCheck = true
        showHint()
    }

    private fun answered(address: String, response: WebResourceResponse) {
        val mitigated = response.responseHeaders.orEmpty().any { (name, value) ->
            name.equals("cf-mitigated", ignoreCase = true) && value?.trim().equals("challenge", ignoreCase = true)
        }
        record("answer", "url" to address, "status" to response.statusCode, "mitigated" to mitigated)
        if (!mitigated) return
        if (pageUrl == address && !pageFinished) {
            pageCheck = true
            sawCheck = true
        } else {
            earlyCheck = address
        }
        showHint()
    }

    private fun titled(title: String?) {
        if (title.isNullOrBlank()) return
        record("title", "title" to title)
        if (looksLikeChallenge(title)) {
            pageCheck = true
            sawCheck = true
        }
        showHint()
    }

    private fun failed(address: String) {
        record("failed", "url" to address)
        pageFailed = true
        showHint()
    }

    private fun finished(address: String, title: String?) {
        titled(title)
        record("finish", "url" to address)
        pageFinished = true
        showHint()
        if (passed()) {
            main.removeCallbacksAndMessages(null)
            // A moment for the page to settle; a check that came back keeps the browser open.
            main.postDelayed({ if (passed()) finishWithCookies() }, 800)
        }
    }

    /** A check's page, then one that isn't, loaded. */
    private fun passed(): Boolean = sawCheck && pageFinished && !pageFailed && !pageCheck

    private fun showHint() {
        hint.text = when {
            pageFinished && pageFailed && !sawCheck -> "The page didn't load. Tap Done."
            pageFinished && !pageCheck && !sawCheck -> withHost("didn't ask for a check. Tap Done.")
            else -> withHost("pass the check, then tap Done")
        }
    }

    private fun withHost(text: String): String =
        if (host.isEmpty()) text.replaceFirstChar { it.uppercase() } else "$host: $text"

    private fun finishWithCookies() {
        if (done) return
        done = true
        val cookies = CookieManager.getInstance().run {
            flush()
            getCookie(url).orEmpty()
        }
        setResult(
            RESULT_OK,
            Intent()
                .putExtra(EXTRA_COOKIES, cookies)
                .putExtra(EXTRA_USER_AGENT, webView.settings.userAgentString.orEmpty())
                .putExtra(EXTRA_SEEN, seen.toString()),
        )
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

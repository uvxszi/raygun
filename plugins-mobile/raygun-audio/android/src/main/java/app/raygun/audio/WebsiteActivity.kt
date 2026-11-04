package app.raygun.audio

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.Message
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.provider.DocumentsContract
import android.text.format.Formatter
import android.webkit.CookieManager
import android.webkit.URLUtil
import android.webkit.GeolocationPermissions
import android.webkit.PermissionRequest
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import org.json.JSONObject
import java.io.ByteArrayInputStream
import kotlin.concurrent.thread

/** The browser's own process keeps its cookies, storage and cache apart from Raygun's webview. */
internal object WebProcess {
    private var ready = false

    fun init() {
        if (ready) return
        ready = true
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            runCatching { WebView.setDataDirectorySuffix("web") }
        }
    }
}

/**
 * Raygun's built-in browser for websites the user added (M46.3), in its own process (`:web`).
 * A plain WebView with no bridge to Raygun, plus a blocker: listed ad and tracker domains,
 * popups to other sites, and script-driven jumps to other sites (held, with a notice to go). On pause and close it
 * reports where it was and whether its blocker is on, in a broadcast the plugin passes to Rust.
 *
 * Nothing leaves for another app on its own: downloads open a sheet here (Download/Raygun or a
 * picked folder) and go to Raygun's downloads in the same broadcast, with the page's cookies,
 * User-Agent and Referer; links to apps are held. Only "Open in another browser" hands a page over.
 */
class WebsiteActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_REQUEST = "app.raygun.audio.WEBSITE"
        const val ACTION_REPORT = "app.raygun.audio.WEBSITE_REPORT"
        const val EXTRA_REPORT = "json"
        private const val DESKTOP_AGENT =
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
        private val EMPTY = ByteArray(0)
    }

    private val main = Handler(Looper.getMainLooper())
    private lateinit var webView: WebView
    private lateinit var titleView: TextView
    private lateinit var hostView: TextView
    private lateinit var shieldView: TextView
    private lateinit var progress: ProgressBar
    private lateinit var fullScreen: FrameLayout
    private lateinit var notice: TextView
    private var customView: View? = null
    /** The download sheet while it's shown, and the download it's about. */
    private var sheet: View? = null
    private var offered: JSONObject? = null

    /** The folder picker for "Choose folder…"; its grant is the app's, so Raygun's process can write there. */
    private val pickFolder = registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
        val download = offered ?: return@registerForActivityResult
        offered = null
        if (tree == null) return@registerForActivityResult
        runCatching {
            contentResolver.takePersistableUriPermission(
                tree,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
            )
        }
        val name = runCatching {
            val folder = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
            contentResolver.query(folder, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)
                ?.use { if (it.moveToFirst()) it.getString(0) else null }
        }.getOrNull() ?: tree.lastPathSegment ?: "Folder"
        startDownload(download, JSONObject().put("kind", "folder").put("uri", tree.toString()).put("name", name))
    }
    private var customCallback: WebChromeClient.CustomViewCallback? = null

    private var siteId = ""
    private var home = ""
    private var blockAds = true
    private var blockPopups = true
    private var desktop = false
    private var mobileAgent = ""
    @Volatile private var blocklist: Blocklist = Blocklist.builtIn()
    /** The site of the page shown: its own requests are never blocked. */
    @Volatile private var pageSite = ""
    @Volatile private var blockedRequests = 0
    private var blockedPopups = 0

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        WebProcess.init()
        super.onCreate(savedInstanceState)
        val request = runCatching { JSONObject(intent.getStringExtra(EXTRA_REQUEST) ?: "{}") }.getOrDefault(JSONObject())
        siteId = request.optString("id")
        home = request.optString("home")
        blockAds = request.optBoolean("blockAds", true)
        blockPopups = request.optBoolean("blockPopups", true)
        desktop = request.optBoolean("desktop", false)
        val listPath = request.optString("blocklist").ifBlank { null }
        thread(name = "raygun-blocklist") { blocklist = Blocklist.load(listPath) }

        buildViews()
        configure()
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                when {
                    sheet != null -> closeSheet()
                    customView != null -> webView.webChromeClient?.onHideCustomView()
                    webView.canGoBack() -> webView.goBack()
                    else -> finish()
                }
            }
        })
        if (savedInstanceState != null) {
            webView.restoreState(savedInstanceState)
            offered = savedInstanceState.getString("offered")?.let { runCatching { JSONObject(it) }.getOrNull() }
        } else {
            webView.loadUrl(request.optString("url").ifBlank { home })
        }
    }

    private fun label(size: Float, bold: Boolean = false): TextView = TextView(this).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        if (bold) typeface = Typeface.DEFAULT_BOLD
        maxLines = 1
        ellipsize = android.text.TextUtils.TruncateAt.END
    }

    private fun buildViews() {
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(PANEL)
            setPadding(dp(4), 0, dp(4), 0)
        }
        bar.addView(iconButton(R.drawable.raygun_ic_close, getString(android.R.string.cancel)) { finish() })
        val titles = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(4), 0, dp(4), 0)
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }
        titleView = label(15f, bold = true)
        hostView = label(12f).apply { alpha = 0.7f }
        titles.addView(titleView)
        titles.addView(hostView)
        bar.addView(titles)
        shieldView = label(13f, bold = true).apply {
            setCompoundDrawablesRelativeWithIntrinsicBounds(R.drawable.raygun_ic_shield, 0, 0, 0)
            compoundDrawablePadding = dp(4)
            setPadding(dp(8), dp(8), dp(8), dp(8))
            contentDescription = "Blocked"
            setOnClickListener { showBlocked() }
        }
        bar.addView(shieldView)
        bar.addView(iconButton(R.drawable.raygun_ic_reload, "Reload") { webView.reload() })
        lateinit var more: View
        more = iconButton(R.drawable.raygun_ic_more, "More") { showMenu(more) }
        bar.addView(more)
        column.addView(bar, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(56)))

        progress = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 100
            progressTintList = android.content.res.ColorStateList.valueOf(ACCENT)
            visibility = View.GONE
        }
        column.addView(progress, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(3)))

        webView = WebView(this)
        column.addView(webView, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))

        // Edge to edge (always, from Android 15): the bars' space is padded, in the bar's colour.
        val root = FrameLayout(this).apply { setBackgroundColor(PANEL) }
        androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
            insets
        }
        root.addView(column)
        fullScreen = FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            visibility = View.GONE
        }
        root.addView(fullScreen)
        notice = pill("") {}.apply {
            background = rounded(PANEL, dp(20).toFloat())
            visibility = View.GONE
        }
        root.addView(
            notice,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(24)
            },
        )
        setContentView(root)
        updateShield()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun configure() {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            loadWithOverviewMode = true
            useWideViewPort = true
            builtInZoomControls = true
            displayZoomControls = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = !blockPopups
            mediaPlaybackRequiresUserGesture = true
        }
        mobileAgent = webView.settings.userAgentString
        applyAgent()
        CookieManager.getInstance().setAcceptCookie(true)
        CookieManager.getInstance().setAcceptThirdPartyCookies(webView, true)
        webView.webViewClient = Client()
        webView.webChromeClient = Chrome()
        webView.setDownloadListener { url, userAgent, disposition, mime, length ->
            offerDownload(url, userAgent, disposition, mime, length)
        }
    }

    private fun applyAgent() {
        webView.settings.userAgentString = if (desktop) DESKTOP_AGENT else mobileAgent
    }

    private fun updateShield() {
        val total = blockedRequests + blockedPopups
        shieldView.text = if (blockAds || blockPopups) total.toString() else "off"
        shieldView.alpha = if (blockAds || blockPopups) 1f else 0.5f
    }

    private fun countRequest() {
        blockedRequests += 1
        main.post { updateShield() }
    }

    private fun showBlocked() {
        val text = if (!blockAds && !blockPopups) {
            "The blocker is off for this site"
        } else {
            val requests = if (blockedRequests == 1) "1 ad or tracker" else "$blockedRequests ads and trackers"
            val popups = if (blockedPopups == 1) "1 popup or redirect" else "$blockedPopups popups and redirects"
            "Blocked $requests and $popups · ${String.format(java.util.Locale.getDefault(), "%,d", blocklist.size)} sites listed"
        }
        Toast.makeText(this, text, Toast.LENGTH_LONG).show()
    }

    /** A short notice at the bottom; `action` adds a button to it. */
    private fun showNotice(text: String, action: String? = null, onAction: () -> Unit = {}) {
        notice.text = if (action != null) "$text · $action" else text
        notice.setOnClickListener {
            notice.visibility = View.GONE
            onAction()
        }
        notice.visibility = View.VISIBLE
        main.removeCallbacksAndMessages(notice)
        main.postAtTime({ notice.visibility = View.GONE }, notice, android.os.SystemClock.uptimeMillis() + 4000)
    }

    private fun showMenu(anchor: View) {
        val menu = PopupMenu(this, anchor)
        menu.menu.add(0, 1, 0, "Forward").isEnabled = webView.canGoForward()
        menu.menu.add(0, 2, 1, "Home")
        menu.menu.add(0, 3, 2, "Share link")
        menu.menu.add(0, 4, 3, "Open in another browser")
        menu.menu.add(0, 5, 4, "Desktop site").apply {
            isCheckable = true
            isChecked = desktop
        }
        menu.menu.add(0, 6, 5, "Block ads on this site").apply {
            isCheckable = true
            isChecked = blockAds
        }
        menu.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                1 -> webView.goForward()
                2 -> webView.loadUrl(home)
                3 -> {
                    val send = Intent(Intent.ACTION_SEND)
                        .setType("text/plain")
                        .putExtra(Intent.EXTRA_TEXT, webView.url ?: home)
                    startActivity(Intent.createChooser(send, "Share"))
                }
                4 -> openOutside(Uri.parse(webView.url ?: home))
                5 -> {
                    desktop = !desktop
                    applyAgent()
                    webView.reload()
                }
                6 -> {
                    blockAds = !blockAds
                    blockedRequests = 0
                    updateShield()
                    report()
                    webView.reload()
                }
            }
            true
        }
        menu.show()
    }

    /**
     * A download the page started (a link to a file, or a response sent as an attachment): the
     * sheet asks where it goes. `blob:` addresses only the page can read, so those aren't saved.
     */
    private fun offerDownload(url: String, userAgent: String?, disposition: String?, mime: String?, length: Long) {
        val scheme = Uri.parse(url).scheme?.lowercase()
        if (scheme != "http" && scheme != "https" && scheme != "data") {
            showNotice("This download can't be saved from here")
            return
        }
        val type = mime?.substringBefore(';')?.trim()?.takeIf { it.isNotEmpty() && it != "application/octet-stream" }
        val name = URLUtil.guessFileName(url, disposition, type)
        val download = JSONObject()
            .put("url", url)
            .put("userAgent", userAgent?.takeIf { it.isNotBlank() } ?: webView.settings.userAgentString)
            .put("referer", webView.url ?: home)
            .put("contentDisposition", disposition)
            .put("name", name)
            .put("mime", type)
            .put("size", if (length > 0) length.toDouble() else JSONObject.NULL)
        if (scheme != "data") {
            CookieManager.getInstance().getCookie(url)?.takeIf { it.isNotBlank() }?.let { download.put("cookies", it) }
        }
        showSheet(download, name, type, length)
    }

    private fun showSheet(download: JSONObject, name: String, type: String?, length: Long) {
        closeSheet()
        val root = window.decorView.findViewById<ViewGroup>(android.R.id.content).getChildAt(0) as? FrameLayout ?: return
        val scrim = FrameLayout(this).apply {
            setBackgroundColor(SCRIM)
            isClickable = true
            setOnClickListener { closeSheet() }
        }
        val panel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = rounded(PANEL, dp(20).toFloat())
            setPadding(dp(20), dp(20), dp(20), dp(16))
            isClickable = true
        }
        panel.addView(label(13f).apply {
            text = "Download file"
            alpha = 0.7f
        })
        panel.addView(label(17f, bold = true).apply {
            text = name
            maxLines = 3
            setPadding(0, dp(6), 0, dp(4))
        })
        val details = listOfNotNull(
            length.takeIf { it > 0 }?.let { Formatter.formatShortFileSize(this, it) },
            type,
        ).joinToString(" · ")
        if (details.isNotEmpty()) panel.addView(label(14f).apply { text = details })
        Uri.parse(webView.url ?: home).host?.let { host ->
            panel.addView(label(13f).apply {
                text = "From $host"
                alpha = 0.7f
            })
        }
        val choice = { text: String, filled: Boolean, action: () -> Unit ->
            pill(text, filled) { action() }.apply {
                layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(48)).apply {
                    topMargin = dp(10)
                }
                if (!filled) background = rounded(0x22FFFFFF, dp(24).toFloat())
            }
        }
        panel.addView(choice("Download to Download/Raygun", true) {
            closeSheet()
            startDownload(download, JSONObject().put("kind", "downloads"))
        }.apply { (layoutParams as LinearLayout.LayoutParams).topMargin = dp(18) })
        panel.addView(choice("Choose folder…", false) {
            closeSheet()
            offered = download
            runCatching { pickFolder.launch(null) }.onFailure {
                offered = null
                showNotice("No folder picker on this phone")
            }
        })
        panel.addView(choice(getString(android.R.string.cancel), false) { closeSheet() })
        scrim.addView(
            panel,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.BOTTOM
                setMargins(dp(8), 0, dp(8), dp(8))
            },
        )
        root.addView(scrim, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        sheet = scrim
    }

    private fun closeSheet() {
        val shown = sheet ?: return
        (shown.parent as? ViewGroup)?.removeView(shown)
        sheet = null
    }

    /** Hands the download to Raygun, which downloads it and shows it in its notifications. */
    private fun startDownload(download: JSONObject, destination: JSONObject) {
        download.put("destination", destination)
        val json = JSONObject().put("download", download).toString()
        sendBroadcast(Intent(ACTION_REPORT).setPackage(packageName).putExtra(EXTRA_REPORT, json))
        showNotice("Downloading ${download.optString("name")}")
    }

    /**
     * A link to another app. An `intent:` link's web fallback opens here; anything else stays,
     * with a notice: only the menu's "Open in another browser" leaves.
     */
    private fun holdAppLink(view: WebView, uri: Uri) {
        if (uri.scheme.equals("intent", ignoreCase = true)) {
            val fallback = runCatching {
                Intent.parseUri(uri.toString(), Intent.URI_INTENT_SCHEME).getStringExtra("browser_fallback_url")
            }.getOrNull()
            val scheme = fallback?.let { Uri.parse(it).scheme?.lowercase() }
            if (fallback != null && (scheme == "https" || scheme == "http")) {
                view.loadUrl(fallback)
                return
            }
        }
        blockedPopups += 1
        updateShield()
        showNotice("Link to another app held")
    }

    private fun openOutside(uri: Uri) {
        runCatching {
            startActivity(Intent.createChooser(Intent(Intent.ACTION_VIEW, uri), "Open with"))
        }
    }

    private fun sameSite(host: String?): Boolean = Blocklist.site(host) == pageSite

    private inner class Client : WebViewClient() {
        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
            if (!blockAds || request.isForMainFrame) return null
            val host = request.url.host ?: return null
            if (sameSite(host) || !blocklist.blocks(host)) return null
            countRequest()
            return WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(EMPTY))
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val uri = request.url
            val scheme = uri.scheme?.lowercase()
            if (scheme != "http" && scheme != "https") {
                if (scheme != "about" && scheme != "data" && scheme != "blob") holdAppLink(view, uri)
                return scheme != "about"
            }
            // Leaving for another site: a tap doesn't prove much (the browser counts any navigation
            // within seconds of a tap as tapped, which is how tap-to-redirect ads work), so it's held
            // with a notice. Server redirects of a navigation already let through go on.
            val leaving = request.isForMainFrame && !request.isRedirect && pageSite.isNotEmpty() &&
                !sameSite(uri.host)
            if (blockPopups && leaving) {
                blockedPopups += 1
                updateShield()
                showNotice("Leaving for ${uri.host} held", "Go") { view.loadUrl(uri.toString()) }
                return true
            }
            return false
        }

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            pageSite = Blocklist.site(Uri.parse(url).host)
            hostView.text = Uri.parse(url).host ?: url
            progress.visibility = View.VISIBLE
        }

        override fun onPageFinished(view: WebView, url: String) {
            progress.visibility = View.GONE
            report()
        }
    }

    private inner class Chrome : WebChromeClient() {
        override fun onProgressChanged(view: WebView, newProgress: Int) {
            progress.progress = newProgress
            progress.visibility = if (newProgress < 100) View.VISIBLE else View.GONE
        }

        override fun onReceivedTitle(view: WebView, title: String?) {
            titleView.text = title.orEmpty()
        }

        /**
         * New windows get a throwaway WebView, so their address can be read: a link to the same
         * site opens in the page, anything else is a popup.
         */
        override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message): Boolean {
            val probe = WebView(this@WebsiteActivity)
            probe.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(probeView: WebView, request: WebResourceRequest): Boolean {
                    val target = request.url
                    if (!blockPopups || (isUserGesture && sameSite(target.host))) {
                        webView.loadUrl(target.toString())
                    } else {
                        blockedPopups += 1
                        updateShield()
                        showNotice("Popup blocked", "Open") { webView.loadUrl(target.toString()) }
                    }
                    main.post { probeView.destroy() }
                    return true
                }
            }
            (resultMsg.obj as WebView.WebViewTransport).webView = probe
            resultMsg.sendToTarget()
            // A window that never navigates (written by script) is dropped too.
            main.postDelayed({ runCatching { probe.destroy() } }, 10_000)
            return true
        }

        override fun onShowCustomView(view: View, callback: CustomViewCallback) {
            if (customView != null) {
                callback.onCustomViewHidden()
                return
            }
            customView = view
            customCallback = callback
            fullScreen.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
            fullScreen.visibility = View.VISIBLE
            WindowInsetsControllerCompat(window, window.decorView).apply {
                hide(WindowInsetsCompat.Type.systemBars())
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
            requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }

        override fun onHideCustomView() {
            val view = customView ?: return
            fullScreen.removeView(view)
            fullScreen.visibility = View.GONE
            customView = null
            customCallback?.onCustomViewHidden()
            customCallback = null
            WindowInsetsControllerCompat(window, window.decorView).show(WindowInsetsCompat.Type.systemBars())
            requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }

        override fun onPermissionRequest(request: PermissionRequest) {
            request.deny()
        }

        override fun onGeolocationPermissionsShowPrompt(origin: String, callback: GeolocationPermissions.Callback) {
            callback.invoke(origin, false, false)
        }
    }

    /** Where the site is and whether its blocker is on, for Raygun to remember. */
    private fun report() {
        if (siteId.isEmpty()) return
        val json = JSONObject()
            .put("id", siteId)
            .put("url", webView.url ?: home)
            .put("blockAds", blockAds)
            .toString()
        sendBroadcast(Intent(ACTION_REPORT).setPackage(packageName).putExtra(EXTRA_REPORT, json))
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        webView.saveState(outState)
        // A download waiting for the folder picker survives the activity being remade.
        offered?.let { outState.putString("offered", it.toString()) }
    }

    override fun onPause() {
        report()
        webView.onPause()
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        webView.onResume()
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        (webView.parent as? ViewGroup)?.removeView(webView)
        webView.destroy()
        super.onDestroy()
    }
}

/** Clears the built-in browser's cookies, storage and cache, in its process, without a window. */
class WebsiteDataActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        WebProcess.init()
        super.onCreate(savedInstanceState)
        CookieManager.getInstance().removeAllCookies(null)
        CookieManager.getInstance().flush()
        WebStorage.getInstance().deleteAllData()
        WebView(this).apply {
            clearCache(true)
            clearHistory()
            destroy()
        }
        finish()
    }
}

/** Passes the browser's reports, sent from its own process, to Rust. */
internal class WebsiteReports : android.content.BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        intent.getStringExtra(WebsiteActivity.EXTRA_REPORT)?.let { NativeBridge.website(it) }
    }
}

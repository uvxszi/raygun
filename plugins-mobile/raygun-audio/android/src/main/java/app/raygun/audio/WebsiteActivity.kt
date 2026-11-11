package app.raygun.audio

import android.annotation.SuppressLint
import android.app.Activity
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Typeface
import android.media.AudioManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.Message
import android.os.SystemClock
import android.util.Log
import android.util.TypedValue
import android.view.ContextMenu
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
import android.webkit.WebSettings
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
import androidx.core.content.ContextCompat
import androidx.core.os.BundleCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.util.UUID
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
 * Plain WebViews with no bridge to Raygun, plus a blocker: listed ad and tracker domains,
 * popups to other sites, and script-driven jumps to other sites (held, with a notice to go). On pause and close it
 * reports where it was and whether its blocker is on, in a broadcast the plugin passes to Rust.
 *
 * Tabs: a link the person taps that's meant for a new window opens a tab, and so do New tab and
 * a long press's Open in new tab; the bar's tab button then lists them. Popups and windows that
 * scripts open never make one (see [newWindowAction]).
 *
 * Nothing leaves for another app on its own: downloads open a sheet here (Download/Raygun or a
 * picked folder) and go to Raygun's downloads in the same broadcast, with the page's cookies,
 * User-Agent and Referer; Raygun's word on them comes back to the download bar under the page
 * ([DownloadBar]). Links to apps are held. Only "Open in another browser" hands a page over.
 *
 * Site scripts: extensions may add scripts to pages of the sites they name. Raygun composes each
 * one's into a bundle and names the files in the request; every tab registers Raygun's page bridge
 * (`raygunBridge`, every frame) and then each bundle that's on, for its origins. Through the bridge
 * pages say hello, report what plays and offer videos, songs and playlists to Raygun (the bar's
 * "In Raygun" sheet); the browser tells them when it leaves the screen and passes on controls. The
 * menu switches each extension's scripts and offers to install suggested ones.
 *
 * Background play (`backgroundPlay`, on unless the request says otherwise): a page that plays goes
 * on when the browser leaves the screen ([PageWebView]), shown in a media notification
 * ([WebMediaService]) whose controls go to the frame that plays.
 */
class WebsiteActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_REQUEST = "app.raygun.audio.WEBSITE"
        const val ACTION_REPORT = "app.raygun.audio.WEBSITE_REPORT"
        /** Raygun's word on the downloads this browser started, to the browser's process. */
        const val ACTION_DOWNLOADS = "app.raygun.audio.WEBSITE_DOWNLOADS"
        /** Raygun's word on the site scripts (the list after an install, or a failed one), to the browser's process. */
        const val ACTION_SCRIPTS = "app.raygun.audio.WEBSITE_SCRIPTS"
        const val EXTRA_REPORT = "json"
        /** How often pages hear how things stand while the browser is off screen, ms. */
        private const val STATE_EVERY_MS = 3_000L
        /** Off screen, how long nothing may play before the notification goes and pages rest, ms. */
        private const val QUIET_REST_MS = 15_000L
        /** A main frame's hello this soon before its page start is that page's own, ms. */
        private const val HELLO_RACE_MS = 500L
        /** Offers the sheet lists, at most (the newest). */
        private const val SHEET_OFFERS = 5
        /** The menu's items for site scripts and for suggested ones start at these ids. */
        private const val SCRIPT_ITEMS = 100
        private const val SUGGESTION_ITEMS = 200
        private const val DESKTOP_AGENT =
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
        private val EMPTY = ByteArray(0)
        /** Tabs open at once, at most: each is a whole WebView. */
        private const val MAX_TABS = 16
        /**
         * Tabs whose history is kept when the browser is made again; the others keep their page.
         * Few, as all of it goes through one Binder call when the browser leaves the screen.
         */
        private const val KEPT_HISTORIES = 2
    }

    private val main = Handler(Looper.getMainLooper())
    private val tabs = mutableListOf<BrowserTab>()
    /** The tab shown. */
    private lateinit var tab: BrowserTab
    private var nextTabId = 1
    /** The tabs' WebViews, one shown. */
    private lateinit var pages: FrameLayout
    private lateinit var titleView: TextView
    private lateinit var hostView: TextView
    private lateinit var shieldView: TextView
    private lateinit var tabsButton: View
    private lateinit var tabsCount: TextView
    /** "In Raygun": shown while the tab shown offers something. */
    private lateinit var offersButton: View
    private lateinit var progress: ProgressBar
    private lateinit var fullScreen: KeptVisibleFrame
    private lateinit var notice: TextView
    private lateinit var downloads: DownloadBar
    private var tabList: TabListPopup? = null
    private var customView: View? = null
    private var customCallback: WebChromeClient.CustomViewCallback? = null
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

    /** Raygun's word on this browser's downloads, from its process. */
    private val downloadReports = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            intent.getStringExtra(EXTRA_REPORT)?.let { downloads.update(it) }
        }
    }

    /** Raygun's word on the site scripts, from its process. */
    private val scriptReports = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            intent.getStringExtra(EXTRA_REPORT)?.let { scriptsReport(it) }
        }
    }

    private var siteId = ""
    private var home = ""
    private var blockAds = true
    private var blockPopups = true
    private var desktop = false
    private var mobileAgent = ""
    @Volatile private var blocklist: Blocklist = Blocklist.builtIn()
    @Volatile private var blockedRequests = 0
    private var blockedPopups = 0

    /** Pages that play go on off screen. */
    private var backgroundPlay = true
    /** Raygun's page bridge; null without one. */
    private var bridgeCode: String? = null
    private var siteScripts: MutableList<SiteScript> = mutableListOf()
    private var suggestions: List<SiteSuggestion> = emptyList()
    /** Suggestions said in a notice while the browser is open (once each), and ones being installed. */
    private val suggested = mutableSetOf<String>()
    private val installing = mutableSetOf<String>()
    /** Raygun's word on site scripts read so far: only the newest is used. */
    private var scriptsHeard = 0
    /** Between onStart and onStop. */
    private var onScreen = false
    /** The tab whose media the notification shows. */
    private var mediaTab: BrowserTab? = null
    /** Whether pages hear how things stand every few seconds (off screen). */
    private var ticking = false
    /** The views are made (not when the browser closed at once, with no request). */
    private var built = false

    override fun onCreate(savedInstanceState: Bundle?) {
        WebProcess.init()
        super.onCreate(savedInstanceState)
        val asked = intent.getStringExtra(EXTRA_REQUEST)
        if (asked == null && savedInstanceState == null) {
            // Only the playing notification opens the browser without a request: for one that's gone.
            finish()
            return
        }
        val request = runCatching { JSONObject(asked ?: "{}") }.getOrDefault(JSONObject())
        siteId = request.optString("id")
        home = request.optString("home")
        blockAds = request.optBoolean("blockAds", true)
        blockPopups = request.optBoolean("blockPopups", true)
        desktop = request.optBoolean("desktop", false)
        val listPath = request.optString("blocklist").ifBlank { null }
        thread(name = "raygun-blocklist") { blocklist = Blocklist.load(listPath) }
        backgroundPlay = request.optBoolean("backgroundPlay", true)
        bridgeCode = readScript(request.text("bridge"))
        // The switches as they were when the browser is made again (Raygun keeps them too).
        val keptScripts = savedInstanceState?.getString("siteScripts")?.let { runCatching { JSONArray(it) }.getOrNull() }
        siteScripts = parseSiteScripts(keptScripts ?: request.optJSONArray("siteScripts"))
        suggestions = parseSuggestions(request.optJSONArray("suggestions"))
        savedInstanceState?.getStringArrayList("suggested")?.let { suggested.addAll(it) }

        mobileAgent = runCatching { WebSettings.getDefaultUserAgent(this) }.getOrDefault("")
        CookieManager.getInstance().setAcceptCookie(true)
        buildViews()
        built = true
        ContextCompat.registerReceiver(
            this,
            downloadReports,
            IntentFilter(ACTION_DOWNLOADS),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        ContextCompat.registerReceiver(
            this,
            scriptReports,
            IntentFilter(ACTION_SCRIPTS),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = back()
        })
        val restored = savedInstanceState != null && restoreTabs(savedInstanceState)
        if (savedInstanceState != null) {
            offered = savedInstanceState.getString("offered")?.let { runCatching { JSONObject(it) }.getOrNull() }
            downloads.restore(savedInstanceState)
        }
        if (!restored) openTab(request.optString("url").ifBlank { home }, opener = null)
    }

    /**
     * The playing notification brings the browser back as it was: nothing new to open, and the
     * first request stays the one read when the browser is made again.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
    }

    private fun back() {
        when {
            sheet != null -> closeSheet()
            customView != null -> hideCustomView()
            tab.webView.canGoBack() -> tab.webView.goBack()
            // A tab a link opened goes, back to the page it came from.
            tabs.size > 1 && tab.opener != null -> closeTab(tab)
            else -> finish()
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
        offersButton = iconButton(R.drawable.raygun_ic_play, "In Raygun") { showOffers() }.apply { visibility = View.GONE }
        bar.addView(offersButton)
        bar.addView(iconButton(R.drawable.raygun_ic_reload, "Reload") { tab.webView.reload() })
        val (button, count) = tabCountButton { showTabs() }
        tabsButton = button.apply { visibility = View.GONE }
        tabsCount = count
        bar.addView(button)
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

        // The page, with notices over its foot (above the download bar).
        val pageArea = FrameLayout(this)
        pages = FrameLayout(this)
        pageArea.addView(pages, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        notice = pill("") {}.apply {
            background = rounded(PANEL, dp(20).toFloat())
            visibility = View.GONE
        }
        pageArea.addView(
            notice,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(16)
                leftMargin = dp(16)
                rightMargin = dp(16)
            },
        )
        column.addView(pageArea, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))

        downloads = DownloadBar(
            this,
            ask = { question -> send(JSONObject().put("downloadAsk", question)) },
            notice = { text -> showNotice(text) },
        )
        column.addView(downloads.root, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))

        // Edge to edge (always, from Android 15): the bars' space is padded, in the bar's colour.
        val root = FrameLayout(this).apply { setBackgroundColor(PANEL) }
        // A camera cutout too (at a side in landscape), and the keyboard below.
        SafeInsets.pad(root, keyboard = true)
        root.addView(column)
        // A page's video full screen: shown to the page while the browser is away and it plays.
        fullScreen = KeptVisibleFrame(this) { ::tab.isInitialized && tab.pageView.keepVisible }.apply {
            setBackgroundColor(Color.BLACK)
            visibility = View.GONE
        }
        root.addView(fullScreen)
        setContentView(root)
        // The bar's icon buttons say their names when held.
        ControlTooltips.follow(root)
        updateShield()
    }

    // ---- Tabs ----

    /**
     * Opens a tab at `url` (or from a kept `state`), after `opener` when a link opened it. Shown
     * at once unless `show` is false (a long press's Open in new tab).
     */
    private fun openTab(
        url: String?,
        opener: BrowserTab?,
        state: Bundle? = null,
        show: Boolean = true,
        referer: String? = null,
    ): BrowserTab? {
        if (tabs.size >= MAX_TABS) {
            showNotice("Close a tab to open another")
            return null
        }
        val view = PageWebView(this)
        val made = BrowserTab(nextTabId++, view, opener?.id)
        setUp(made)
        view.visibility = View.GONE
        pages.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        tabs.add(made)
        made.address = url.orEmpty()
        val restored = state != null && view.restoreState(state) != null
        if (!restored && !url.isNullOrBlank()) {
            if (referer != null) view.loadUrl(url, mapOf("Referer" to referer)) else view.loadUrl(url)
        }
        if (show) switchTo(made) else onTabsChanged()
        return made
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setUp(owner: BrowserTab) {
        val view = owner.webView
        view.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            loadWithOverviewMode = true
            useWideViewPort = true
            builtInZoomControls = true
            displayZoomControls = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = !blockPopups
            mediaPlaybackRequiresUserGesture = true
            userAgentString = if (desktop) DESKTOP_AGENT else mobileAgent
        }
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, true)
        view.webViewClient = Client(owner)
        view.webChromeClient = Chrome(owner)
        view.setDownloadListener { url, userAgent, disposition, mime, length ->
            offerDownload(owner, url, userAgent, disposition, mime, length)
        }
        // A long press on a link: Open in new tab, Copy link.
        registerForContextMenu(view)
        addPageScripts(owner)
    }

    private fun switchTo(next: BrowserTab) {
        val previous = if (::tab.isInitialized) tab else null
        tab = next
        if (previous !== next) previous?.webView?.visibility = View.GONE
        next.webView.visibility = View.VISIBLE
        if (previous != null && previous !== next && customView != null) hideCustomView()
        titleView.text = next.title
        hostView.text = Uri.parse(next.shownAddress).host ?: next.shownAddress
        progress.progress = next.progress
        progress.visibility = if (next.progress < 100) View.VISIBLE else View.GONE
        onTabsChanged()
        updateOffersButton()
        next.heldJump?.let { held ->
            next.heldJump = null
            showHeldJump(next, held)
        }
        if (previous != null && previous !== next) report()
    }

    private fun closeTab(closing: BrowserTab) {
        val index = tabs.indexOf(closing)
        if (index < 0) return
        if (tabs.size == 1) {
            finish()
            return
        }
        tabs.removeAt(index)
        // Tabs it opened now count as opened from where it came from.
        tabs.forEach { if (it.opener == closing.id) it.opener = closing.opener }
        if (closing === tab) {
            switchTo(tabs.firstOrNull { it.id == closing.opener } ?: tabs[minOf(index, tabs.size - 1)])
        }
        forgetPage(closing)
        pages.removeView(closing.webView)
        closing.webView.stopLoading()
        closing.webView.destroy()
        onTabsChanged()
    }

    private fun newTab() {
        openTab(home, opener = null)
    }

    /** The tab button shows while more than one tab is open; the list follows changes. */
    private fun onTabsChanged() {
        val count = tabs.size
        tabsButton.visibility = if (count > 1) View.VISIBLE else View.GONE
        tabsCount.show(count.toString())
        tabsButton.contentDescription = "$count tabs"
        tabList?.refresh()
    }

    private fun showTabs() {
        tabList?.dismiss()
        val list = TabListPopup(
            this,
            tabs = { tabs },
            current = { tab },
            onPick = { picked -> if (picked in tabs) switchTo(picked) },
            onClose = { closing -> closeTab(closing) },
            onNew = { newTab() },
        )
        list.onDismiss = { if (tabList === list) tabList = null }
        tabList = list
        list.show(tabsButton)
    }

    /** The link the person last touched in `view`, when it was one (its full address). */
    private fun linkAt(view: WebView): String? {
        val hit = view.hitTestResult
        return when (hit.type) {
            WebView.HitTestResult.SRC_ANCHOR_TYPE -> hit.extra
            WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE -> {
                // A linked picture: `extra` is the picture; the link comes with the focused node.
                val message = main.obtainMessage()
                view.requestFocusNodeHref(message)
                message.data?.getString("url")
            }
            else -> null
        }?.takeIf { it.isNotBlank() }
    }

    override fun onCreateContextMenu(menu: ContextMenu, view: View, menuInfo: ContextMenu.ContextMenuInfo?) {
        super.onCreateContextMenu(menu, view, menuInfo)
        val owner = tabs.firstOrNull { it.webView === view } ?: return
        val link = linkAt(owner.webView) ?: return
        val scheme = Uri.parse(link).scheme?.lowercase()
        if (scheme != "http" && scheme != "https") return
        menu.setHeaderTitle(link)
        menu.add(0, 1, 0, "Open in new tab").setOnMenuItemClickListener {
            val target = Uri.parse(link)
            val made = openTab(
                link,
                owner,
                show = false,
                referer = refererFor(owner.webView.url, target, sameSite(owner, target.host)),
            )
            if (made != null) showNotice("Opened in a new tab", "Switch") { if (made in tabs) switchTo(made) }
            true
        }
        menu.add(0, 2, 1, "Copy link").setOnMenuItemClickListener {
            copyLink(link)
            true
        }
    }

    private fun copyLink(link: String) {
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return
        clipboard.setPrimaryClip(ClipData.newPlainText("Link", link))
        // Android 13 and later say so themselves.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) showNotice("Link copied")
    }

    /**
     * A new window a page asked for ([newWindowAction]): a tab for a link the person tapped, a
     * held tab for one to another site, else the page in its own tab or a blocked popup.
     */
    private fun newWindow(owner: BrowserTab, target: Uri, userGesture: Boolean, tappedLink: Boolean) {
        if (owner !in tabs) return
        val address = target.toString()
        val scheme = target.scheme?.lowercase()
        val web = scheme == "http" || scheme == "https"
        if (!web && scheme != "blob" && scheme != "data") {
            if (scheme != "about") holdAppLink(owner, target)
            return
        }
        val sameSite = web && sameSite(owner, target.host)
        val referer = refererFor(owner.webView.url, target, sameSite)
        when (newWindowAction(blockPopups, userGesture, web && tappedLink, sameSite)) {
            NewWindow.TAB -> openTab(address, owner, referer = referer)
            NewWindow.HELD_TAB -> {
                blockedPopups += 1
                updateShield()
                showNotice("Link to ${target.host} held", "Open in new tab") {
                    if (owner in tabs) openTab(address, owner, referer = referer)
                }
            }
            NewWindow.HERE -> owner.webView.loadUrl(address)
            NewWindow.BLOCKED -> {
                blockedPopups += 1
                updateShield()
                if (owner === tab) {
                    showNotice("Popup blocked", "Open") { if (owner in tabs) owner.webView.loadUrl(address) }
                }
            }
        }
    }

    // ---- The bar, the blocker and notices ----

    private fun applyAgent() {
        val agent = if (desktop) DESKTOP_AGENT else mobileAgent
        tabs.forEach { it.webView.settings.userAgentString = agent }
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

    /** A short notice at the page's foot; `action` adds a button to it. */
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
        val view = tab.webView
        val menu = PopupMenu(this, anchor)
        menu.menu.add(0, 7, 0, "New tab")
        menu.menu.add(0, 1, 1, "Forward").isEnabled = view.canGoForward()
        menu.menu.add(0, 2, 2, "Home")
        menu.menu.add(0, 3, 3, "Share link")
        menu.menu.add(0, 4, 4, "Open in another browser")
        menu.menu.add(0, 5, 5, "Desktop site").apply {
            isCheckable = true
            isChecked = desktop
        }
        menu.menu.add(0, 6, 6, "Block ads on this site").apply {
            isCheckable = true
            isChecked = blockAds
        }
        // Site scripts: each extension's switch, then the ones this page could use.
        val scripts = siteScripts.toList()
        scripts.forEachIndexed { index, script ->
            menu.menu.add(0, SCRIPT_ITEMS + index, 10 + index, script.name).apply {
                isCheckable = true
                isChecked = script.on
            }
        }
        val installable = suggestionsFor(Uri.parse(view.url ?: tab.address))
        installable.forEachIndexed { index, suggestion ->
            val busy = suggestion.id in installing
            menu.menu.add(
                0,
                SUGGESTION_ITEMS + index,
                80 + index,
                if (busy) "Installing ${suggestion.name}…" else "Install ${suggestion.name}…",
            ).isEnabled = !busy
        }
        menu.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                in SCRIPT_ITEMS until SCRIPT_ITEMS + scripts.size -> toggleScript(scripts[item.itemId - SCRIPT_ITEMS])
                in SUGGESTION_ITEMS until SUGGESTION_ITEMS + installable.size ->
                    install(installable[item.itemId - SUGGESTION_ITEMS])
                1 -> view.goForward()
                2 -> view.loadUrl(home)
                3 -> {
                    val send = Intent(Intent.ACTION_SEND)
                        .setType("text/plain")
                        .putExtra(Intent.EXTRA_TEXT, view.url ?: home)
                    startActivity(Intent.createChooser(send, "Share"))
                }
                4 -> openOutside(Uri.parse(view.url ?: home))
                5 -> {
                    desktop = !desktop
                    applyAgent()
                    view.reload()
                }
                6 -> {
                    blockAds = !blockAds
                    blockedRequests = 0
                    updateShield()
                    report()
                    view.reload()
                }
                7 -> newTab()
            }
            true
        }
        menu.show()
    }

    // ---- Downloads ----

    /**
     * A download the page started (a link to a file, or a response sent as an attachment): the
     * sheet asks where it goes. `blob:` addresses only the page can read, so those aren't saved.
     */
    private fun offerDownload(owner: BrowserTab, url: String, userAgent: String?, disposition: String?, mime: String?, length: Long) {
        val scheme = Uri.parse(url).scheme?.lowercase()
        if (scheme != "http" && scheme != "https" && scheme != "data") {
            showNotice("This download can't be saved from here")
            return
        }
        val type = mime?.substringBefore(';')?.trim()?.takeIf { it.isNotEmpty() && it != "application/octet-stream" }
        val name = URLUtil.guessFileName(url, disposition, type)
        val page = owner.webView.url?.takeIf { it.startsWith("http") }
        val download = JSONObject()
            .put("url", url)
            .put("userAgent", userAgent?.takeIf { it.isNotBlank() } ?: owner.webView.settings.userAgentString)
            .put("referer", page ?: tabs.firstOrNull { it.id == owner.opener }?.webView?.url ?: home)
            .put("contentDisposition", disposition)
            .put("name", name)
            .put("mime", type)
            .put("size", if (length > 0) length.toDouble() else JSONObject.NULL)
        if (scheme != "data") {
            CookieManager.getInstance().getCookie(url)?.takeIf { it.isNotBlank() }?.let { download.put("cookies", it) }
        }
        // A tab a link opened only for the file (nothing loaded in it) goes again, once its
        // WebView is out of this call.
        if (tabs.size > 1 && owner.opener != null && owner.webView.copyBackForwardList().size == 0) {
            main.post { if (owner in tabs) closeTab(owner) }
        }
        val from = Uri.parse(download.optString("referer")).host
        showSheet(download, name, type, length, from)
    }

    private fun showSheet(download: JSONObject, name: String, type: String?, length: Long, from: String?) {
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
        from?.let { host ->
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
        hostSheet(panel)
    }

    /** `panel` at the window's foot over the dimmed page; a tap outside or Back closes it. */
    private fun hostSheet(panel: View) {
        closeSheet()
        val root = window.decorView.findViewById<ViewGroup>(android.R.id.content).getChildAt(0) as? FrameLayout ?: return
        val scrim = FrameLayout(this).apply {
            setBackgroundColor(SCRIM)
            isClickable = true
            setOnClickListener { closeSheet() }
        }
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

    /**
     * Hands the download to Raygun, which downloads it and shows it in its notifications; its
     * ticket names it in Raygun's word back, which the download bar shows.
     */
    private fun startDownload(download: JSONObject, destination: JSONObject) {
        val ticket = UUID.randomUUID().toString()
        download.put("destination", destination).put("ticket", ticket)
        send(JSONObject().put("download", download))
        val folder = if (destination.optString("kind") == "folder") destination.optString("name") else "Download/Raygun"
        downloads.add(ticket, download.optString("name"), folder)
    }

    /** To Raygun's process: reports, downloads and questions about them. */
    private fun send(json: JSONObject) {
        sendBroadcast(Intent(ACTION_REPORT).setPackage(packageName).putExtra(EXTRA_REPORT, json.toString()))
    }

    // ---- Links ----

    /**
     * A link to another app. An `intent:` link's web fallback opens here; anything else stays,
     * with a notice: only the menu's "Open in another browser" leaves.
     */
    private fun holdAppLink(owner: BrowserTab, uri: Uri) {
        if (uri.scheme.equals("intent", ignoreCase = true)) {
            val fallback = runCatching {
                Intent.parseUri(uri.toString(), Intent.URI_INTENT_SCHEME).getStringExtra("browser_fallback_url")
            }.getOrNull()
            val scheme = fallback?.let { Uri.parse(it).scheme?.lowercase() }
            if (fallback != null && (scheme == "https" || scheme == "http")) {
                owner.webView.loadUrl(fallback)
                return
            }
        }
        blockedPopups += 1
        updateShield()
        if (owner === tab) showNotice("Link to another app held")
    }

    /** A jump to another site, held: said now when its tab is shown, else when it next is. */
    private fun showHeldJump(owner: BrowserTab, uri: Uri) {
        if (owner !== tab) {
            owner.heldJump = uri
            return
        }
        showNotice("Leaving for ${uri.host} held", "Go") { if (owner in tabs) owner.webView.loadUrl(uri.toString()) }
    }

    private fun openOutside(uri: Uri) {
        runCatching {
            startActivity(Intent.createChooser(Intent(Intent.ACTION_VIEW, uri), "Open with"))
        }
    }

    private fun sameSite(owner: BrowserTab, host: String?): Boolean = Blocklist.site(host) == owner.pageSite

    private inner class Client(private val owner: BrowserTab) : WebViewClient() {
        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
            if (!blockAds || request.isForMainFrame) return null
            val host = request.url.host ?: return null
            if (sameSite(owner, host) || !blocklist.blocks(host)) return null
            countRequest()
            return WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(EMPTY))
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val uri = request.url
            val scheme = uri.scheme?.lowercase()
            if (scheme != "http" && scheme != "https") {
                if (scheme != "about" && scheme != "data" && scheme != "blob") holdAppLink(owner, uri)
                return scheme != "about"
            }
            // Leaving for another site: a tap doesn't prove much (the browser counts any navigation
            // within seconds of a tap as tapped, which is how tap-to-redirect ads work), so it's held
            // with a notice. Server redirects of a navigation already let through go on.
            val leaving = request.isForMainFrame && !request.isRedirect && owner.pageSite.isNotEmpty() &&
                !sameSite(owner, uri.host)
            if (blockPopups && leaving) {
                blockedPopups += 1
                updateShield()
                showHeldJump(owner, uri)
                return true
            }
            return false
        }

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            owner.pageSite = Blocklist.site(Uri.parse(url).host)
            owner.address = url
            if (owner === tab) {
                hostView.text = Uri.parse(url).host ?: url
                progress.visibility = View.VISIBLE
            }
            pageStarted(owner, url)
        }

        override fun onPageFinished(view: WebView, url: String) {
            owner.address = url
            if (owner === tab) {
                progress.visibility = View.GONE
                report()
                suggestFor(owner, url)
            }
            tabList?.refresh()
        }
    }

    private inner class Chrome(private val owner: BrowserTab) : WebChromeClient() {
        override fun onProgressChanged(view: WebView, newProgress: Int) {
            owner.progress = newProgress
            if (owner === tab) {
                progress.progress = newProgress
                progress.visibility = if (newProgress < 100) View.VISIBLE else View.GONE
            }
        }

        override fun onReceivedTitle(view: WebView, title: String?) {
            owner.title = title.orEmpty()
            if (owner === tab) titleView.text = owner.title
            tabList?.refresh()
        }

        /**
         * New windows get a throwaway WebView, so their address can be read before anything
         * loads; [newWindow] decides what it becomes. Whether a link was tapped is read now, from
         * what's under the finger.
         */
        override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message): Boolean {
            val tapped = if (isUserGesture) linkAt(view) else null
            val probe = WebView(this@WebsiteActivity)
            var decided = false
            val decide = { target: Uri ->
                if (!decided) {
                    decided = true
                    newWindow(owner, target, isUserGesture, sameAddress(tapped, target.toString()))
                    main.post { runCatching { probe.destroy() } }
                }
            }
            probe.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(probeView: WebView, request: WebResourceRequest): Boolean {
                    decide(request.url)
                    return true
                }

                override fun onPageStarted(probeView: WebView, url: String, favicon: Bitmap?) {
                    // A window's first page may start without being asked about.
                    val target = Uri.parse(url)
                    val scheme = target.scheme?.lowercase()
                    if (scheme == "http" || scheme == "https") {
                        probeView.stopLoading()
                        decide(target)
                    }
                }
            }
            (resultMsg.obj as WebView.WebViewTransport).webView = probe
            resultMsg.sendToTarget()
            // A window that never navigates (written by script) is dropped too.
            main.postDelayed({ runCatching { probe.destroy() } }, 10_000)
            return true
        }

        /** A page closing its own window: a tab a link opened goes. */
        override fun onCloseWindow(window: WebView) {
            main.post { if (owner.opener != null && tabs.size > 1 && owner in tabs) closeTab(owner) }
        }

        override fun onShowCustomView(view: View, callback: CustomViewCallback) {
            if (customView != null || owner !== tab) {
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
            hideCustomView()
        }

        override fun onPermissionRequest(request: PermissionRequest) {
            request.deny()
        }

        override fun onGeolocationPermissionsShowPrompt(origin: String, callback: GeolocationPermissions.Callback) {
            callback.invoke(origin, false, false)
        }
    }

    private fun hideCustomView() {
        val view = customView ?: return
        fullScreen.removeView(view)
        fullScreen.visibility = View.GONE
        customView = null
        customCallback?.onCustomViewHidden()
        customCallback = null
        WindowInsetsControllerCompat(window, window.decorView).show(WindowInsetsCompat.Type.systemBars())
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    }

    // ---- Site scripts and the page bridge ----

    private val BrowserTab.pageView: PageWebView get() = webView as PageWebView

    private val BrowserTab.page: TabPage get() = pageView.page

    /**
     * What every page the tab loads gets: the bridge's listener and script (every frame), then
     * each extension's bundle that's on, for its origins. Without document-start scripts they run
     * as each page starts instead ([runScriptsLate]); without web message listeners pages have no
     * bridge (scripts still run).
     */
    private fun addPageScripts(owner: BrowserTab) {
        val view = owner.webView
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            runCatching { WebViewCompat.addWebMessageListener(view, BRIDGE_NAME, setOf("*"), Bridge(owner)) }
                .onFailure { Log.w("RaygunAudio", "pages have no bridge", it) }
        }
        val code = bridgeCode
        if (code != null && WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            owner.page.bridge = runCatching { WebViewCompat.addDocumentStartJavaScript(view, code, setOf("*")) }
                .onFailure { Log.w("RaygunAudio", "the page bridge couldn't be added", it) }
                .getOrNull()
        }
        siteScripts.filter { it.on }.forEach { addScript(owner, it) }
    }

    private fun addScript(owner: BrowserTab, script: SiteScript) {
        val code = script.code ?: return
        if (script.origins.isEmpty()) return
        removeScript(owner, script.id)
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            runCatching { WebViewCompat.addDocumentStartJavaScript(owner.webView, code, script.origins.toSet()) }
                .onSuccess { owner.page.scripts[script.id] = Registered(it, code) }
                .onFailure { Log.w("RaygunAudio", "${script.id}'s site scripts couldn't be added", it) }
        }
    }

    private fun removeScript(owner: BrowserTab, id: String) {
        val registered = owner.page.scripts.remove(id) ?: return
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            runCatching { registered.handler.remove() }
        }
    }

    /** The tab's scripts as the list has them now: gone, switched off or changed ones go, new ones come. */
    private fun syncScripts(owner: BrowserTab) {
        val registered = owner.page.scripts
        for (id in registered.keys.toList()) {
            val script = siteScripts.firstOrNull { it.id == id }
            if (script == null || !script.on || script.code != registered[id]?.code) removeScript(owner, id)
        }
        siteScripts.filter { it.on && it.id !in registered }.forEach { addScript(owner, it) }
    }

    /** Without document-start scripts: the bridge and the bundles for the page's origin, as it starts. */
    private fun runScriptsLate(owner: BrowserTab, url: String) {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) return
        val view = owner.webView
        bridgeCode?.let { view.evaluateJavascript(it, null) }
        val origin = Uri.parse(url)
        siteScripts.filter { it.on && it.runsOn(origin) }.forEach { script ->
            script.code?.let { view.evaluateJavascript(it, null) }
        }
    }

    /** A page started in the tab's main frame: what the one before said through the bridge goes. */
    private fun pageStarted(owner: BrowserTab, url: String) {
        // The new page's hello may arrive first: a main-frame hello just now was that page's own.
        if (SystemClock.uptimeMillis() - owner.page.mainHelloAt > HELLO_RACE_MS) newPage(owner)
        runScriptsLate(owner, url)
    }

    private fun newPage(owner: BrowserTab) {
        val page = owner.page
        val hadMedia = page.media != null
        page.clear()
        if (owner === tab) updateOffersButton()
        if (hadMedia) mediaChanged(owner) else keepFor(owner)
    }

    /** A tab closed: its media and offers go with it. */
    private fun forgetPage(closing: BrowserTab) {
        closing.page.clear()
        if (mediaTab === closing) {
            mediaTab = tabs.firstOrNull { it.page.media?.playing == true } ?: tabs.firstOrNull { it.page.media != null }
            showMedia()
        }
    }

    /** Messages from a tab's pages (`raygunBridge.postMessage`), on the main thread. */
    private inner class Bridge(private val owner: BrowserTab) : WebViewCompat.WebMessageListener {
        override fun onPostMessage(
            view: WebView,
            message: WebMessageCompat,
            sourceOrigin: Uri,
            isMainFrame: Boolean,
            replyProxy: JavaScriptReplyProxy,
        ) {
            if (owner !in tabs || message.type != WebMessageCompat.TYPE_STRING) return
            val data = runCatching { message.data }.getOrNull() ?: return
            if (data.length > MAX_PAGE_MESSAGE) return
            val json = runCatching { JSONObject(data) }.getOrNull() ?: return
            when (json.optString("type")) {
                "hello" -> hello(owner, replyProxy, isMainFrame)
                "media" -> mediaReport(owner, json, replyProxy)
                "offer" -> parseOffer(json, sourceOrigin, siteScripts)?.let { offer ->
                    owner.page.addOffer(offer)
                    if (owner === tab) updateOffersButton()
                }
                "withdraw" -> if (withdrawOffers(owner.page, json, sourceOrigin) && owner === tab) updateOffersButton()
            }
        }
    }

    /** A frame's bridge started: the frame is answered from now on; a main frame's starts a new page. */
    private fun hello(owner: BrowserTab, frame: JavaScriptReplyProxy, isMainFrame: Boolean) {
        val page = owner.page
        if (isMainFrame) {
            // What older pages said goes (their page start may come after this).
            if (page.frames.isNotEmpty() || page.offers.isNotEmpty() || page.media != null) newPage(owner)
            page.mainHelloAt = SystemClock.uptimeMillis()
        }
        page.addFrame(frame)
        // A page that starts while the browser is away (the next one, playing on) hears so at once.
        if (!onScreen) frame.tell(pageState())
    }

    /** What plays in a frame: `playing`, `paused`, or `none` (said by the frame that played). */
    private fun mediaReport(owner: BrowserTab, json: JSONObject, frame: JavaScriptReplyProxy) {
        val page = owner.page
        page.addFrame(frame)
        val media = parseMedia(json, frame)
        if (media == null) {
            if (json.optString("state") != "none") return
            val was = page.media ?: return
            if (was.frame !== frame) return
            page.media = null
        } else {
            page.media = media
        }
        mediaChanged(owner)
    }

    /** A `control` message to the frame that plays. */
    private fun postControl(frame: JavaScriptReplyProxy, action: String, seekTime: Double? = null) {
        val json = JSONObject().put("type", "control").put("action", action)
        if (seekTime != null && seekTime.isFinite()) json.put("seekTime", seekTime)
        frame.tell(json.toString())
    }

    /** How things stand, for pages: off screen or not, a call, other sound playing. */
    private fun pageState(): String {
        val audio = getSystemService(AudioManager::class.java)
        return JSONObject()
            .put("type", "state")
            .put("background", !onScreen)
            .put("call", audio != null && audio.mode != AudioManager.MODE_NORMAL)
            .put("audioActive", audio?.isMusicActive == true)
            .toString()
    }

    /** To every frame that said hello, in every tab. */
    private fun sendState() {
        val state = pageState()
        tabs.forEach { owner -> owner.page.frames.forEach { frame -> frame.tell(state) } }
    }

    /** Off screen while something plays or is paused: pages hear how things stand every 3 s. */
    private val stateTicks = object : Runnable {
        override fun run() {
            ticking = false
            if (onScreen || tabs.none { it.page.media != null }) return
            sendState()
            ticking = true
            main.postDelayed(this, STATE_EVERY_MS)
        }
    }

    private fun startStateTicks() {
        if (!backgroundPlay || ticking || onScreen) return
        ticking = true
        main.postDelayed(stateTicks, STATE_EVERY_MS)
    }

    private fun stopStateTicks() {
        main.removeCallbacks(stateTicks)
        ticking = false
    }

    // ---- What pages offer ----

    private fun updateOffersButton() {
        if (!::tab.isInitialized) return
        offersButton.visibility = if (tab.page.offers.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun showOffers() {
        val owner = tab
        val offers = owner.page.offers.take(SHEET_OFFERS)
        if (offers.isEmpty()) return
        hostSheet(offersPanel(offers) { offer, action -> pageAction(owner, offer, action) })
    }

    /**
     * An offer taken up: Raygun plays, queues, saves or downloads it. Playing pauses the page; a
     * video or a playlist plays in Raygun's own window, so the browser closes for it.
     */
    private fun pageAction(owner: BrowserTab, offer: PageOffer, action: String) {
        closeSheet()
        if (owner !in tabs) return
        send(
            JSONObject().put(
                "pageAction",
                JSONObject()
                    .put("site", siteId)
                    .put("page", owner.webView.url ?: offer.page)
                    .put("extension", offer.extension)
                    .put("action", action)
                    .put("offer", offer.json()),
            ),
        )
        showNotice(
            when (action) {
                "play" -> "Playing in Raygun"
                "queue" -> "Added to Raygun's queue"
                "save" -> "Saved to your library"
                else -> "Downloading in Raygun"
            },
        )
        if (action != "play") return
        owner.page.media?.frame?.let { postControl(it, "pause") }
        if (offer.kind == "video" || offer.kind == "playlist") {
            report()
            finish()
        }
    }

    // ---- The menu's site scripts ----

    /** Suggested extensions, not installed, whose scripts run on pages of `origin`. */
    private fun suggestionsFor(origin: Uri): List<SiteSuggestion> = suggestions.filter { suggestion ->
        siteScripts.none { it.id == suggestion.id } && suggestion.origins.any { originMatches(it, origin) }
    }

    /** An extension's scripts switched on or off for this site: Raygun keeps it; the page loads again. */
    private fun toggleScript(script: SiteScript) {
        if (siteScripts.none { it === script }) return
        script.on = !script.on
        send(JSONObject().put("siteScripts", JSONObject().put("site", siteId).put("extension", script.id).put("on", script.on)))
        tabs.forEach { owner ->
            if (script.on) {
                addScript(owner, script)
            } else {
                removeScript(owner, script.id)
                owner.page.offers.removeAll { it.extension == script.id }
            }
        }
        updateOffersButton()
        tab.webView.reload()
    }

    private fun install(suggestion: SiteSuggestion) {
        installing.add(suggestion.id)
        send(
            JSONObject().put(
                "installSiteScripts",
                JSONObject().put("site", siteId).put("extension", suggestion.id).put("repository", suggestion.repository),
            ),
        )
        showNotice("Installing ${suggestion.name}…")
    }

    /** A page finished in the tab shown: a suggestion for it, once while the browser is open. */
    private fun suggestFor(owner: BrowserTab, url: String) {
        // Another notice says something now: next time.
        if (owner !== tab || notice.visibility == View.VISIBLE) return
        val suggestion = suggestionsFor(Uri.parse(url)).firstOrNull { it.id !in suggested && it.id !in installing } ?: return
        suggested.add(suggestion.id)
        showNotice("${suggestion.name} can improve this site", "Install") { install(suggestion) }
    }

    /** Raygun's word on site scripts: the list after an install (its bundles read off the main thread), or a failure. */
    private fun scriptsReport(json: String) {
        val message = runCatching { JSONObject(json) }.getOrNull() ?: return
        if (message.optString("site") != siteId) return
        val error = message.text("error")
        if (error != null) {
            message.text("extension")?.let { installing.remove(it) }
            showNotice("Couldn't install: ${firstLine(error)}")
            return
        }
        val list = message.optJSONArray("siteScripts") ?: return
        val installed = message.text("installed")
        val heard = ++scriptsHeard
        thread(name = "raygun-site-scripts") {
            val read = parseSiteScripts(list)
            main.post { if (heard == scriptsHeard && !isDestroyed) useScripts(read, installed) }
        }
    }

    private fun useScripts(read: MutableList<SiteScript>, installed: String?) {
        siteScripts = read
        tabs.forEach { owner ->
            syncScripts(owner)
            owner.page.offers.removeAll { offer -> read.none { it.id == offer.extension && it.on } }
        }
        updateOffersButton()
        if (installed == null) return
        installing.remove(installed)
        val name = read.firstOrNull { it.id == installed }?.name
            ?: suggestions.firstOrNull { it.id == installed }?.name
            ?: "Site scripts"
        showNotice("$name installed")
        tab.webView.reload()
    }

    /** An error's first line, about 80 characters at most. */
    private fun firstLine(error: String): String {
        val line = error.lineSequence().firstOrNull { it.isNotBlank() }?.trim().orEmpty()
        return if (line.length > 80) line.take(79).trimEnd() + "…" else line
    }

    // ---- Playing off screen ----

    /** The notification's and the media session's controls, for the frame that plays. */
    private val mediaOwner = object : WebMediaOwner {
        override fun control(action: String, seekTime: Double?) {
            val frame = mediaTab?.takeIf { it in tabs }?.page?.media?.frame ?: return
            postControl(frame, action, seekTime)
        }

        override fun released() {
            restPages(paused = true)
        }
    }

    /** A tab's media changed: whether its page keeps playing off screen, and what the notification shows. */
    private fun mediaChanged(owner: BrowserTab) {
        val media = owner.page.media
        val current = mediaTab?.takeIf { it in tabs && it.page.media != null }
        mediaTab = when {
            media?.playing == true -> owner
            media != null && current == null -> owner
            current != null -> current
            else -> tabs.firstOrNull { it.page.media?.playing == true } ?: tabs.firstOrNull { it.page.media != null }
        }
        keepFor(owner)
        showMedia()
    }

    /**
     * Whether a tab's page hears it's shown when the browser leaves the screen: while it plays
     * (with background play). Paused off screen it stays so, as the notification may play it
     * again; on screen it goes, and off screen [restPages] lets it go.
     */
    private fun keepFor(owner: BrowserTab) {
        val keep = when {
            !backgroundPlay -> false
            owner.page.media?.playing == true -> true
            onScreen -> false
            else -> owner.pageView.keepVisible
        }
        setKeep(owner, keep)
    }

    private fun setKeep(owner: BrowserTab, keep: Boolean) {
        val view = owner.pageView
        if (view.keepVisible == keep) return
        view.keepVisible = keep
        // Off screen the page hears it now; on screen the window shows (or is about to) anyway.
        if (onScreen) return
        view.tellWindow()
        if (owner === tab) fullScreen.recheck()
    }

    /**
     * Off screen: pages that kept playing rest like the others once nothing plays in them (paused
     * ones too once the notification went), their WebViews paused as [onPause] would have.
     */
    private fun restPages(paused: Boolean) {
        if (onScreen) return
        tabs.forEach { owner ->
            val media = owner.page.media
            if (media?.playing == true || (media != null && !paused) || !owner.pageView.keepVisible) return@forEach
            setKeep(owner, false)
            owner.webView.onPause()
        }
    }

    /** Off screen, nothing played for a while: the notification goes and pages rest. */
    private val quiet = Runnable {
        if (tabs.none { it.page.media != null }) WebMedia.clear(mediaOwner)
        restPages(paused = false)
    }

    /**
     * The notification shows [mediaTab]'s media. Once no tab has any it goes at once on screen;
     * off screen after a while, as the page may go on to the next video (a service stopped there
     * can't start again until the browser is back).
     */
    private fun showMedia() {
        if (!backgroundPlay) return
        val owner = mediaTab?.takeIf { it in tabs }
        val media = owner?.page?.media
        main.removeCallbacks(quiet)
        if (owner == null || media == null) {
            if (onScreen) WebMedia.clear(mediaOwner) else main.postDelayed(quiet, QUIET_REST_MS)
            return
        }
        WebMedia.show(this, mediaOwner, mediaInfo(owner, media), offScreen = !onScreen)
        startStateTicks()
    }

    private fun mediaInfo(owner: BrowserTab, media: PageMedia): WebMediaInfo {
        val host = Uri.parse(owner.shownAddress).host
        val title = firstText(media.title, owner.title, host) ?: "A website"
        return WebMediaInfo(
            title = title,
            artist = media.artist ?: host?.takeIf { it != title },
            album = media.album,
            artworkUrl = media.artwork,
            durationMs = media.duration?.let { (it * 1000).toLong() },
            positionMs = media.position?.let { (it * 1000).toLong() },
            rate = media.rate.toFloat(),
            playing = media.playing,
            actions = media.actions,
            at = media.at,
            userAgent = owner.webView.settings.userAgentString,
        )
    }

    /** Where the site is (the tab shown) and whether its blocker is on, for Raygun to remember. */
    private fun report() {
        if (siteId.isEmpty() || !::tab.isInitialized) return
        val json = JSONObject()
            .put("id", siteId)
            .put("url", tab.webView.url ?: tab.address.ifBlank { home })
            .put("blockAds", blockAds)
        send(json)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (tabs.isNotEmpty()) {
            // The shown tab and a few others keep their history; the rest their page.
            val historied = (listOf(tab) + tabs.filter { it !== tab }).take(KEPT_HISTORIES).toSet()
            val saved = ArrayList<Bundle>()
            tabs.forEach { kept ->
                val bundle = Bundle()
                bundle.putInt("id", kept.id)
                bundle.putInt("opener", kept.opener ?: -1)
                bundle.putString("title", kept.title)
                bundle.putString("address", kept.shownAddress)
                if (kept in historied) {
                    val state = Bundle()
                    kept.webView.saveState(state)
                    bundle.putBundle("state", state)
                }
                saved.add(bundle)
            }
            outState.putParcelableArrayList("tabs", saved)
            outState.putInt("shown", tabs.indexOf(tab))
        }
        // A download waiting for the folder picker survives the activity being remade.
        offered?.let { outState.putString("offered", it.toString()) }
        downloads.save(outState)
        outState.putString("siteScripts", JSONArray().apply { siteScripts.forEach { put(it.json()) } }.toString())
        outState.putStringArrayList("suggested", ArrayList(suggested))
    }

    /** The tabs as they were before the activity was made again; false when none were kept. */
    private fun restoreTabs(saved: Bundle): Boolean {
        val kept = BundleCompat.getParcelableArrayList(saved, "tabs", Bundle::class.java)
        if (kept.isNullOrEmpty()) return false
        val made = mutableMapOf<Int, BrowserTab>()
        for (bundle in kept) {
            val address = bundle.getString("address").orEmpty().ifBlank { home }
            val opened = openTab(address, opener = null, state = bundle.getBundle("state"), show = false) ?: continue
            opened.title = bundle.getString("title").orEmpty()
            made[bundle.getInt("id")] = opened
        }
        if (tabs.isEmpty()) return false
        for (bundle in kept) {
            made[bundle.getInt("id")]?.opener = made[bundle.getInt("opener")]?.id
        }
        switchTo(tabs.getOrNull(saved.getInt("shown", 0)) ?: tabs.first())
        return true
    }

    override fun onStart() {
        super.onStart()
        onScreen = true
        // Raygun tells the download bar how its downloads go while the browser is on screen.
        downloads.watch()
        stopStateTicks()
        // Back on screen: paused pages no longer need to look shown.
        tabs.forEach { keepFor(it) }
        sendState()
        if (backgroundPlay) {
            WebMedia.screen(mediaOwner, offScreen = false)
            showMedia()
        }
    }

    override fun onStop() {
        onScreen = false
        downloads.unwatch()
        sendState()
        if (backgroundPlay) {
            WebMedia.screen(mediaOwner, offScreen = true)
            if (tabs.any { it.page.media != null }) startStateTicks()
        }
        super.onStop()
    }

    override fun onPause() {
        report()
        // A page that plays goes on off screen (background play): its WebView isn't paused.
        tabs.forEach { if (!it.pageView.keepVisible) it.webView.onPause() }
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        tabs.forEach { it.webView.onResume() }
    }

    override fun onDestroy() {
        if (!built) {
            super.onDestroy()
            return
        }
        main.removeCallbacksAndMessages(null)
        WebMedia.clear(mediaOwner)
        downloads.clear()
        tabList?.dismiss()
        runCatching { unregisterReceiver(downloadReports) }
        runCatching { unregisterReceiver(scriptReports) }
        tabs.forEach { closing ->
            (closing.webView.parent as? ViewGroup)?.removeView(closing.webView)
            closing.webView.destroy()
        }
        tabs.clear()
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

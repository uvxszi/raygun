package app.raygun.audio

import android.annotation.SuppressLint
import android.app.Service
import android.content.Intent
import android.content.res.Resources
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.os.SystemClock
import android.util.Base64
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.ProfileStore
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.File
import java.net.InetAddress
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong

/**
 * Hidden web views for converted sources (`raygun_ext_host::webview`), in a process of their own (`:views`) with
 * their own WebView data, apart from Raygun's page and the websites' and checks' browsers. Each extension's views
 * share a profile of their own when the WebView has profiles (cookies and storage kept per extension). Commands
 * come from [HiddenViews] through this bound service's [Messenger]; events go back to it.
 *
 * A view is never shown: it's laid out off screen at the display's size. It loads only what the extension may
 * reach (its hosts, `*` any public one, never this device or the local network) and never a browser check's
 * provider (Turnstile, hCaptcha, reCAPTCHA…): Raygun never passes a check by itself. Its page reaches nothing but
 * the message objects the source named, for the origins it named. Requests the source answers wait for it on
 * WebView's own threads until the view's time is up; the view is closed then whatever happens.
 */
class HiddenViewService : Service() {
    companion object {
        const val MSG_COMMAND = 1
        const val MSG_EVENT = 2
        const val KEY_JSON = "json"

        /** Views open at once in this process. */
        private const val MOST_VIEWS = 4

        private var dataReady = false
    }

    private val main = Handler(Looper.getMainLooper())
    private val views = HashMap<String, HiddenView>()

    @Volatile
    private var client: Messenger? = null

    private val incoming = Messenger(
        Handler(Looper.getMainLooper()) { message ->
            message.replyTo?.let { client = it }
            message.data?.getString(KEY_JSON)?.let { handle(it) }
            true
        },
    )

    override fun onCreate() {
        CrashLog.install(this)
        if (!dataReady && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            dataReady = true
            runCatching { WebView.setDataDirectorySuffix("views") }
        }
        super.onCreate()
    }

    override fun onBind(intent: Intent?): IBinder = incoming.binder

    override fun onUnbind(intent: Intent?): Boolean {
        closeAll()
        return false
    }

    override fun onDestroy() {
        closeAll()
        main.removeCallbacksAndMessages(null)
        super.onDestroy()
    }

    private fun handle(json: String) {
        val command = runCatching { JSONObject(json) }.getOrNull() ?: return
        val id = command.optString("id")
        val view = views[id]
        when (command.optString("op")) {
            "open" -> open(id, command)
            "load" -> view?.load(command.optString("url"), headers(command.optJSONObject("headers")))
            "loadData" -> view?.loadData(command.optString("baseUrl"), command.optString("data"), command.optString("mime"))
            "evaluate" -> {
                val call = command.optLong("call")
                if (view == null) {
                    emit(id, "evaluated", JSONObject().put("call", call).put("result", JSONObject.NULL))
                } else {
                    view.evaluate(call, command.optString("script"))
                }
            }
            "answer" -> view?.answer(command.optLong("request"), command.optJSONObject("response"))
                ?: removeSpilled(command.optJSONObject("response"))
            "reply" -> view?.reply(command.optLong("reply"), command.optString("message"))
            "stop" -> view?.stop()
            "close" -> close(id)
        }
    }

    private fun open(id: String, spec: JSONObject) {
        if (id.isEmpty() || views.containsKey(id)) return
        if (views.size >= MOST_VIEWS) {
            emit(id, "failed", JSONObject().put("url", "").put("code", -1).put("description", "too many web views").put("mainFrame", true))
            emit(id, "closed", JSONObject())
            return
        }
        val view = runCatching { HiddenView(this, id, spec) }.getOrElse { error ->
            emit(id, "failed", JSONObject().put("url", "").put("code", -1).put("description", error.message ?: "the web view didn't start").put("mainFrame", true))
            emit(id, "closed", JSONObject())
            return
        }
        views[id] = view
        main.postDelayed({ close(id) }, view.timeoutMs + 5_000)
    }

    fun close(id: String) {
        val view = views.remove(id) ?: return
        emit(id, "closed", JSONObject().put("cookies", view.close()))
    }

    private fun closeAll() {
        for (id in views.keys.toList()) close(id)
    }

    /** An event for Raygun; may be called from any thread. */
    fun emit(id: String, kind: String, fields: JSONObject) {
        val target = client ?: return
        val event = fields.put("id", id).put("kind", kind)
        val message = Message.obtain(null, MSG_EVENT).apply {
            data = Bundle().apply { putString(KEY_JSON, event.toString()) }
        }
        runCatching { target.send(message) }
    }

    fun gone(id: String, crashed: Boolean) {
        main.post {
            emit(id, "gone", JSONObject().put("crashed", crashed))
            close(id)
        }
    }

    private fun headers(given: JSONObject?): Map<String, String> {
        val headers = mutableMapOf<String, String>()
        given?.keys()?.forEach { name -> headers[name] = given.optString(name) }
        return headers
    }

    private fun removeSpilled(response: JSONObject?) {
        response?.optString("bodyFile")?.takeIf { it.isNotEmpty() }?.let { runCatching { File(it).delete() } }
    }
}

/** A request the source answers: the WebView thread waits on [latch] for [response]. */
private class Pending {
    val latch = CountDownLatch(1)

    @Volatile
    var answered = false

    @Volatile
    var response: WebResourceResponse? = null
}

/** One hidden view and what its run set up. */
@SuppressLint("SetJavaScriptEnabled")
private class HiddenView(private val service: HiddenViewService, private val id: String, spec: JSONObject) {
    val timeoutMs: Long = spec.optLong("timeoutMs", 30_000L).coerceIn(1_000L, 60_000L)
    private val deadline = SystemClock.uptimeMillis() + timeoutMs
    private val webView = WebView(service)
    private val cookies: CookieManager
    private val intercept = spec.optBoolean("intercept")
    private val hosts = strings(spec.optJSONArray("hosts"))
    private val challengeHosts = strings(spec.optJSONArray("challengeHosts"))
    private val challengePaths = strings(spec.optJSONArray("challengePaths"))
    private val pending = ConcurrentHashMap<Long, Pending>()
    private val replies = ConcurrentHashMap<Long, JavaScriptReplyProxy>()
    private val nextRequest = AtomicLong(0)
    private val nextReply = AtomicLong(0)
    private val pages = mutableListOf<String>()

    /** Document-start scripts the WebView can't add itself, run as each page starts. */
    private val lateScripts = mutableListOf<Pair<String, List<String>>>()

    @Volatile
    private var closed = false

    init {
        cookies = profileCookies(spec.optString("extension"))
        webView.settings.apply {
            javaScriptEnabled = spec.optBoolean("javascript", true)
            domStorageEnabled = spec.optBoolean("domStorage", true)
            blockNetworkImage = spec.optBoolean("blockImages")
            spec.optString("userAgent").takeIf { it.isNotBlank() }?.let { userAgentString = it }
            allowFileAccess = false
            allowContentAccess = false
            mediaPlaybackRequiresUserGesture = true
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
        }
        cookies.setAcceptCookie(true)
        cookies.setAcceptThirdPartyCookies(webView, true)
        val given = spec.optJSONArray("cookies") ?: JSONArray()
        for (index in 0 until given.length()) {
            val entry = given.optJSONObject(index) ?: continue
            val url = entry.optString("url")
            entry.optString("cookies").split("; ").filter { it.contains('=') }.forEach { pair -> cookies.setCookie(url, pair) }
        }
        layOut()
        addBridges(spec.optJSONArray("bridges"))
        addScripts(spec.optJSONArray("scripts"))
        webView.webChromeClient = object : WebChromeClient() {
            override fun onReceivedTitle(view: WebView, title: String?) {
                if (!title.isNullOrBlank()) emit("title", JSONObject().put("title", title))
            }
        }
        webView.webViewClient = Client()
    }

    /** The extension's own profile when the WebView has profiles, else the process's cookies, cleared for each view. */
    private fun profileCookies(extension: String): CookieManager {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROFILE)) {
            val name = "ext-" + extension.replace(Regex("[^A-Za-z0-9._-]"), "_").take(80)
            val profile = runCatching { ProfileStore.getInstance().getOrCreateProfile(name) }.getOrNull()
            if (profile != null && runCatching { WebViewCompat.setProfile(webView, name) }.isSuccess) {
                return profile.cookieManager
            }
        }
        val shared = CookieManager.getInstance()
        shared.removeAllCookies(null)
        WebStorage.getInstance().deleteAllData()
        return shared
    }

    /** Laid out at the display's size, never shown: pages that measure themselves see a phone's screen. */
    private fun layOut() {
        runCatching {
            val metrics = Resources.getSystem().displayMetrics
            webView.layoutParams = ViewGroup.LayoutParams(metrics.widthPixels, metrics.heightPixels)
            webView.measure(
                View.MeasureSpec.makeMeasureSpec(metrics.widthPixels, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(metrics.heightPixels, View.MeasureSpec.EXACTLY),
            )
            webView.layout(0, 0, metrics.widthPixels, metrics.heightPixels)
        }
    }

    private fun addBridges(bridges: JSONArray?) {
        bridges ?: return
        for (index in 0 until bridges.length()) {
            val bridge = bridges.optJSONObject(index) ?: continue
            val name = bridge.optString("name").takeIf { it.isNotEmpty() } ?: continue
            val origins = strings(bridge.optJSONArray("origins")).ifEmpty { listOf("*") }
            if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
                runCatching {
                    WebViewCompat.addWebMessageListener(webView, name, origins.toSet()) { _, message: WebMessageCompat, origin: Uri, isMainFrame, reply ->
                        val number = nextReply.incrementAndGet()
                        replies[number] = reply
                        emit(
                            "message",
                            JSONObject()
                                .put("bridge", name)
                                .put("message", message.data ?: "")
                                .put("origin", origin.toString())
                                .put("mainFrame", isMainFrame)
                                .put("reply", number),
                        )
                    }
                }
            } else if (origins == listOf("*")) {
                // Older WebViews: an interface every frame reaches, so only where the source allowed any origin.
                webView.addJavascriptInterface(
                    object {
                        @JavascriptInterface
                        fun postMessage(message: String) {
                            emit("message", JSONObject().put("bridge", name).put("message", message))
                        }
                    },
                    name,
                )
            }
        }
    }

    private fun addScripts(scripts: JSONArray?) {
        scripts ?: return
        for (index in 0 until scripts.length()) {
            val entry = scripts.optJSONObject(index) ?: continue
            val script = entry.optString("script")
            val origins = strings(entry.optJSONArray("origins")).ifEmpty { listOf("*") }
            val added = WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT) &&
                runCatching { WebViewCompat.addDocumentStartJavaScript(webView, script, origins.toSet()) }.isSuccess
            if (!added) lateScripts += script to origins
        }
    }

    fun load(url: String, headers: Map<String, String>) {
        if (closed) return
        if (url == "about:blank" || allowed(Uri.parse(url))) {
            webView.loadUrl(url, headers)
        } else {
            emit("failed", JSONObject().put("url", url).put("code", -1).put("description", "not a host the source may reach").put("mainFrame", true))
        }
    }

    fun loadData(baseUrl: String, data: String, mime: String) {
        if (closed) return
        webView.loadDataWithBaseURL(baseUrl, data, mime.ifEmpty { "text/html" }, "UTF-8", null)
    }

    fun evaluate(call: Long, script: String) {
        if (closed) {
            emit("evaluated", JSONObject().put("call", call).put("result", JSONObject.NULL))
            return
        }
        webView.evaluateJavascript(script) { value ->
            emit("evaluated", JSONObject().put("call", call).put("result", value ?: JSONObject.NULL))
        }
    }

    fun reply(number: Long, message: String) {
        if (closed) return
        runCatching { replies[number]?.postMessage(message) }
    }

    fun stop() {
        if (!closed) webView.stopLoading()
    }

    /** The source's answer to an intercepted request (null: the view loads it). */
    fun answer(number: Long, response: JSONObject?) {
        val waiting = pending[number]
        val built = response?.let { responseOf(it) }
        if (waiting == null) return
        waiting.response = built
        waiting.answered = true
        waiting.latch.countDown()
    }

    /** Closes the view; answers its cookies for the pages it loaded. */
    fun close(): JSONArray {
        closed = true
        for (waiting in pending.values) waiting.latch.countDown()
        pending.clear()
        replies.clear()
        val found = JSONArray()
        runCatching {
            cookies.flush()
            for (page in pages.distinct().take(16)) {
                val header = cookies.getCookie(page).orEmpty()
                if (header.isNotEmpty()) found.put(JSONObject().put("url", page).put("cookies", header))
            }
        }
        runCatching {
            webView.stopLoading()
            webView.destroy()
        }
        return found
    }

    private fun emit(kind: String, fields: JSONObject) {
        if (!closed || kind == "evaluated") service.emit(id, kind, fields)
    }

    private fun responseOf(answer: JSONObject): WebResourceResponse {
        val file = answer.optString("bodyFile").takeIf { it.isNotEmpty() }?.let { File(it) }
        val bytes = when {
            answer.optBoolean("noBody") -> null
            file != null -> runCatching { file.readBytes() }.getOrNull().also { runCatching { file.delete() } }
            else -> runCatching { Base64.decode(answer.optString("body"), Base64.DEFAULT) }.getOrElse { ByteArray(0) }
        }
        val mime = answer.optString("mime").takeIf { !answer.isNull("mime") && it.isNotEmpty() }
        val encoding = answer.optString("encoding").takeIf { !answer.isNull("encoding") && it.isNotEmpty() }
        val data = bytes?.let { ByteArrayInputStream(it) }
        val status = answer.optInt("status")
        // WebView takes 100-299 and 400-599 with a reason; anything else as a plain answer.
        if (status !in 100..299 && status !in 400..599) return WebResourceResponse(mime, encoding, data)
        val headers = mutableMapOf<String, String>()
        answer.optJSONObject("headers")?.let { given -> given.keys().forEach { name -> headers[name] = given.optString(name) } }
        val reason = answer.optString("reason").ifBlank { if (status < 300) "OK" else "Error" }
        return WebResourceResponse(mime, encoding, status, reason, headers, data)
    }

    /** Whether the view may load from `url`: a host the source may reach, never a check's provider. */
    private fun allowed(url: Uri): Boolean {
        val scheme = url.scheme?.lowercase() ?: return false
        if (scheme != "http" && scheme != "https") return false
        val host = url.host?.lowercase()?.trimEnd('.') ?: return false
        if (isChallenge(host, url.path.orEmpty())) return false
        return hosts.any { pattern -> matches(pattern.lowercase(), host) }
    }

    private fun isChallenge(host: String, path: String): Boolean =
        challengeHosts.any { known -> host == known || host.endsWith(".$known") } ||
            challengePaths.any { path.startsWith(it) }

    /** `raygun_ext_host::permissions::HostPattern`: a host, `*.` its subdomains, `*` any public host. */
    private fun matches(pattern: String, host: String): Boolean = when {
        pattern == "*" -> isPublic(host)
        pattern.startsWith("*.") -> host.endsWith(pattern.substring(1)) && host.length > pattern.length - 1
        else -> host == pattern
    }

    /** `raygun_ext_host::permissions::is_public_host`, for names; addresses by their kind. */
    private fun isPublic(host: String): Boolean {
        val bare = host.trim('[', ']')
        val literal = bare.isNotEmpty() && (bare.all { it.isDigit() || it == '.' } || bare.contains(':'))
        if (literal) {
            val address = runCatching { InetAddress.getByName(bare) }.getOrNull() ?: return false
            val bytes = address.address
            val sharedRange = bytes.size == 4 && (bytes[0].toInt() and 0xff) == 100 && (bytes[1].toInt() and 0xc0) == 64
            val uniqueLocal = bytes.size == 16 && (bytes[0].toInt() and 0xfe) == 0xfc
            return !(
                address.isLoopbackAddress || address.isSiteLocalAddress || address.isLinkLocalAddress ||
                    address.isAnyLocalAddress || address.isMulticastAddress || sharedRange || uniqueLocal ||
                    (bytes.size == 4 && bytes[0].toInt() == 0)
                )
        }
        return !(
            bare == "localhost" || bare.endsWith(".localhost") || bare.endsWith(".local") ||
                bare.endsWith(".internal") || bare.endsWith(".lan") || bare.endsWith(".home.arpa") || !bare.contains('.')
            )
    }

    private fun blocked(): WebResourceResponse = WebResourceResponse("text/plain", "utf-8", 403, "Blocked", emptyMap(), ByteArrayInputStream(ByteArray(0)))

    private fun originOf(url: String): String = runCatching {
        val uri = Uri.parse(url)
        val port = if (uri.port == -1) "" else ":${uri.port}"
        "${uri.scheme}://${uri.host}$port"
    }.getOrDefault("")

    private fun allowsOrigin(origins: List<String>, url: String): Boolean {
        val origin = originOf(url)
        return origins.any { rule -> rule == "*" || rule.trimEnd('/') == origin }
    }

    private inner class Client : WebViewClient() {
        override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) {
            val address = url.orEmpty()
            if (address.startsWith("http") && !pages.contains(address) && pages.size < 32) pages += address
            emit("started", JSONObject().put("url", address))
            for ((script, origins) in lateScripts) {
                if (allowsOrigin(origins, address)) view.evaluateJavascript(script, null)
            }
        }

        override fun onPageFinished(view: WebView, url: String?) {
            emit("finished", JSONObject().put("url", url.orEmpty()))
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            emit(
                "failed",
                JSONObject()
                    .put("url", request.url.toString())
                    .put("code", error.errorCode)
                    .put("description", error.description?.toString().orEmpty())
                    .put("mainFrame", request.isForMainFrame),
            )
        }

        override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, response: WebResourceResponse) {
            if (!request.isForMainFrame) return
            val mitigated = response.responseHeaders.orEmpty().any { (name, value) ->
                name.equals("cf-mitigated", ignoreCase = true) && value?.trim().equals("challenge", ignoreCase = true)
            }
            emit("answer", JSONObject().put("url", request.url.toString()).put("status", response.statusCode).put("mitigated", mitigated))
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            // Apps and other schemes never open from a hidden view.
            val scheme = request.url.scheme?.lowercase()
            return scheme != "http" && scheme != "https" && scheme != "about" && scheme != "data"
        }

        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            closed = true
            service.gone(id, detail.didCrash())
            return true
        }

        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
            if (closed) return blocked()
            val url = request.url
            when (url.scheme?.lowercase()) {
                "data", "blob", "about" -> return null
            }
            if (!allowed(url)) return blocked()
            if (!intercept) return null
            val number = nextRequest.incrementAndGet()
            val waiting = Pending()
            pending[number] = waiting
            val headers = JSONObject()
            request.requestHeaders?.forEach { (name, value) -> headers.put(name, value) }
            emit(
                "request",
                JSONObject()
                    .put("request", number)
                    .put("url", url.toString())
                    .put("method", request.method)
                    .put("headers", headers)
                    .put("mainFrame", request.isForMainFrame),
            )
            val left = deadline - SystemClock.uptimeMillis()
            val answered = left > 0 && runCatching { waiting.latch.await(left, TimeUnit.MILLISECONDS) }.getOrDefault(false)
            pending.remove(number)
            if (!answered || closed || !waiting.answered) return blocked()
            return waiting.response
        }
    }
}

private fun strings(array: JSONArray?): List<String> {
    array ?: return emptyList()
    return (0 until array.length()).mapNotNull { index -> array.optString(index).takeIf { it.isNotEmpty() } }
}

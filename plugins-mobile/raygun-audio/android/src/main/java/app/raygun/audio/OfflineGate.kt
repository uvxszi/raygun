package app.raygun.audio

import android.graphics.Bitmap
import android.net.Uri
import android.net.http.SslError
import android.os.Message
import android.view.KeyEvent
import android.webkit.ClientCertRequest
import android.webkit.HttpAuthHandler
import android.webkit.RenderProcessGoneDetail
import android.webkit.SslErrorHandler
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream

/**
 * Offline Mode in Raygun's own webview: while it's on, nothing the page asks of the network loads;
 * only this device answers (Raygun's pages, served in place, and the stream proxy on loopback).
 * Rust sets it with every change (`setOfflineGate`, `NativeAudio::offline_gate`).
 */
object OfflineGate {
    @Volatile
    var on: Boolean = false

    /** Whether the page may load `url` now. */
    fun allows(url: Uri): Boolean {
        if (!on) return true
        val scheme = url.scheme?.lowercase() ?: return true
        // data:, blob:, file: and Raygun's own schemes never reach the network.
        if (scheme != "http" && scheme != "https" && scheme != "ws" && scheme != "wss") return true
        val host = url.host?.lowercase()?.trim('[', ']')?.trimEnd('.') ?: return false
        return host == "localhost" ||
            host.endsWith(".localhost") ||
            host == "::1" ||
            host.startsWith("127.") ||
            host == "::ffff:127.0.0.1"
    }

    /** What a refused load gets: an empty answer, as a site that isn't there. */
    fun refusal(): WebResourceResponse =
        WebResourceResponse(
            "text/plain",
            "utf-8",
            503,
            "Offline mode",
            mapOf("Cache-Control" to "no-store"),
            ByteArrayInputStream(ByteArray(0)),
        )
}

/**
 * Raygun's webview client as wry made it, with Offline Mode's gate before its own interception.
 * Everything else goes to wry's client as it is, so the page loads and talks to Rust as before.
 */
class OfflineGateClient(private val inner: WebViewClient) : WebViewClient() {
    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest,
    ): WebResourceResponse? =
        if (OfflineGate.allows(request.url)) {
            inner.shouldInterceptRequest(view, request)
        } else {
            OfflineGate.refusal()
        }

    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
        inner.shouldOverrideUrlLoading(view, request)

    override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) =
        inner.onPageStarted(view, url, favicon)

    override fun onPageFinished(view: WebView, url: String) = inner.onPageFinished(view, url)

    override fun onPageCommitVisible(view: WebView, url: String) =
        inner.onPageCommitVisible(view, url)

    override fun onLoadResource(view: WebView, url: String) = inner.onLoadResource(view, url)

    override fun onReceivedError(
        view: WebView,
        request: WebResourceRequest,
        error: WebResourceError,
    ) {
        inner.onReceivedError(view, request, error)
        // Raygun's own page didn't load (wry retries refused connections itself): its error
        // screen rather than a blank one.
        if (request.isForMainFrame &&
            request.url.host == RAYGUN_HOST &&
            error.errorCode != ERROR_CONNECT &&
            error.errorCode != ERROR_UNKNOWN &&
            !OfflineGate.on
        ) {
            CrashLog.write(
                view.context,
                "page",
                "Raygun's page couldn't load: ${error.description}",
                "Address: ${request.url}\nError ${error.errorCode}: ${error.description}\n",
            )
            leave(view, "page")
        }
    }

    override fun onReceivedHttpError(
        view: WebView,
        request: WebResourceRequest,
        errorResponse: WebResourceResponse,
    ) = inner.onReceivedHttpError(view, request, errorResponse)

    override fun onReceivedSslError(view: WebView, handler: SslErrorHandler, error: SslError) =
        inner.onReceivedSslError(view, handler, error)

    override fun onReceivedClientCertRequest(view: WebView, request: ClientCertRequest) =
        inner.onReceivedClientCertRequest(view, request)

    override fun onReceivedHttpAuthRequest(
        view: WebView,
        handler: HttpAuthHandler,
        host: String,
        realm: String,
    ) = inner.onReceivedHttpAuthRequest(view, handler, host, realm)

    override fun onFormResubmission(view: WebView, dontResend: Message, resend: Message) =
        inner.onFormResubmission(view, dontResend, resend)

    override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) =
        inner.doUpdateVisitedHistory(view, url, isReload)

    /**
     * The page's renderer went (it crashed, or the system ended it for memory). wry doesn't handle
     * it, and unhandled, Android ends Raygun with it: every start again if what the page restores
     * is what ends it. Handled here: a crash record, Raygun's error screen when it's in front, and
     * this activity goes (its webview can't be used again); music playing goes on.
     */
    @androidx.annotation.RequiresApi(26)
    override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
        CrashLog.rendererGone(view.context, detail)
        leave(view, "renderer")
        return true
    }

    private fun leave(view: WebView, kind: String) {
        val activity = activityOf(view.context) ?: return
        if (CrashLog.mainInFront) CrashLog.showErrorScreen(activity, kind)
        runCatching { (view.parent as? android.view.ViewGroup)?.removeView(view) }
        activity.finish()
    }

    private fun activityOf(context: android.content.Context?): android.app.Activity? {
        var current = context
        while (current is android.content.ContextWrapper) {
            if (current is android.app.Activity) return current
            current = current.baseContext
        }
        return null
    }

    private companion object {
        /** Where wry serves Raygun's own page on Android. */
        const val RAYGUN_HOST = "tauri.localhost"
    }

    override fun shouldOverrideKeyEvent(view: WebView, event: KeyEvent): Boolean =
        inner.shouldOverrideKeyEvent(view, event)

    override fun onUnhandledKeyEvent(view: WebView, event: KeyEvent) =
        inner.onUnhandledKeyEvent(view, event)

    override fun onScaleChanged(view: WebView, oldScale: Float, newScale: Float) =
        inner.onScaleChanged(view, oldScale, newScale)
}

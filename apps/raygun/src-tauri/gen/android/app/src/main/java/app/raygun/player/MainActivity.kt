package app.raygun.player

import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.raygun.audio.OpenWith
import app.raygun.audio.RaygunWindow
import app.raygun.audio.WebAwake

class MainActivity : TauriActivity() {
  /** Lets mDNS (device sync discovery) through Android's Wi-Fi multicast filter. */
  private var multicastLock: WifiManager.MulticastLock? = null

  /** wry's own webview field (`WryActivity.mWebView`), or null if a wry version renamed it. */
  private val wryWebView: java.lang.reflect.Field? by lazy {
    runCatching {
      WryActivity::class.java.getDeclaredField("mWebView").apply { isAccessible = true }
    }.getOrNull()
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    // Recreated after Android stopped Raygun in the background: what another app opened it with
    // played then, and doesn't again.
    if (savedInstanceState != null) OpenWith.handled(intent)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    resizeForKeyboard()
    // Opened again in a process that carried on without an activity (music played on with
    // Raygun swiped away): the main window is made again for this one.
    RaygunWindow.created(this)
  }

  override fun onDestroy() {
    val webView = if (isChangingConfigurations) null else wryWebView?.get(this) as? WebView
    // Before wry hears of it: Tauri stops sending this webview scripts (see RaygunWindow).
    RaygunWindow.closing(this)
    RaygunWindow.destroyed(this)
    super.onDestroy()
    // Raygun may carry on without this activity (music plays on): its webview goes with it, or
    // YouTube's player in it (kept awake by WebAwake) would play on where nothing controls it.
    webView?.let { view ->
      (view.parent as? ViewGroup)?.removeView(view)
      view.destroy()
    }
  }

  /**
   * Another "Open with" or "Share" while Raygun runs. It becomes the activity's intent too, so the
   * plugin still finds it when it loads after this (the app was restarted for the intent).
   */
  override fun onNewIntent(intent: Intent) {
    setIntent(intent)
    super.onNewIntent(intent)
  }

  /** wry hands over its webview here: it goes in a parent that can keep it playing off screen. */
  override fun setContentView(view: View?) {
    super.setContentView(view?.let { WebAwake.host(this, it) })
  }

  // Held only while Raygun is in front: it costs battery, and sync runs in the foreground (M21).
  override fun onResume() {
    super.onResume()
    WebAwake.resumed()
    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
    multicastLock =
      wifi?.createMulticastLock("raygun-sync")?.apply {
        setReferenceCounted(false)
        acquire()
      }
  }

  override fun onPause() {
    multicastLock?.release()
    multicastLock = null
    // While a song plays in the webview (YouTube's player), wry mustn't pause it at all: even a
    // moment's pause hides the page, and YouTube's player stops and stays stopped when the page
    // shows again. Its webview field is set aside while wry pauses the activity.
    val field = if (WebAwake.keepAwake) wryWebView else null
    val webView = field?.get(this)
    if (webView != null) field?.set(this, null)
    try {
      super.onPause()
    } finally {
      if (webView != null) field?.set(this, webView)
    }
    // After wry paused the webview: YouTube's player keeps going while a song plays there.
    WebAwake.paused()
  }

  /**
   * Edge-to-edge windows aren't resized for the on-screen keyboard, so bottom sheets and forms
   * would sit under it. Shrink the content by the keyboard's height instead.
   */
  private fun resizeForKeyboard() {
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      view.setPadding(0, 0, 0, keyboard)
      insets
    }
  }
}

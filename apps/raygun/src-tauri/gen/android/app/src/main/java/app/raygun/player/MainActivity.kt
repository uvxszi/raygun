package app.raygun.player

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.raygun.audio.WebAwake

class MainActivity : TauriActivity() {
  /** Lets mDNS (device sync discovery) through Android's Wi-Fi multicast filter. */
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    resizeForKeyboard()
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
    super.onPause()
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

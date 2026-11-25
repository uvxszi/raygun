package app.raygun.audio

import android.app.Service
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Binder
import android.os.IBinder

/**
 * Raygun's process held as important as the browser while the browser is open. The browser runs
 * in its own process (`:web`); with only it on screen Raygun's process counts as cached, so
 * Android freezes it (it then hears nothing the browser sends: downloads waited until Raygun was
 * opened again) or ends it. The browser binds here (this service lives in Raygun's process), which
 * lends Raygun's process the browser's importance. It does nothing itself.
 */
class BrowserLinkService : Service() {
    private val binder = Binder()

    override fun onBind(intent: Intent?): IBinder = binder
}

/** The browser's hold on Raygun's process ([BrowserLinkService]). */
internal class BrowserLink(private val context: Context) {
    private var bound = false
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {}
        override fun onServiceDisconnected(name: ComponentName?) {}
    }

    fun hold() {
        if (bound) return
        bound = runCatching {
            context.bindService(
                Intent(context, BrowserLinkService::class.java),
                connection,
                Context.BIND_AUTO_CREATE or Context.BIND_IMPORTANT,
            )
        }.getOrDefault(false)
    }

    fun release() {
        if (!bound) return
        bound = false
        runCatching { context.unbindService(connection) }
    }
}

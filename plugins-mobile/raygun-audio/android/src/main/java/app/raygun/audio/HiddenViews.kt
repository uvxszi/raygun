package app.raygun.audio

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.util.Base64
import android.util.Log
import org.json.JSONObject
import java.io.File
import java.util.UUID

/**
 * Raygun's side of the hidden web views converted sources drive (`raygun_ext_host::webview`, Keiyoushi's
 * `runWebView`): Rust's commands (`{"op", "id", …}` JSON) go to [HiddenViewService], which runs the views in
 * its own process (`:views`) with its own browser data, through a bound service's [Messenger]; its events
 * (`{"id", "kind", …}`) come back to Rust over JNI ([NativeBridge.webView]). Nothing of Raygun's reaches the
 * views: no bridge, no IPC, only what Rust sends. The service is let go a while after its last view closes,
 * so the system can end its process; if that process ends with views open, Rust hears they're gone.
 */
internal object HiddenViews {
    private const val TAG = "RaygunViews"

    /** An answer's body past this many base64 characters goes through a file (a binder call holds 1 MB). */
    private const val INLINE_BODY = 96_000

    /** How long the service is kept after its last view closed, ms. */
    private const val KEEP_MS = 30_000L

    private val main = Handler(Looper.getMainLooper())
    private val thread = HandlerThread("raygun-views").apply { start() }

    /** The service's events, on a thread of their own. */
    private val events = Messenger(
        Handler(thread.looper) { message ->
            val json = message.data?.getString(HiddenViewService.KEY_JSON)
            if (json != null) heard(json)
            true
        },
    )

    // Only touched on the main thread.
    private var context: Context? = null
    private var service: Messenger? = null
    private var binding = false
    private val waiting = ArrayDeque<String>()
    private val open = mutableSetOf<String>()

    private val letGo = Runnable { if (open.isEmpty()) unbind() }

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            val messenger = Messenger(binder)
            service = messenger
            while (waiting.isNotEmpty()) post(messenger, waiting.removeFirst())
        }

        override fun onServiceDisconnected(name: ComponentName) {
            // Its process ended: its views went with it.
            service = null
            gone()
        }

        override fun onBindingDied(name: ComponentName) {
            service = null
            binding = false
            gone()
            runCatching { context?.unbindService(this) }
        }
    }

    /** A command from Rust. Called on the main thread ([DirectCalls]). */
    fun send(context: Context, json: String) {
        this.context = context.applicationContext
        val command = runCatching { JSONObject(json) }.getOrNull() ?: return
        val id = command.optString("id")
        when (command.optString("op")) {
            "open" -> {
                open += id
                main.removeCallbacks(letGo)
            }
            "close" -> if (!open.contains(id)) {
                // Nothing to close: answer at once.
                heard(JSONObject().put("id", id).put("kind", "closed").toString())
                return
            }
        }
        val body = spill(command)
        val messenger = service
        if (messenger == null) {
            waiting.addLast(body)
            bind()
        } else {
            post(messenger, body)
        }
    }

    private fun bind() {
        val context = context ?: return
        if (binding) return
        binding = runCatching {
            context.bindService(Intent(context, HiddenViewService::class.java), connection, Context.BIND_AUTO_CREATE)
        }.getOrElse { error ->
            Log.w(TAG, "the views' service didn't start", error)
            false
        }
        if (!binding) {
            waiting.clear()
            gone()
        }
    }

    private fun unbind() {
        if (!binding) return
        binding = false
        service = null
        runCatching { context?.unbindService(connection) }
    }

    private fun post(messenger: Messenger, json: String) {
        val message = Message.obtain(null, HiddenViewService.MSG_COMMAND).apply {
            data = Bundle().apply { putString(HiddenViewService.KEY_JSON, json) }
            replyTo = events
        }
        runCatching { messenger.send(message) }.onFailure { error ->
            Log.w(TAG, "a command didn't reach the views", error)
        }
    }

    /** An answer whose body is large goes through a file in the cache, which the views' process reads and deletes. */
    private fun spill(command: JSONObject): String {
        val response = command.optJSONObject("response")
        val body = response?.optString("body").orEmpty()
        if (command.optString("op") != "answer" || body.length <= INLINE_BODY) return command.toString()
        val context = context ?: return command.toString()
        return runCatching {
            val folder = File(context.cacheDir, "views").apply { mkdirs() }
            val file = File(folder, UUID.randomUUID().toString())
            file.writeBytes(Base64.decode(body, Base64.DEFAULT))
            response!!.put("body", "").put("bodyFile", file.absolutePath)
            command.toString()
        }.getOrElse { command.toString() }
    }

    /** An event from the service (or one of its own): to Rust, and its bookkeeping. */
    private fun heard(json: String) {
        NativeBridge.webView(json)
        val event = runCatching { JSONObject(json) }.getOrNull() ?: return
        if (event.optString("kind") == "closed" || event.optString("kind") == "gone") {
            val id = event.optString("id")
            main.post {
                open.remove(id)
                if (open.isEmpty()) {
                    main.removeCallbacks(letGo)
                    main.postDelayed(letGo, KEEP_MS)
                }
            }
        }
    }

    /** The views' process went: every open view is gone. */
    private fun gone() {
        val ids = open.toList()
        open.clear()
        for (id in ids) {
            NativeBridge.webView(JSONObject().put("id", id).put("kind", "gone").put("crashed", true).toString())
            NativeBridge.webView(JSONObject().put("id", id).put("kind", "closed").toString())
        }
    }
}

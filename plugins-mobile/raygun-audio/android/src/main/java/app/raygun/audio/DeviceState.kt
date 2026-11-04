package app.raygun.audio

import android.animation.ValueAnimator
import android.app.Activity
import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.database.ContentObserver
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import org.json.JSONObject

/**
 * What Rust and the webview adapt to: whether Raygun is on screen, battery saver, and the
 * system's animation settings (accessibility's Remove animations sets every scale to 0; developer
 * options set each scale on its own). Read once at start, then reported through
 * [NativeBridge.deviceState] whenever any of them changes.
 *
 * Raygun counts as on screen while any of its activities in this process is started (the main
 * one or the video player); the websites browser runs in its own process.
 */
object DeviceState {
    private val main = Handler(Looper.getMainLooper())
    private var context: Context? = null
    /** The started activities, by identity. */
    private val started = mutableSetOf<Int>()
    private var lastSent: String? = null

    /** Whether the system lets animations run (Remove animations off, animator scale above 0). */
    fun animatorsEnabled(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ValueAnimator.areAnimatorsEnabled()
        } else {
            context?.let { scale(it, Settings.Global.ANIMATOR_DURATION_SCALE) != 0f } ?: true
        }

    /** Starts following the state; later calls do nothing. Main thread. */
    fun start(activity: Activity) {
        if (context != null) return
        val app = activity.applicationContext
        context = app
        // The activity may already be started (the webview loads after `onStart`).
        val state = (activity as? LifecycleOwner)?.lifecycle?.currentState
        if (state == null || state.isAtLeast(Lifecycle.State.STARTED)) {
            started.add(System.identityHashCode(activity))
        }
        (app as? Application)?.registerActivityLifecycleCallbacks(lifecycle)
        val observer = object : ContentObserver(main) {
            override fun onChange(selfChange: Boolean) = report()
        }
        for (name in listOf(
            Settings.Global.ANIMATOR_DURATION_SCALE,
            Settings.Global.TRANSITION_ANIMATION_SCALE,
            Settings.Global.WINDOW_ANIMATION_SCALE,
        )) {
            app.contentResolver.registerContentObserver(Settings.Global.getUriFor(name), false, observer)
        }
        ContextCompat.registerReceiver(
            app,
            object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) = report()
            },
            IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        report()
    }

    /** The state now, as reported. */
    fun snapshot(): JSONObject {
        val app = context
        val power = app?.getSystemService(PowerManager::class.java)
        return JSONObject()
            .put("foreground", started.isNotEmpty())
            .put("powerSave", power?.isPowerSaveMode == true)
            .put("animatorScale", app?.let { scale(it, Settings.Global.ANIMATOR_DURATION_SCALE) } ?: 1f)
            .put("transitionScale", app?.let { scale(it, Settings.Global.TRANSITION_ANIMATION_SCALE) } ?: 1f)
            .put("windowScale", app?.let { scale(it, Settings.Global.WINDOW_ANIMATION_SCALE) } ?: 1f)
            .put("animatorsEnabled", animatorsEnabled())
    }

    private fun report() {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            main.post(::report)
            return
        }
        val json = snapshot().toString()
        if (json == lastSent) return
        lastSent = json
        NativeBridge.deviceState(json)
    }

    private fun scale(context: Context, name: String): Float =
        runCatching { Settings.Global.getFloat(context.contentResolver, name, 1f) }.getOrDefault(1f)

    private val lifecycle = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityStarted(activity: Activity) {
            if (started.add(System.identityHashCode(activity)) && started.size == 1) report()
        }

        override fun onActivityStopped(activity: Activity) {
            if (started.remove(System.identityHashCode(activity)) && started.isEmpty()) report()
        }

        override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit
        override fun onActivityResumed(activity: Activity) = Unit
        override fun onActivityPaused(activity: Activity) = Unit
        override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit
        override fun onActivityDestroyed(activity: Activity) = Unit
    }
}

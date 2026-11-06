package app.raygun.audio

import android.app.Activity

/**
 * Raygun's activity coming back in a process that carried on without one.
 *
 * Swiped away from the recent apps (or left with Back) while a song plays or downloads run, Raygun
 * loses its activity but not its process: the playback or download service keeps it, and Rust
 * keeps working (`raygun-app/src/app_window.rs`). Opened again, Android makes a new activity in
 * that process. Tauri made the main window and loaded the plugins for the first activity only, so
 * the app's `MainActivity` reports both ends here: the plugin moves to the new activity, Rust makes
 * the main window again for it, and what the activity was opened with (a tapped notification,
 * another app's file) goes to the plugin as a new intent would.
 */
object RaygunWindow {
    /** The last activity went for good while the process lived on. Main thread. */
    private var gone = false

    /** From `onDestroy`: a rotation's activity comes back with its window (wry keeps it). */
    @JvmStatic
    fun destroyed(activity: Activity) {
        if (!activity.isChangingConfigurations) gone = true
    }

    /** From `onCreate`, after the activity made its own: it needs the main window again. */
    @JvmStatic
    fun created(activity: Activity) {
        if (!gone) return
        gone = false
        val plugin = DirectCalls.plugin
        plugin?.attach(activity)
        NativeBridge.windowWanted()
        plugin?.onNewIntent(activity.intent)
    }
}

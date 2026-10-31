package app.raygun.audio

import android.util.Log

/**
 * Direct line to the Rust core (`Java_app_raygun_audio_NativeBridge_onEngineEvent` in
 * `plugins-mobile/raygun-audio/src/android.rs`). Engine reports go straight to Rust instead of
 * through the webview, so they still arrive while the webview's JavaScript is paused in the
 * background.
 */
object NativeBridge {
    @JvmStatic
    external fun onEngineEvent(json: String)

    /** A user request from the system UI or a headset, e.g. "next" or "previous". */
    @JvmStatic
    external fun onControl(control: String)

    fun send(json: String) {
        try {
            onEngineEvent(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    fun control(name: String) {
        try {
            onControl(name)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }
}

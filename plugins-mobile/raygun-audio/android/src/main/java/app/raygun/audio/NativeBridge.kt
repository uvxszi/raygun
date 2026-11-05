package app.raygun.audio

import android.util.Log

/**
 * Direct line to the Rust core (`Java_app_raygun_audio_NativeBridge_onEngineEvent` in
 * `plugins-mobile/raygun-audio/src/android.rs`). Engine reports go straight to Rust instead of
 * through the webview, so they still arrive while the webview's JavaScript is paused in the
 * background.
 */
object NativeBridge {
    /** Hands Rust this class, for [call]: once, when the plugin loads. */
    @JvmStatic
    external fun onAttach()

    /**
     * Rust's line to the plugin's commands that work without a window (see [DirectCalls]):
     * `ok:<JSON>` or `error:<message>`. Called on Rust's threads.
     */
    @JvmStatic
    fun call(command: String, args: String): String =
        try {
            DirectCalls.run(command, args)
        } catch (error: Throwable) {
            "error:${error.message ?: error.javaClass.simpleName}"
        }

    fun attach() {
        try {
            onAttach()
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    @JvmStatic
    external fun onEngineEvent(json: String)

    /** A user request from the system UI or a headset, e.g. "next" or "previous". */
    @JvmStatic
    external fun onControl(control: String)

    /** Text-to-speech progress (M33). */
    @JvmStatic
    external fun onTts(json: String)

    /** Video player progress (M38). */
    @JvmStatic
    external fun onVideo(json: String)

    /** The built-in browser's state for a site (M46.3). */
    @JvmStatic
    external fun onWebsite(json: String)

    /** A download notification's button: `{"action", "keys"}`. */
    @JvmStatic
    external fun onNotificationAction(json: String)

    /** On screen or not, battery saver and the system's animation settings (DeviceState). */
    @JvmStatic
    external fun onDeviceState(json: String)

    fun deviceState(json: String) {
        try {
            onDeviceState(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    /** A tapped notification's screen (`player`, `/downloads?tab=music`). */
    @JvmStatic
    external fun onOpen(route: String)

    fun notificationAction(json: String) {
        try {
            onNotificationAction(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    fun open(route: String) {
        try {
            onOpen(route)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    fun website(json: String) {
        try {
            onWebsite(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    fun video(json: String) {
        try {
            onVideo(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

    fun tts(json: String) {
        try {
            onTts(json)
        } catch (error: UnsatisfiedLinkError) {
            Log.e("RaygunAudio", "Rust engine bridge is not linked", error)
        }
    }

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

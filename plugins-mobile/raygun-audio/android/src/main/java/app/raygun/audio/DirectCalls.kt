package app.raygun.audio

import android.os.Handler
import android.os.Looper
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.plugin.Invoke
import org.json.JSONObject
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Method
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** How long a command may take to answer, s (saving a file to a cloud folder can be slow). */
private const val ANSWER_S = 90L

private const val ANSWERED = 0L
private const val FAILED = 1L

/**
 * Runs the plugin's commands for Rust on its own line ([NativeBridge.call]) rather than through
 * Tauri's, which reaches the plugin through Raygun's activity: once that activity is gone (Raygun
 * swiped away while a song plays or downloads run, the process kept by the playback or download
 * service), every call through Tauri fails. Commands run on the main thread, as Tauri runs them,
 * with an [Invoke] whose answer comes back here; commands that need a window (pickers,
 * permission questions) still go through Tauri.
 */
internal object DirectCalls {
    @Volatile
    var plugin: RaygunAudioPlugin? = null

    private val main = Handler(Looper.getMainLooper())

    private val commands: Map<String, Method> by lazy {
        RaygunAudioPlugin::class.java.declaredMethods
            .filter { it.isAnnotationPresent(Command::class.java) }
            .associateBy { it.name }
    }

    /** `ok:<JSON>` with the command's answer, or `error:<message>`. */
    fun run(command: String, args: String): String {
        val plugin = plugin ?: return "error:Raygun's player isn't ready"
        val method = commands[command] ?: return "error:The player has no command $command"
        val mapper = runCatching { plugin.jsonMapper() }.getOrNull()
            ?: return "error:Raygun's player isn't ready"
        val answered = CountDownLatch(1)
        var answer = "error:The player didn't answer $command"
        val invoke = Invoke(
            0L,
            command,
            ANSWERED,
            FAILED,
            { callback, data ->
                answer = if (callback == ANSWERED) "ok:$data" else "error:${message(data)}"
                answered.countDown()
            },
            args,
            mapper,
        )
        val task = Runnable {
            try {
                method.invoke(plugin, invoke)
            } catch (error: Throwable) {
                val cause = (error as? InvocationTargetException)?.targetException ?: error
                Log.w("RaygunAudio", "$command failed", cause)
                answer = "error:${cause.message ?: cause.javaClass.simpleName}"
                answered.countDown()
            }
        }
        if (Looper.myLooper() == Looper.getMainLooper()) task.run() else main.post(task)
        if (!answered.await(ANSWER_S, TimeUnit.SECONDS)) {
            return "error:The player didn't answer $command in time"
        }
        return answer
    }

    /** The message of a rejection (`{"message": …}`), else its text. */
    private fun message(data: String): String =
        runCatching { JSONObject(data).optString("message") }.getOrNull()?.takeIf { it.isNotEmpty() }
            ?: data
}

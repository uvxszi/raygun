package app.raygun.audio

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import org.json.JSONObject

/**
 * Reading aloud for the novel reader (M33) with the phone's text-to-speech engine. Each
 * utterance id is `<paragraph>` or `<paragraph>:end` for the last one queued; progress goes to
 * Rust through [NativeBridge.tts] as `{"state": "speaking"|"ended"|"error", "paragraph": n}`.
 */
class Speech(private val context: Context) {
    private val main = Handler(Looper.getMainLooper())
    private var engine: TextToSpeech? = null
    /** Null while the engine starts. */
    private var ready: Boolean? = null
    private val waiting = mutableListOf<(Boolean) -> Unit>()

    /** Calls [answer] on the main thread with whether speech works, starting the engine first. */
    fun whenReady(answer: (Boolean) -> Unit) {
        main.post {
            ready?.let {
                answer(it)
                return@post
            }
            waiting.add(answer)
            if (engine != null) return@post
            engine = TextToSpeech(context) { status ->
                main.post { started(status == TextToSpeech.SUCCESS) }
            }
        }
    }

    private fun started(success: Boolean) {
        val tts = engine
        ready = success && tts != null
        tts?.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(id: String) = send("speaking", id)

            override fun onDone(id: String) {
                if (id.endsWith(":end")) send("ended", id)
            }

            @Deprecated("Deprecated in Java")
            override fun onError(id: String) = send("error", id)

            override fun onError(id: String, errorCode: Int) = send("error", id)
        })
        waiting.toList().forEach { it(ready == true) }
        waiting.clear()
    }

    private fun send(state: String, id: String) {
        val paragraph = id.substringBefore(':').toIntOrNull() ?: return
        NativeBridge.tts(JSONObject().put("state", state).put("paragraph", paragraph).toString())
    }

    /** Replaces what is being read with [items] (id and text pairs), at [rate] (1 = normal). */
    fun speak(items: List<Pair<String, String>>, rate: Float): Boolean {
        val tts = engine?.takeIf { ready == true } ?: return false
        tts.setSpeechRate(rate)
        items.forEachIndexed { index, (id, text) ->
            val mode = if (index == 0) TextToSpeech.QUEUE_FLUSH else TextToSpeech.QUEUE_ADD
            tts.speak(text, mode, null, id)
        }
        return true
    }

    fun stop() {
        engine?.stop()
    }
}

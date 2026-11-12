package app.raygun.audio

import android.content.Context
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import org.json.JSONObject
import java.util.Locale

/**
 * Reading aloud with the phone's own text-to-speech engine (M33). Rust sends a chapter's
 * sentences; each utterance id is `<run>:<sentence>`, with `:end` on the chapter's last. Progress
 * goes to Rust through [NativeBridge.tts]: `{"state": "speaking"|"ended"|"error", "id"}` as
 * utterances start and the last one ends, and `{"state": "range", "id", "start", "end"}` as each
 * word starts (Android 8 and later), so a pause can go on from the word it stopped at.
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

            override fun onRangeStart(id: String, start: Int, end: Int, frame: Int) {
                NativeBridge.tts(
                    JSONObject().put("state", "range").put("id", id).put("start", start).put("end", end).toString(),
                )
            }
        })
        waiting.toList().forEach { it(ready == true) }
        waiting.clear()
    }

    private fun send(state: String, id: String) {
        NativeBridge.tts(JSONObject().put("state", state).put("id", id).toString())
    }

    /** A sentence to read: its id in reports, and the language of a passage in another one. */
    class Item(val id: String, val text: String, val language: String?)

    /**
     * Replaces what is being read with [items], at [rate] and [pitch] (1 = normal) and [volume]
     * (0–1), in [language] (a BCP 47 tag) when the engine has it, else in its own language. A
     * passage in another language is read in it (each utterance takes the language set when it's
     * queued).
     */
    fun speak(items: List<Item>, rate: Float, pitch: Float, volume: Float, language: String?): Boolean {
        val tts = engine?.takeIf { ready == true } ?: return false
        tts.setSpeechRate(rate)
        tts.setPitch(pitch)
        useLanguage(tts, language)
        var current = language
        val params = Bundle().apply { putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, volume.coerceIn(0f, 1f)) }
        items.forEachIndexed { index, item ->
            val wanted = item.language?.takeIf { it.isNotBlank() } ?: language
            if (wanted != current) {
                useLanguage(tts, wanted)
                current = wanted
            }
            val mode = if (index == 0) TextToSpeech.QUEUE_FLUSH else TextToSpeech.QUEUE_ADD
            tts.speak(item.text, mode, params, item.id)
        }
        return true
    }

    /** The book's language when the engine speaks it; otherwise the engine's own voice. */
    private fun useLanguage(tts: TextToSpeech, language: String?) {
        val locale = language?.takeIf { it.isNotBlank() }?.let { Locale.forLanguageTag(it) }
        try {
            if (locale != null && tts.isLanguageAvailable(locale) >= TextToSpeech.LANG_AVAILABLE) {
                tts.language = locale
            } else {
                tts.defaultVoice?.let { tts.voice = it }
            }
        } catch (_: Exception) {
            // Some engines throw for languages they half know: their own voice reads then.
        }
    }

    fun stop() {
        engine?.stop()
    }
}

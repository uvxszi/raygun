package raygun.mihon.host

import android.content.SharedPreferences
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/** Preferences kept as one JSON object in a file, written on each change. */
class FilePreferences private constructor(private val file: File) : SharedPreferences {
    private val values = ConcurrentHashMap<String, Any>()
    private val listeners = mutableSetOf<SharedPreferences.OnSharedPreferenceChangeListener>()

    init {
        if (file.isFile) {
            runCatching {
                val json = Json.parseToJsonElement(file.readText()) as JsonObject
                for ((key, value) in json) read(value)?.let { values[key] = it }
            }
        }
    }

    private fun read(value: JsonElement): Any? = when (value) {
        is JsonNull -> null
        is JsonArray -> value.map { (it as JsonPrimitive).content }.toSet()
        is JsonPrimitive -> when {
            value.isString -> value.content
            value.booleanOrNull != null -> value.booleanOrNull
            value.longOrNull != null -> value.longOrNull
            else -> value.doubleOrNull
        }
        is JsonObject -> null
    }

    private fun save() {
        val json = JsonObject(
            values.mapValues { (_, value) ->
                when (value) {
                    is String -> JsonPrimitive(value)
                    is Boolean -> JsonPrimitive(value)
                    is Number -> JsonPrimitive(value)
                    is Set<*> -> JsonArray(value.map { JsonPrimitive(it.toString()) })
                    else -> JsonPrimitive(value.toString())
                }
            },
        )
        runCatching {
            file.parentFile?.mkdirs()
            file.writeText(json.toString())
        }
    }

    override fun getAll(): Map<String, *> = HashMap(values)

    override fun getString(key: String, defValue: String?): String? = values[key] as? String ?: defValue

    @Suppress("UNCHECKED_CAST")
    override fun getStringSet(key: String, defValues: Set<String>?): Set<String>? =
        values[key] as? Set<String> ?: defValues

    override fun getInt(key: String, defValue: Int): Int = (values[key] as? Number)?.toInt() ?: defValue

    override fun getLong(key: String, defValue: Long): Long = (values[key] as? Number)?.toLong() ?: defValue

    override fun getFloat(key: String, defValue: Float): Float = (values[key] as? Number)?.toFloat() ?: defValue

    override fun getBoolean(key: String, defValue: Boolean): Boolean = values[key] as? Boolean ?: defValue

    override fun contains(key: String): Boolean = values.containsKey(key)

    override fun edit(): SharedPreferences.Editor = Editor()

    override fun registerOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener,
    ) {
        synchronized(listeners) { listeners += listener }
    }

    override fun unregisterOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener,
    ) {
        synchronized(listeners) { listeners -= listener }
    }

    private inner class Editor : SharedPreferences.Editor {
        private val changes = mutableMapOf<String, Any?>()
        private var clearing = false

        override fun putString(key: String, value: String?) = apply { changes[key] = value }

        override fun putStringSet(key: String, values: Set<String>?) = apply { changes[key] = values?.toSet() }

        override fun putInt(key: String, value: Int) = apply { changes[key] = value }

        override fun putLong(key: String, value: Long) = apply { changes[key] = value }

        override fun putFloat(key: String, value: Float) = apply { changes[key] = value }

        override fun putBoolean(key: String, value: Boolean) = apply { changes[key] = value }

        override fun remove(key: String) = apply { changes[key] = null }

        override fun clear() = apply { clearing = true }

        override fun commit(): Boolean {
            if (clearing) values.clear()
            for ((key, value) in changes) {
                if (value == null) values.remove(key) else values[key] = value
            }
            save()
            val notify = synchronized(listeners) { listeners.toList() }
            for (key in changes.keys) notify.forEach { it.onSharedPreferenceChanged(this@FilePreferences, key) }
            return true
        }

        override fun apply() {
            commit()
        }
    }

    companion object {
        private val open = ConcurrentHashMap<String, FilePreferences>()

        @JvmStatic
        fun open(file: File): SharedPreferences = open.getOrPut(file.absolutePath) { FilePreferences(file) }
    }
}

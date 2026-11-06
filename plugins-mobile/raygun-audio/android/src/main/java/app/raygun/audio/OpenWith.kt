package app.raygun.audio

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.util.Log
import androidx.core.content.IntentCompat
import org.json.JSONArray
import org.json.JSONObject

/**
 * Media and links other apps hand to Raygun: "Open with" (`ACTION_VIEW`) and "Share"
 * (`ACTION_SEND`, `ACTION_SEND_MULTIPLE`), as the app's manifest lists them for its main activity.
 * This only reads the intent (each address with its type, name and size, and shared text) and
 * keeps a document readable for good when the app that shared it allows that. What each item is
 * and where it plays is decided in Rust (`raygun-app/src/open_with`).
 *
 * The read access other apps give with an intent lasts while Raygun's main activity does, which
 * received it: the video player reads a document as it is, and songs are copied in.
 */
object OpenWith {
    private const val TAG = "RaygunOpenWith"

    /** Set on an intent once it's handed over: a recreated activity gets the same intent again. */
    private const val HANDED_OVER = "app.raygun.audio.HANDED_OVER"

    /** Single documents kept readable for good at most; the oldest are let go. */
    private const val KEPT_DOCUMENTS = 64

    /** What an intent hands over, as read from it; [describe] asks the sharing apps for the rest. */
    class Handover internal constructor(
        private val uris: List<Uri>,
        private val given: String?,
        private val forGood: Boolean,
        private val text: String?,
        private val subject: String?,
    ) {
        /** The JSON Rust reads. It asks other apps' providers, so it runs off the main thread. */
        fun describe(context: Context): JSONObject {
            val resolver = context.contentResolver
            val items = JSONArray()
            for (uri in uris) items.put(describe(resolver, uri, given, forGood))
            val handover = JSONObject().put("items", items)
            text?.let { handover.put("text", it) }
            subject?.let { handover.put("subject", it) }
            return handover
        }
    }

    /** Marks [intent] as handed over already: the activity was recreated, and it played then. */
    fun handled(intent: Intent?) {
        intent?.putExtra(HANDED_OVER, true)
    }

    /** What [intent] hands over, or null when nothing was opened with Raygun or shared to it. */
    fun take(intent: Intent?): Handover? {
        val received = intent ?: return null
        if (received.getBooleanExtra(HANDED_OVER, false)) return null
        // Reopened from Recents: Android repeats the intent the activity first started with.
        if (received.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return null
        val uris = when (received.action) {
            Intent.ACTION_VIEW -> listOfNotNull(received.data)
            Intent.ACTION_SEND -> listOfNotNull(stream(received))
            Intent.ACTION_SEND_MULTIPLE -> streams(received)
            else -> return null
        }
        val text = received.takeIf { it.action != Intent.ACTION_VIEW }
            ?.getCharSequenceExtra(Intent.EXTRA_TEXT)
            ?.toString()
            ?.takeIf { it.isNotBlank() }
        if (uris.isEmpty() && text == null) return null
        received.putExtra(HANDED_OVER, true)
        return Handover(
            uris = uris,
            // Several shared files have one type for all of them (`audio/*`): each says its own.
            given = received.type.takeIf { uris.size == 1 },
            forGood = received.flags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION != 0,
            text = text,
            subject = received.getStringExtra(Intent.EXTRA_SUBJECT)?.takeIf { it.isNotBlank() },
        )
    }

    private fun stream(intent: Intent): Uri? =
        runCatching { IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java) }.getOrNull()
            ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri

    private fun streams(intent: Intent): List<Uri> {
        val listed = runCatching {
            IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
        }.getOrNull().orEmpty()
        val clipped = intent.clipData
            ?.let { clip -> (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri } }
            .orEmpty()
        return (listed + clipped).distinct()
    }

    /** An address with its type, and for a document its name and size as its app gives them. */
    private fun describe(resolver: ContentResolver, uri: Uri, given: String?, forGood: Boolean): JSONObject {
        val item = JSONObject().put("uri", uri.toString())
        val document = uri.scheme.equals(ContentResolver.SCHEME_CONTENT, ignoreCase = true)
        val provided = if (document) runCatching { resolver.getType(uri) }.getOrNull() else null
        (provided ?: given)?.takeIf { it.isNotBlank() && it != "*/*" }?.let { item.put("mime", it) }
        if (!document) return item
        try {
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)
                ?.use { row ->
                    if (row.moveToFirst()) {
                        val name = row.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                        if (name >= 0 && !row.isNull(name)) item.put("name", row.getString(name))
                        val size = row.getColumnIndex(OpenableColumns.SIZE)
                        if (size >= 0 && !row.isNull(size)) item.put("size", row.getLong(size))
                    }
                }
        } catch (error: Exception) {
            Log.w(TAG, "couldn't read the name of $uri", error)
        }
        if (forGood) keep(resolver, uri)
        return item
    }

    /** Keeps [uri] readable after Raygun closes; the oldest single documents kept are let go. */
    private fun keep(resolver: ContentResolver, uri: Uri) {
        try {
            resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        } catch (error: SecurityException) {
            return
        }
        // Folders (trees) are Local Files' and the save folders'; only single documents go.
        resolver.persistedUriPermissions
            .filter { !DocumentsContract.isTreeUri(it.uri) }
            .sortedByDescending { it.persistedTime }
            .drop(KEPT_DOCUMENTS)
            .forEach { old ->
                val flags = (if (old.isReadPermission) Intent.FLAG_GRANT_READ_URI_PERMISSION else 0) or
                    (if (old.isWritePermission) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)
                runCatching { resolver.releasePersistableUriPermission(old.uri, flags) }
            }
    }
}

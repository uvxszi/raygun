package app.raygun.audio

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.provider.Settings
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject

/**
 * Local Files' own folder browser (M46.51). Android's folder picker refuses the storage's top,
 * Download and Android/data, where people keep their files, so Raygun lists shared storage
 * itself: by path with access to all files (Rust walks it), or else as MediaStore knows it, which
 * the media permissions allow and which lists songs, videos and pictures (other apps' books and
 * archives aren't in it).
 */
internal object LocalStorage {
    /** Shared storage's top folder: `/storage/emulated/0`. */
    @Suppress("DEPRECATION")
    fun root(): String = Environment.getExternalStorageDirectory().absolutePath.trimEnd('/')

    /** Whether every file in shared storage reads by path: access to all files, or (Android 10 and older) the storage permission. */
    fun allFiles(storageGranted: Boolean): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) Environment.isExternalStorageManager() else storageGranted

    fun canAskAllFiles(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R

    /** The page where access to all files is allowed: Raygun's own, else the list of apps. */
    fun allFilesIntents(context: Context): List<Intent> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            listOf(
                Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${context.packageName}")),
                Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION),
            )
        } else {
            emptyList()
        }

    /**
     * The folders in `path` holding files with these extensions (lower case, no dot), with how
     * many each holds (its own folders too), and how many are right in it. Hidden folders are
     * left out, as Rust's walk leaves them out.
     */
    fun mediaFolders(context: Context, path: String, extensions: Set<String>): JSObject {
        val root = root()
        val folder = path.trimEnd('/')
        require(folder == root || folder.startsWith("$root/")) { "This folder isn't on the phone's storage" }
        val inside = if (folder == root) "" else folder.removePrefix("$root/")
        val prefix = if (inside.isEmpty()) "" else "$inside/"
        // RELATIVE_PATH (Android 10+) is the file's folder in its volume ("Download/Shows/");
        // before it, DATA is the file's whole path.
        val modern = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
        val collection = if (modern) {
            MediaStore.Files.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
        } else {
            MediaStore.Files.getContentUri("external")
        }
        @Suppress("DEPRECATION")
        val column = if (modern) MediaStore.MediaColumns.RELATIVE_PATH else MediaStore.MediaColumns.DATA
        val start = if (modern) prefix else "$root/$prefix"
        val selection = if (start.isEmpty()) null else "$column LIKE ? ESCAPE '\\'"
        val arguments = if (start.isEmpty()) null else arrayOf(escapeLike(start) + "%")
        // By lower-case name: the name as first seen, and the count.
        val folders = linkedMapOf<String, Pair<String, Int>>()
        var here = 0
        context.contentResolver.query(
            collection,
            arrayOf(column, MediaStore.MediaColumns.DISPLAY_NAME),
            selection,
            arguments,
            null,
        )?.use { cursor ->
            while (cursor.moveToNext()) {
                val name = cursor.getString(1) ?: continue
                if (name.substringAfterLast('.', "").lowercase() !in extensions) continue
                val location = cursor.getString(0) ?: continue
                val parent = if (modern) location else location.substringBeforeLast('/', "") + "/"
                if (!parent.regionMatches(0, start, 0, start.length, ignoreCase = true)) continue
                val rest = parent.substring(start.length).trim('/')
                if (rest.isEmpty()) {
                    here++
                    continue
                }
                val segments = rest.split('/')
                if (segments.any { it.startsWith(".") }) continue
                val first = segments.first()
                val key = first.lowercase()
                val (shown, count) = folders[key] ?: (first to 0)
                folders[key] = shown to count + 1
            }
        }
        val list = JSArray()
        folders.values.forEach { (name, count) -> list.put(JSObject().put("name", name).put("items", count)) }
        return JSObject().put("folders", list).put("items", here)
    }

    /** A picked document's name and size, from its provider. */
    fun describe(context: Context, uri: Uri): JSObject {
        var name: String? = null
        var size = 0L
        runCatching {
            context.contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE),
                null,
                null,
                null,
            )?.use { cursor ->
                if (cursor.moveToFirst()) {
                    name = cursor.getString(0)
                    size = if (cursor.isNull(1)) 0L else cursor.getLong(1)
                }
            }
        }
        return JSObject()
            .put("uri", uri.toString())
            .put("name", name ?: uri.lastPathSegment?.substringAfterLast('/') ?: "file")
            .put("size", size)
            .put("mime", runCatching { context.contentResolver.getType(uri) }.getOrNull())
    }

    private fun escapeLike(text: String) =
        text.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
}

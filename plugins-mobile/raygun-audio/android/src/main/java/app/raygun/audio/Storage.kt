package app.raygun.audio

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.MediaStore
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File

/** A file made for Rust to write: where it is and the descriptor Rust now owns. */
internal class Output(val uri: Uri, val fd: Int, val name: String)

/** `name`, or `name (1)`, `name (2)`… before its extension: the first one not `taken` (as Rust's `unique_name`). */
internal fun uniqueName(name: String, taken: (String) -> Boolean): String {
    if (!taken(name)) return name
    val dot = name.lastIndexOf('.')
    val extension = name.substring(dot + 1)
    val hasExtension = dot > 0 && extension.isNotEmpty() && extension.length <= 8 && ' ' !in extension
    val stem = if (hasExtension) name.substring(0, dot) else name
    for (number in 1 until 10_000) {
        val candidate = if (hasExtension) "$stem ($number).$extension" else "$stem ($number)"
        if (!taken(candidate)) return candidate
    }
    return "$stem (${System.currentTimeMillis()})" + if (hasExtension) ".$extension" else ""
}

/**
 * Files Raygun writes outside its own storage (M46.1): the phone's Downloads and Pictures
 * folders (MediaStore, no permission on Android 10+), or a folder the user picked (the Storage
 * Access Framework). Rust can't open `content://` URIs, so each file is created here and its
 * descriptor handed over; Rust writes it like any file and calls [finish] when done.
 */
internal class Storage(private val context: Context) {
    private val resolver get() = context.contentResolver
    /** Folder document URIs already found or made inside picked folders, by tree and path. */
    private val folders = mutableMapOf<String, Uri>()

    /** The picked folder's name, and whether Raygun may still write to it. */
    fun folderState(tree: Uri): Pair<Boolean, String?> {
        val granted = resolver.persistedUriPermissions.any { it.uri == tree && it.isWritePermission }
        return granted to if (granted) folderName(tree) else null
    }

    /** Keeps the picker's grant across restarts; some devices refuse, which only lasts a session. */
    fun keep(tree: Uri): String? {
        runCatching {
            resolver.takePersistableUriPermission(
                tree,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
            )
        }
        return folderName(tree)
    }

    fun release(tree: Uri) {
        runCatching {
            resolver.releasePersistableUriPermission(
                tree,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
            )
        }
        folders.keys.removeAll { it.startsWith(tree.toString()) }
    }

    private fun folderName(tree: Uri): String? = runCatching {
        val document = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        resolver.query(document, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        }
    }.getOrNull()

    /**
     * Makes `name` in `place` (`downloads`, `pictures` or `tree`) under the folders in `path`,
     * replacing a file of the same name there (or, `unique`, keeping it and taking `name (1)`…),
     * and opens it for writing.
     */
    fun create(place: String, tree: String?, path: List<String>, name: String, mime: String, unique: Boolean = false): Output = when (place) {
        "tree" -> createInTree(Uri.parse(tree ?: error("No folder was picked")), path, name, mime, unique)
        "downloads", "pictures", "music" ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) createInMediaStore(place, path, name, mime, unique)
            else createInPublicFolder(place, path, name, unique)
        else -> error("Unknown place $place")
    }

    private fun createInMediaStore(place: String, path: List<String>, wanted: String, mime: String, unique: Boolean): Output {
        // MediaStore's picture collection refuses types Android doesn't know, such as JPEG XL
        // (M46.38): those go to Download/Raygun.
        val pictures = place == "pictures" && mime.startsWith("image/") &&
            android.webkit.MimeTypeMap.getSingleton().hasMimeType(mime)
        // Saved songs go to Music/Raygun/<artist>/, where music players look (M46.10).
        val music = place == "music" && mime.startsWith("audio/")
        val collection = when {
            pictures -> MediaStore.Images.Media.EXTERNAL_CONTENT_URI
            music -> MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
            else -> MediaStore.Downloads.EXTERNAL_CONTENT_URI
        }
        val top = when {
            pictures -> Environment.DIRECTORY_PICTURES
            music -> Environment.DIRECTORY_MUSIC
            else -> Environment.DIRECTORY_DOWNLOADS
        }
        val relative = (listOf(top, "Raygun") + path).joinToString("/") + "/"
        val named = { candidate: String ->
            resolver.query(
                collection,
                arrayOf(MediaStore.MediaColumns._ID),
                "${MediaStore.MediaColumns.RELATIVE_PATH}=? AND ${MediaStore.MediaColumns.DISPLAY_NAME}=?",
                arrayOf(relative, candidate),
                null,
            )?.use { it.moveToFirst() } ?: false
        }
        val name = if (unique) uniqueName(wanted, named) else wanted
        val existing = resolver.query(
            collection,
            arrayOf(MediaStore.MediaColumns._ID),
            "${MediaStore.MediaColumns.RELATIVE_PATH}=? AND ${MediaStore.MediaColumns.DISPLAY_NAME}=?",
            arrayOf(relative, name),
            null,
        )?.use { if (it.moveToFirst()) it.getLong(0) else null }
        val uri = if (existing != null) {
            val uri = android.content.ContentUris.withAppendedId(collection, existing)
            resolver.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 1) }, null, null)
            uri
        } else {
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                put(MediaStore.MediaColumns.MIME_TYPE, mime)
                put(MediaStore.MediaColumns.RELATIVE_PATH, relative)
                put(MediaStore.MediaColumns.IS_PENDING, 1)
            }
            resolver.insert(collection, values) ?: error("Android refused to make $name")
        }
        // MediaStore may still rename it; the name it holds is the one shown.
        val given = resolver.query(uri, arrayOf(MediaStore.MediaColumns.DISPLAY_NAME), null, null, null)
            ?.use { if (it.moveToFirst()) it.getString(0) else null }
        return Output(uri, open(uri), given ?: name)
    }

    /** Android 9 and older: plain folders under shared storage. */
    private fun createInPublicFolder(place: String, path: List<String>, wanted: String, unique: Boolean): Output {
        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
            PackageManager.PERMISSION_GRANTED
        if (!granted) error("Allow Raygun to use storage in the system settings to save files")
        @Suppress("DEPRECATION")
        val top = Environment.getExternalStoragePublicDirectory(
            when (place) {
                "pictures" -> Environment.DIRECTORY_PICTURES
                "music" -> Environment.DIRECTORY_MUSIC
                else -> Environment.DIRECTORY_DOWNLOADS
            },
        )
        val dir = File(top, (listOf("Raygun") + path).joinToString(File.separator))
        if (!dir.isDirectory && !dir.mkdirs()) error("Couldn't make ${dir.path}")
        val name = if (unique) uniqueName(wanted) { File(dir, it).exists() } else wanted
        val file = File(dir, name)
        val descriptor = ParcelFileDescriptor.open(
            file,
            ParcelFileDescriptor.MODE_WRITE_ONLY or ParcelFileDescriptor.MODE_CREATE or ParcelFileDescriptor.MODE_TRUNCATE,
        )
        return Output(Uri.fromFile(file), descriptor.detachFd(), name)
    }

    private fun createInTree(tree: Uri, path: List<String>, wanted: String, mime: String, unique: Boolean): Output {
        var parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        var key = tree.toString()
        for (segment in path) {
            key += "/$segment"
            val known = folders[key]
            parent = known ?: (child(tree, parent, segment, directory = true)
                ?: DocumentsContract.createDocument(resolver, parent, DocumentsContract.Document.MIME_TYPE_DIR, segment)
                ?: error("Couldn't make the folder $segment"))
            folders[key] = parent
        }
        val folder = parent
        val name = if (unique) uniqueName(wanted) { child(tree, folder, it, directory = false) != null } else wanted
        val uri = (if (unique) null else child(tree, parent, name, directory = false))
            ?: DocumentsContract.createDocument(resolver, parent, mime, name)
            ?: error("Couldn't make $name")
        // The provider may change the name (an extension added); the one it holds is shown.
        val given = runCatching {
            resolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)
                ?.use { if (it.moveToFirst()) it.getString(0) else null }
        }.getOrNull()
        return Output(uri, open(uri), given ?: name)
    }

    /** A child document of `parent` named `name`, if there is one. */
    private fun child(tree: Uri, parent: Uri, name: String, directory: Boolean): Uri? {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent))
        val columns = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        resolver.query(children, columns, null, null, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                val isDirectory = cursor.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                if (cursor.getString(1) == name && isDirectory == directory) {
                    return DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0))
                }
            }
        }
        return null
    }

    /** Opens `uri` for writing from the start; some providers only know "w". */
    private fun open(uri: Uri): Int {
        val descriptor = runCatching { resolver.openFileDescriptor(uri, "wt") }.getOrNull()
            ?: resolver.openFileDescriptor(uri, "w")
            ?: error("Couldn't open the file")
        return descriptor.detachFd()
    }

    /** Publishes a finished file, or removes one whose writing failed. */
    fun finish(uri: Uri, ok: Boolean) {
        when {
            uri.scheme == "file" -> if (!ok) File(uri.path ?: return).delete()
            uri.authority == MediaStore.AUTHORITY ->
                if (ok) resolver.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
                else resolver.delete(uri, null, null)
            !ok -> runCatching { DocumentsContract.deleteDocument(resolver, uri) }
        }
    }

    /** Deletes a saved file: a MediaStore entry, a picked folder's document or a plain file. */
    fun delete(uri: Uri) {
        when {
            uri.scheme == "file" -> {
                val file = File(uri.path ?: return)
                if (file.exists() && !file.delete()) error("The file couldn't be deleted")
            }
            uri.authority == MediaStore.AUTHORITY -> resolver.delete(uri, null, null)
            else -> if (!DocumentsContract.deleteDocument(resolver, uri)) error("The file couldn't be deleted")
        }
    }

    /** Opens a saved file in the app Android picks. */
    fun view(uri: Uri, mime: String): Intent {
        val shown = if (uri.scheme == "file") shareUriOrNull(uri.path) ?: uri else uri
        val type = mime.takeIf { it.isNotBlank() && it != "*/*" }
            ?: resolver.getType(shown)
            ?: "*/*"
        val view = Intent(Intent.ACTION_VIEW)
            .setDataAndType(shown, type)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        return Intent.createChooser(view, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    /** The folder a saved file is in: a picked folder's own view, else the system's downloads. */
    fun folderOf(uri: Uri): Intent {
        if (uri.authority != MediaStore.AUTHORITY && uri.scheme == "content" && DocumentsContract.isDocumentUri(context, uri)) {
            runCatching {
                val tree = DocumentsContract.getTreeDocumentId(uri)
                val folder = DocumentsContract.buildDocumentUriUsingTree(uri, tree)
                return Intent(Intent.ACTION_VIEW)
                    .setDataAndType(folder, DocumentsContract.Document.MIME_TYPE_DIR)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }
        return Intent(android.app.DownloadManager.ACTION_VIEW_DOWNLOADS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private fun shareUriOrNull(path: String?): Uri? =
        path?.let { runCatching { shareUri(it) }.getOrNull() }

    private fun shareUri(path: String): Uri =
        FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", File(path))

    /** The system share sheet for a file in the app's cache. */
    fun share(path: String, mime: String, title: String): Intent {
        val send = Intent(Intent.ACTION_SEND)
            .setType(mime)
            .putExtra(Intent.EXTRA_STREAM, shareUri(path))
            .putExtra(Intent.EXTRA_TITLE, title)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return Intent.createChooser(send, title)
    }

    /** Puts an image from the app's cache on the clipboard. */
    fun copyImage(path: String, label: String) {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newUri(resolver, label, shareUri(path)))
    }
}

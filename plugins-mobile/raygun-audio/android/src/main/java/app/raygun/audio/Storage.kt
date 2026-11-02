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
internal class Output(val uri: Uri, val fd: Int)

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
     * replacing a file of the same name there, and opens it for writing.
     */
    fun create(place: String, tree: String?, path: List<String>, name: String, mime: String): Output = when (place) {
        "tree" -> createInTree(Uri.parse(tree ?: error("No folder was picked")), path, name, mime)
        "downloads", "pictures" ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) createInMediaStore(place, path, name, mime)
            else createInPublicFolder(place, path, name)
        else -> error("Unknown place $place")
    }

    private fun createInMediaStore(place: String, path: List<String>, name: String, mime: String): Output {
        val pictures = place == "pictures" && mime.startsWith("image/")
        val collection = if (pictures) MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        else MediaStore.Downloads.EXTERNAL_CONTENT_URI
        val top = if (pictures) Environment.DIRECTORY_PICTURES else Environment.DIRECTORY_DOWNLOADS
        val relative = (listOf(top, "Raygun") + path).joinToString("/") + "/"
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
        return Output(uri, open(uri))
    }

    /** Android 9 and older: plain folders under shared storage. */
    private fun createInPublicFolder(place: String, path: List<String>, name: String): Output {
        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
            PackageManager.PERMISSION_GRANTED
        if (!granted) error("Allow Raygun to use storage in the system settings to save files")
        @Suppress("DEPRECATION")
        val top = Environment.getExternalStoragePublicDirectory(
            if (place == "pictures") Environment.DIRECTORY_PICTURES else Environment.DIRECTORY_DOWNLOADS,
        )
        val dir = File(top, (listOf("Raygun") + path).joinToString(File.separator))
        if (!dir.isDirectory && !dir.mkdirs()) error("Couldn't make ${dir.path}")
        val file = File(dir, name)
        val descriptor = ParcelFileDescriptor.open(
            file,
            ParcelFileDescriptor.MODE_WRITE_ONLY or ParcelFileDescriptor.MODE_CREATE or ParcelFileDescriptor.MODE_TRUNCATE,
        )
        return Output(Uri.fromFile(file), descriptor.detachFd())
    }

    private fun createInTree(tree: Uri, path: List<String>, name: String, mime: String): Output {
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
        val uri = child(tree, parent, name, directory = false)
            ?: DocumentsContract.createDocument(resolver, parent, mime, name)
            ?: error("Couldn't make $name")
        return Output(uri, open(uri))
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

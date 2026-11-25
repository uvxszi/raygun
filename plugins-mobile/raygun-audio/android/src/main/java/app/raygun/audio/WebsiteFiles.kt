package app.raygun.audio

import android.content.Context
import android.net.Uri
import android.util.Base64
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLDecoder

/**
 * Files only the page can read, for the browser's downloads: `blob:` addresses (read in the page,
 * through [FILES_NAME]) and `data:` ones (decoded here), written to [filesFolder] in the app's
 * cache, from where Raygun's process takes them (`localFile` in the request; its
 * `website_downloads::fetch::LOCAL_FILES`). Requests between the processes stay small: a broadcast
 * can't carry a whole file.
 */

/** The page's channel for the bytes of a `blob:` download. */
internal const val FILES_NAME = "raygunFiles"

/** A file read from a page, at most. */
internal const val MAX_PAGE_FILE = 512L * 1024 * 1024

/** A piece of a file in one message, at most (base64). */
private const val MAX_PIECE = 1024 * 1024

/** Where the browser leaves files for Raygun's process (the same name on its side). */
internal fun filesFolder(context: Context): File = File(context.cacheDir, "web-files")

/**
 * Runs in every frame before the page: keeps the blob behind each `blob:` address a while after
 * the page lets it go (pages often revoke it right after the click that downloads it), and the
 * name a link's `download` attribute gives.
 */
internal const val FILES_HOOK = """(() => {
  if (window.__raygunFile) return;
  const kept = new Map();
  const names = new Map();
  const create = URL.createObjectURL;
  const revoke = URL.revokeObjectURL;
  URL.createObjectURL = function (object) {
    const url = create.apply(this, arguments);
    try { if (object instanceof Blob) kept.set(url, object); } catch (error) {}
    return url;
  };
  URL.revokeObjectURL = function (url) {
    const self = this, args = arguments;
    setTimeout(() => { kept.delete(url); names.delete(url); revoke.apply(self, args); }, 60000);
  };
  const note = (link) => {
    try {
      if (link && link.hasAttribute('download') && link.href) names.set(link.href, link.getAttribute('download') || '');
    } catch (error) {}
  };
  const click = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () { note(this); return click.apply(this, arguments); };
  document.addEventListener('click', (event) => {
    const target = event.target;
    note(target && target.closest ? target.closest('a[download]') : null);
  }, true);
  Object.defineProperty(window, '__raygunFile', {
    value: (url) => ({ blob: kept.get(url) || null, name: names.get(url) || '' }),
  });
})();"""

/** Reads `url` (a `blob:` address) in the page and sends its bytes back under `token`. */
internal fun blobReadScript(token: String, url: String): String = """(() => {
  const token = ${JSONObject.quote(token)}, url = ${JSONObject.quote(url)};
  const port = window.$FILES_NAME;
  if (!port) return 'none';
  const send = (message) => port.postMessage(JSON.stringify(Object.assign({ token }, message)));
  const found = window.__raygunFile ? window.__raygunFile(url) : { blob: null, name: '' };
  (found.blob ? Promise.resolve(found.blob) : fetch(url).then((answer) => answer.blob())).then(async (blob) => {
    send({ kind: 'start', size: blob.size, mime: blob.type || '', name: found.name || '' });
    const piece = 393216;
    for (let at = 0; at < blob.size; at += piece) {
      const bytes = new Uint8Array(await blob.slice(at, at + piece).arrayBuffer());
      let text = '';
      for (let i = 0; i < bytes.length; i += 32768) text += String.fromCharCode.apply(null, bytes.subarray(i, i + 32768));
      send({ kind: 'part', data: btoa(text) });
    }
    send({ kind: 'end' });
  }).catch((error) => send({ kind: 'error', error: String((error && error.message) || error) }));
  return 'reading';
})()"""

/** The name a link's `download` attribute gave `url`, when the page noted one. */
internal fun downloadNameScript(url: String): String =
    "(() => { try { return (window.__raygunFile && window.__raygunFile(${JSONObject.quote(url)}).name) || ''; } catch (e) { return ''; } })()"

/**
 * The file name a `Content-Disposition` header gives: `filename*` (RFC 5987, any charset read as
 * UTF-8) first, then `filename`. Paths are cut to their last part.
 */
internal fun dispositionName(disposition: String?): String? {
    if (disposition.isNullOrBlank()) return null
    val parts = splitParams(disposition)
    val extended = parts.firstNotNullOfOrNull { (key, value) ->
        if (!key.equals("filename*", ignoreCase = true)) return@firstNotNullOfOrNull null
        val encoded = value.substringAfter("''", value)
        runCatching { URLDecoder.decode(encoded.replace("+", "%2B"), "UTF-8") }.getOrNull()
    }
    val plain = parts.firstNotNullOfOrNull { (key, value) ->
        if (key.equals("filename", ignoreCase = true)) value else null
    }
    return (extended ?: plain)
        ?.substringAfterLast('/')
        ?.substringAfterLast('\\')
        ?.trim()
        ?.takeIf { it.isNotEmpty() && it != "." && it != ".." }
}

/** `attachment; filename="a;b.txt"; x=y` → (filename, a;b.txt), (x, y). */
private fun splitParams(header: String): List<Pair<String, String>> {
    val found = mutableListOf<Pair<String, String>>()
    var index = header.indexOf(';')
    while (index in header.indices) {
        val equals = header.indexOf('=', index + 1)
        if (equals < 0) break
        val key = header.substring(index + 1, equals).trim()
        var at = equals + 1
        while (at < header.length && header[at] == ' ') at++
        val value: String
        if (at < header.length && header[at] == '"') {
            val text = StringBuilder()
            at++
            while (at < header.length && header[at] != '"') {
                if (header[at] == '\\' && at + 1 < header.length) at++
                text.append(header[at])
                at++
            }
            value = text.toString()
            index = header.indexOf(';', at)
        } else {
            val end = header.indexOf(';', at).let { if (it < 0) header.length else it }
            value = header.substring(at, end).trim()
            index = if (end < header.length) end else -1
        }
        found += key to value
    }
    return found
}

/** A `data:` address's bytes and type; null when it isn't one or can't be read. */
internal fun decodeDataUrl(
    url: String,
    decode64: (String) -> ByteArray? = { runCatching { Base64.decode(it, Base64.DEFAULT) }.getOrNull() },
): Pair<ByteArray, String?>? {
    if (!url.startsWith("data:", ignoreCase = true)) return null
    val comma = url.indexOf(',')
    if (comma < 0) return null
    val meta = url.substring(5, comma)
    val base64 = meta.endsWith(";base64", ignoreCase = true)
    val mime = meta.removeSuffix(";base64").substringBefore(';').trim().takeIf { it.isNotEmpty() }
    val data = url.substring(comma + 1)
    val bytes = if (base64) {
        decode64(percentDecode(data).filterNot { it.isWhitespace() })
    } else {
        percentDecodeBytes(data)
    } ?: return null
    return bytes to mime
}

private fun percentDecode(text: String): String = String(percentDecodeBytes(text), Charsets.ISO_8859_1)

private fun percentDecodeBytes(text: String): ByteArray {
    val out = java.io.ByteArrayOutputStream(text.length)
    var index = 0
    while (index < text.length) {
        val char = text[index]
        if (char == '%' && index + 2 < text.length) {
            val value = text.substring(index + 1, index + 3).toIntOrNull(16)
            if (value != null) {
                out.write(value)
                index += 3
                continue
            }
        }
        out.write(char.code and 0xFF)
        index++
    }
    return out.toByteArray()
}

/** What a page sent of a file, read into [file]. */
internal class PageFile(val token: String, val file: File, val onDone: (PageFile?, String?) -> Unit) {
    var size = -1L
    var mime = ""
    var name = ""
    var written = 0L
    var output: FileOutputStream? = null
}

/**
 * Takes `blob:` files from pages ([blobReadScript] → [FILES_NAME]): only reads the browser asked
 * for (a token of its own) are written, each to its file, nothing past its size.
 */
internal class PageFiles(private val context: Context) : WebViewCompat.WebMessageListener {
    private val reading = mutableMapOf<String, PageFile>()

    /** Starts reading `url` from `view`'s page into the cache; `done` gets the file, or why not. */
    fun read(view: android.webkit.WebView, url: String, done: (PageFile?, String?) -> Unit) {
        val token = java.util.UUID.randomUUID().toString()
        val folder = filesFolder(context).apply { mkdirs() }
        val read = PageFile(token, File(folder, token), done)
        reading[token] = read
        view.evaluateJavascript(blobReadScript(token, url)) { answer ->
            if (answer?.contains("reading") != true) finish(read, "This download can't be saved from here")
        }
    }

    override fun onPostMessage(
        view: android.webkit.WebView,
        message: WebMessageCompat,
        sourceOrigin: Uri,
        isMainFrame: Boolean,
        replyProxy: JavaScriptReplyProxy,
    ) {
        if (message.type != WebMessageCompat.TYPE_STRING) return
        val data = runCatching { message.data }.getOrNull() ?: return
        if (data.length > MAX_PIECE + 1024) return
        val json = runCatching { JSONObject(data) }.getOrNull() ?: return
        val read = reading[json.optString("token")] ?: return
        when (json.optString("kind")) {
            "start" -> {
                read.size = json.optLong("size", -1)
                read.mime = json.optString("mime")
                read.name = json.optString("name")
                if (read.size > MAX_PAGE_FILE) return finish(read, "The file is too large to save from the page")
                read.output = runCatching { FileOutputStream(read.file) }.getOrNull()
                    ?: return finish(read, "The file couldn't be written")
            }
            "part" -> {
                val output = read.output ?: return
                val bytes = runCatching { Base64.decode(json.optString("data"), Base64.DEFAULT) }.getOrNull()
                    ?: return finish(read, "The file is unreadable")
                if (read.written + bytes.size > read.size) return finish(read, "The file is unreadable")
                runCatching { output.write(bytes) }.onFailure { return finish(read, "The file couldn't be written") }
                read.written += bytes.size
            }
            "end" -> {
                if (read.written != read.size) return finish(read, "The file is unreadable")
                finish(read, null)
            }
            "error" -> finish(read, "The page didn't give the file")
        }
    }

    private fun finish(read: PageFile, error: String?) {
        if (reading.remove(read.token) == null) return
        runCatching { read.output?.close() }
        if (error != null) read.file.delete()
        read.onDone(if (error == null) read else null, error)
    }
}

/** Writes a `data:` address's bytes to the cache; the file and type, or null. */
internal fun writeDataUrl(context: Context, url: String): Pair<File, String?>? {
    val (bytes, mime) = decodeDataUrl(url) ?: return null
    val folder = filesFolder(context).apply { mkdirs() }
    val file = File(folder, java.util.UUID.randomUUID().toString())
    return runCatching { file.writeBytes(bytes) }.map { file to mime }.getOrNull()
}

/**
 * Fetches a picture to share into the cache, as the page would (its cookies, User-Agent, the page
 * as Referer). Blocking; null when it fails.
 */
internal fun fetchToCache(context: Context, url: String, userAgent: String?, referer: String?, cookies: String?): Pair<File, String?>? {
    if (url.startsWith("data:", ignoreCase = true)) return writeDataUrl(context, url)
    val scheme = Uri.parse(url).scheme?.lowercase()
    if (scheme != "http" && scheme != "https") return null
    return runCatching {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = 15_000
        connection.readTimeout = 30_000
        userAgent?.let { connection.setRequestProperty("User-Agent", it) }
        referer?.let { connection.setRequestProperty("Referer", it) }
        cookies?.let { connection.setRequestProperty("Cookie", it) }
        try {
            if (connection.responseCode !in 200..299) return null
            val folder = File(context.cacheDir, "web-shared").apply { mkdirs() }
            folder.listFiles()?.forEach { it.delete() }
            val type = connection.contentType?.substringBefore(';')?.trim()
            val name = Uri.parse(url).lastPathSegment?.replace(Regex("[^A-Za-z0-9._-]"), "_")?.takeIf { it.isNotBlank() } ?: "image"
            val file = File(folder, name)
            connection.inputStream.use { input -> file.outputStream().use { input.copyTo(it) } }
            file to type
        } finally {
            connection.disconnect()
        }
    }.getOrNull()
}

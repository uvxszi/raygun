// Adapted from Mihon (https://github.com/mihonapp/mihon), Apache-2.0.
package eu.kanade.tachiyomi.network

interface ProgressListener {
    fun update(bytesRead: Long, contentLength: Long, done: Boolean)
}

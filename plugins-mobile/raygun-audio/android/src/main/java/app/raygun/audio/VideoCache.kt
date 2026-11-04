package app.raygun.audio

import android.content.Context
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import java.io.File

/**
 * What the video player has streamed, kept on disk (Media3's `SimpleCache`): a looping video, or
 * a seek back, reads what was already fetched instead of downloading it again. Files and segments
 * are written once as they stream and read back after; the least recently used go first.
 */
@OptIn(UnstableApi::class)
internal object VideoCache {
    private const val MAX_BYTES = 512L * 1024 * 1024

    /** One cache per folder and process, as `SimpleCache` requires. */
    @Volatile
    private var cache: SimpleCache? = null

    private fun cache(context: Context): SimpleCache =
        cache ?: synchronized(this) {
            cache ?: SimpleCache(
                File(context.applicationContext.cacheDir, "video-cache"),
                LeastRecentlyUsedCacheEvictor(MAX_BYTES),
                StandaloneDatabaseProvider(context.applicationContext),
            ).also { cache = it }
        }

    /**
     * Reads through the cache for streams from the network. Files on the device and Raygun's own
     * proxy (finished downloads) are already local, so they skip it.
     */
    fun wrap(context: Context, uri: Uri, upstream: DataSource.Factory): DataSource.Factory {
        val scheme = uri.scheme?.lowercase()
        val host = uri.host?.lowercase()
        val remote = (scheme == "http" || scheme == "https") &&
            host != "localhost" && host != "127.0.0.1" && host != "[::1]" && host != "::1"
        if (!remote) return upstream
        return runCatching {
            CacheDataSource.Factory()
                .setCache(cache(context))
                .setUpstreamDataSourceFactory(upstream)
                // A cache that can't be read or written never stops the video.
                .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)
        }.getOrDefault(upstream)
    }
}

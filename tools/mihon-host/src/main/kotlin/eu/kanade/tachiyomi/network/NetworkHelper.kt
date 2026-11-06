// Adapted from Mihon (https://github.com/mihonapp/mihon, core/common), Apache-2.0: the client
// extensions get, for Raygun's desktop host. The User-Agent is Raygun's, addresses resolve only
// to public hosts, and cookies come from Raygun's jar for the extension.
package eu.kanade.tachiyomi.network

import eu.kanade.tachiyomi.network.interceptor.UncaughtExceptionInterceptor
import eu.kanade.tachiyomi.network.interceptor.UserAgentInterceptor
import okhttp3.Cache
import okhttp3.Dns
import okhttp3.OkHttpClient
import okhttp3.brotli.BrotliInterceptor
import java.io.File
import java.net.Inet4Address
import java.net.InetAddress
import java.net.UnknownHostException
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds

class NetworkHelper(
    cacheDir: File,
    private val userAgent: String,
) {

    val cookieJar = AndroidCookieJar()

    private val builder: OkHttpClient.Builder = OkHttpClient.Builder()
        .cookieJar(cookieJar)
        .dns(PublicDns)
        .connectTimeout(30.seconds)
        .readTimeout(30.seconds)
        .callTimeout(2.minutes)
        .cache(Cache(directory = File(cacheDir, "network_cache"), maxSize = 5L * 1024 * 1024))
        .addInterceptor(UncaughtExceptionInterceptor())
        .addInterceptor(UserAgentInterceptor(::defaultUserAgentProvider))
        .addInterceptor(BrotliInterceptor)

    val client: OkHttpClient by lazy { builder.build() }

    /**
     * @deprecated Since extension-lib 1.5
     */
    @Deprecated("The regular client handles Cloudflare by default")
    @Suppress("UNUSED")
    val cloudflareClient: OkHttpClient get() = client

    fun defaultUserAgentProvider() = userAgent
}

/** DNS that gives only public addresses: an extension can't reach this computer or its network. */
object PublicDns : Dns {
    override fun lookup(hostname: String): List<InetAddress> {
        val found = Dns.SYSTEM.lookup(hostname).filter { it.isPublic() }
        if (found.isEmpty()) throw UnknownHostException("$hostname isn't a public address")
        return found
    }

    private fun InetAddress.isPublic(): Boolean {
        if (isLoopbackAddress || isSiteLocalAddress || isLinkLocalAddress || isAnyLocalAddress || isMulticastAddress) {
            return false
        }
        val bytes = address
        // Carrier-grade NAT (100.64/10) and unique-local IPv6 (fc00::/7).
        if (this is Inet4Address && bytes[0].toInt() and 0xFF == 100 && bytes[1].toInt() and 0xC0 == 64) return false
        if (bytes.size == 16 && bytes[0].toInt() and 0xFE == 0xFC) return false
        return true
    }
}

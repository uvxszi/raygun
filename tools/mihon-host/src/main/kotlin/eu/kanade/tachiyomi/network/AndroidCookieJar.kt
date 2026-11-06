// Adapted from Mihon (https://github.com/mihonapp/mihon, core/common), Apache-2.0: the same API
// over a store in memory, seeded with Raygun's cookies for the extension.
package eu.kanade.tachiyomi.network

import okhttp3.Cookie
import okhttp3.CookieJar
import okhttp3.HttpUrl

class AndroidCookieJar : CookieJar {

    private val cookies = mutableListOf<Cookie>()

    @Synchronized
    override fun saveFromResponse(url: HttpUrl, cookies: List<Cookie>) {
        for (cookie in cookies) {
            this.cookies.removeAll { it.name == cookie.name && it.domain == cookie.domain && it.path == cookie.path }
            this.cookies += cookie
        }
    }

    @Synchronized
    override fun loadForRequest(url: HttpUrl): List<Cookie> {
        val now = System.currentTimeMillis()
        cookies.removeAll { it.expiresAt < now }
        return cookies.filter { it.matches(url) }
    }

    fun get(url: HttpUrl): List<Cookie> = loadForRequest(url)

    @Synchronized
    fun remove(url: HttpUrl, cookieNames: List<String>? = null, maxAge: Int = -1): Int {
        val before = cookies.size
        cookies.removeAll { it.matches(url) && (cookieNames == null || it.name in cookieNames) }
        return before - cookies.size
    }

    @Synchronized
    fun removeAll() {
        cookies.clear()
    }

    /** Every cookie kept, for Raygun to save. */
    @Synchronized
    fun all(): List<Cookie> = cookies.toList()
}

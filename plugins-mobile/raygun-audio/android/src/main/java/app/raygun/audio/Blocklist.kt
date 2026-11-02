package app.raygun.audio

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The built-in browser's ad and tracker domains (M46.3): sorted 64-bit FNV-1a hashes of domains,
 * written by Rust (`raygun-app/src/websites/blocklist.rs`, the same hash) to `web/blocklist.bin`.
 * A host is blocked when it or one of its parent domains is listed.
 */
internal class Blocklist private constructor(private val hashes: LongArray) {
    val size: Int get() = hashes.size

    fun blocks(host: String): Boolean {
        var domain = host.lowercase().trimEnd('.')
        while (domain.contains('.')) {
            if (hashes.binarySearch(fnv(domain)) >= 0) return true
            domain = domain.substringAfter('.')
        }
        return false
    }

    companion object {
        private val MAGIC = byteArrayOf('R'.code.toByte(), 'G'.code.toByte(), 'B'.code.toByte(), 'L'.code.toByte())

        /** The biggest ad, pop-under and tracker networks: used until a list is downloaded, then with it. */
        private val BUILT_IN = listOf(
            "doubleclick.net", "googlesyndication.com", "googleadservices.com", "adservice.google.com",
            "google-analytics.com", "googletagmanager.com", "amazon-adsystem.com", "adnxs.com",
            "taboola.com", "outbrain.com", "criteo.com", "criteo.net", "pubmatic.com",
            "rubiconproject.com", "openx.net", "scorecardresearch.com", "quantserve.com",
            "moatads.com", "adform.net", "mgid.com", "revcontent.com", "popads.net", "popcash.net",
            "propellerads.com", "propellerclick.com", "onclickads.net", "onclkds.com", "exoclick.com",
            "exosrv.com", "juicyads.com", "trafficjunky.net", "adsterra.com", "adsterratech.com",
            "hilltopads.net", "a-ads.com", "clickadu.com", "adcash.com", "monetag.com", "admaven.com",
            "ad-maven.com", "galaksion.com", "bidvertiser.com", "zedo.com", "yieldmo.com",
            "highperformanceformat.com", "profitablecpmrate.com", "pushame.com", "adskeeper.com",
        )

        /** FNV-1a, 64 bits, over the UTF-8 bytes; the same as Rust's `blocklist::hash`. */
        fun fnv(text: String): Long {
            var hash = 0xcbf29ce484222325uL.toLong()
            for (byte in text.toByteArray(Charsets.UTF_8)) {
                hash = hash xor (byte.toLong() and 0xff)
                hash *= 0x100000001b3L
            }
            return hash
        }

        fun builtIn(): Blocklist = Blocklist(BUILT_IN.map(::fnv).sorted().toLongArray())

        /** The downloaded list, or the built-in one when there's none (or it's unreadable). */
        fun load(path: String?): Blocklist {
            val bytes = path?.let { runCatching { File(it).readBytes() }.getOrNull() }
            if (bytes == null || bytes.size < 12 || !bytes.copyOfRange(0, 4).contentEquals(MAGIC)) {
                return builtIn()
            }
            val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
            buffer.position(4)
            if (buffer.int != 1) return builtIn()
            val count = buffer.int
            if (count < 0 || buffer.remaining() < count.toLong() * 8) return builtIn()
            val hashes = LongArray(count)
            buffer.asLongBuffer().get(hashes)
            // The built-in pop-under and ad networks stay blocked whatever the list leaves out.
            // (Duplicates don't matter to a binary search.)
            val merged = hashes.copyOf(count + BUILT_IN.size)
            BUILT_IN.forEachIndexed { index, domain -> merged[count + index] = fnv(domain) }
            merged.sort()
            return Blocklist(merged)
        }

        private val SECOND_LEVEL = setOf("co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go")

        /** The site a host belongs to: `www.example.co.uk` → `example.co.uk`. */
        fun site(host: String?): String {
            val labels = (host ?: return "").lowercase().trimEnd('.').split('.')
            if (labels.size <= 2) return labels.joinToString(".")
            val keep = if (labels.last().length == 2 && labels[labels.size - 2] in SECOND_LEVEL) 3 else 2
            return labels.takeLast(keep).joinToString(".")
        }
    }
}

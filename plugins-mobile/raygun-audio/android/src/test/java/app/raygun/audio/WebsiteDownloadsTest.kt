package app.raygun.audio

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WebsiteDownloadsTest {
    private val size = { bytes: Double -> "${bytes.toLong()} B" }

    @Test
    fun namesComeFromTheServersHeader() {
        assertEquals("Report été.bin", dispositionName("attachment; filename*=UTF-8''Report%20%C3%A9t%C3%A9.bin"))
        assertEquals("a;b.txt", dispositionName("attachment; filename=\"a;b.txt\""))
        assertEquals("plain.txt", dispositionName("attachment; filename=plain.txt; size=3"))
        // The extended name wins; paths are cut to their last part.
        assertEquals("new.pdf", dispositionName("attachment; filename=\"old.pdf\"; filename*=utf-8''new.pdf"))
        assertEquals("evil.sh", dispositionName("attachment; filename=\"../../evil.sh\""))
        assertEquals("1+1.txt", dispositionName("attachment; filename*=UTF-8''1+1.txt"))
        assertNull(dispositionName("inline"))
        assertNull(dispositionName(null))
    }

    @Test
    fun dataAddressesAreDecoded() {
        val decode = { text: String -> runCatching { java.util.Base64.getMimeDecoder().decode(text) }.getOrNull() }
        val (bytes, mime) = decodeDataUrl("data:text/plain;base64,SGVsbG8gZGF0YQ==", decode)!!
        assertEquals("Hello data", String(bytes))
        assertEquals("text/plain", mime)
        val (plain, none) = decodeDataUrl("data:,a%20b", decode)!!
        assertArrayEquals("a b".toByteArray(), plain)
        assertNull(none)
        assertNull(decodeDataUrl("https://example.org/", decode))
        assertNull(decodeDataUrl("data:text/plain", decode))
    }

    @Test
    fun aLongPressOffersWhatBrowsersDo() {
        assertEquals(
            listOf(
                ContextItem.OPEN_TAB,
                ContextItem.OPEN_BACKGROUND,
                ContextItem.COPY_LINK,
                ContextItem.DOWNLOAD_LINK,
                ContextItem.SHARE_LINK,
            ),
            contextItems("https://example.org/a", null),
        )
        assertEquals(
            listOf(ContextItem.OPEN_IMAGE, ContextItem.DOWNLOAD_IMAGE, ContextItem.COPY_IMAGE, ContextItem.SHARE_IMAGE),
            contextItems(null, "https://example.org/a.png"),
        )
        // A picture only the page holds: download and share, no address to open or copy.
        assertEquals(
            listOf(ContextItem.DOWNLOAD_IMAGE, ContextItem.SHARE_IMAGE),
            contextItems(null, "data:image/png;base64,AAAA"),
        )
        assertEquals(9, contextItems("https://example.org/a", "https://example.org/a.png").size)
        assertEquals(emptyList<ContextItem>(), contextItems(null, null))
    }

    @Test
    fun picturesTypesAreGuessed() {
        assertEquals("image/png", pictureType("https://example.org/a.PNG?x=1"))
        assertEquals("image/jpeg", pictureType("https://example.org/a.jpeg#top"))
        assertEquals("image/webp", pictureType("data:image/webp;base64,AAAA"))
        assertNull(pictureType("https://example.org/picture"))
    }

    @Test
    fun rowsSayHowTheyGo() {
        val row = DownloadRow("t").apply { folder = "Download/Raygun" }
        assertEquals("Starting · to Download/Raygun", DownloadWords.status(row, size))
        row.waiting = "Starts when one of the 3 downloads running ends"
        assertEquals("Starts when one of the 3 downloads running ends", DownloadWords.status(row, size))
        row.unheard = true
        assertEquals("Waiting for Raygun to start", DownloadWords.status(row, size))
        row.unheard = false
        row.waiting = null
        row.state = "downloading"
        row.done = 250.0
        row.total = 1000.0
        assertEquals("25% · 250 B of 1000 B", DownloadWords.status(row, size))
        row.state = "done"
        assertEquals("Saved in Download/Raygun · 1000 B", DownloadWords.status(row, size))
        row.state = "failed"
        row.error = "The file isn't there any more (HTTP 404)"
        assertEquals("The file isn't there any more (HTTP 404)", DownloadWords.status(row, size))
    }

    @Test
    fun theButtonsRingFollowsTheRunningOnes() {
        val half = DownloadRow("a").apply {
            state = "downloading"
            done = 50.0
            total = 100.0
        }
        val finished = DownloadRow("b").apply { state = "done" }
        assertEquals(0.5f, DownloadWords.overall(listOf(half, finished))!!, 0.001f)
        val unsized = DownloadRow("c").apply { state = "queued" }
        assertEquals(-1f, DownloadWords.overall(listOf(half, unsized))!!, 0.001f)
        assertNull(DownloadWords.overall(listOf(finished)))
    }
}

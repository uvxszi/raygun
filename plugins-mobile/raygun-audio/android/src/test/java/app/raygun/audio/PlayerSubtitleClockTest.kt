package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerSubtitleClockTest {
    private val lines = listOf(
        TimedCues(1_000_000, 3_000_000, emptyList()),
        TimedCues(5_000_000, 6_000_000, emptyList()),
        TimedCues(5_500_000, 8_000_000, emptyList()),
    )

    @Test
    fun theDelayMovesTheClockEitherWayAndPastTheEnds() {
        assertEquals(8_000L, subtitleTimeMs(10_000, 2_000))
        assertEquals(12_500L, subtitleTimeMs(10_000, -2_500))
        // Longer than the video: before its start, nothing shows.
        assertEquals(-590_000L, subtitleTimeMs(10_000, 600_000))
        assertTrue(timedAt(lines, -590_000_000).isEmpty())
        // Earlier by more than is left: past its end, nothing shows either.
        assertTrue(timedAt(lines, subtitleTimeMs(10_000, -600_000) * 1000).isEmpty())
    }

    @Test
    fun linesShowFromTheirStartUntilTheirEnd() {
        assertTrue(timedAt(lines, 999_999).isEmpty())
        assertEquals(listOf(lines[0]), timedAt(lines, 1_000_000))
        assertTrue(timedAt(lines, 3_000_000).isEmpty())
        assertEquals(listOf(lines[1], lines[2]), timedAt(lines, 5_600_000))
        // Two seconds of delay: the line at 1 s shows at 3 s of the video.
        assertEquals(listOf(lines[0]), timedAt(lines, subtitleTimeMs(3_000, 2_000) * 1000))
    }

    @Test
    fun theNextChangeIsTheNearestStartOrEnd() {
        assertEquals(1_000_000L, nextEdgeUs(lines, 0))
        assertEquals(3_000_000L, nextEdgeUs(lines, 1_000_000))
        assertEquals(5_500_000L, nextEdgeUs(lines, 5_000_000))
        assertNull(nextEdgeUs(lines, 8_000_000))
        // A line without an end lasts until the next one; its end is never waited for.
        assertNull(nextEdgeUs(listOf(TimedCues(0, Long.MAX_VALUE, emptyList())), 10))
    }

    @Test
    fun theWaitFollowsThePlayingSpeed() {
        // 100 ms of media away: 100 ms at 1×, 50 ms at 2×, 200 ms at 0.5×.
        assertEquals(100L, waitMs(1_100_000, 1_000_000, 1f))
        assertEquals(50L, waitMs(1_100_000, 1_000_000, 2f))
        assertEquals(200L, waitMs(1_100_000, 1_000_000, 0.5f))
        // Never longer than a look every quarter second, never a busy loop.
        assertEquals(CLOCK_MAX_WAIT_MS, waitMs(9_000_000, 1_000_000, 1f))
        assertEquals(CLOCK_MAX_WAIT_MS, waitMs(null, 1_000_000, 1f))
        assertEquals(CLOCK_MIN_WAIT_MS, waitMs(1_000_001, 1_000_000, 1f))
    }

    @Test
    fun smartSubtitlesOrderEachLanguageAndKeepLanguagesInPlace() {
        // (name, language, rank)
        val tracks = listOf(
            Triple("en-a", "en", 2),
            Triple("en-b", "en", 0),
            Triple("de", "de", null),
            Triple("en-c", "en", null),
            Triple("en-d", "en", 1),
        )
        val ordered = smartOrder(tracks, { it.second }, { it.third }).map { it.first }
        assertEquals(listOf("en-b", "en-d", "en-a", "en-c", "de"), ordered)
        // Nothing ranked: as it was.
        val plain = tracks.map { it.copy(third = null) }
        assertEquals(plain, smartOrder(plain, { it.second }, { it.third }))
    }

    @Test
    fun leftOutLinksSayWhyAndHowLong() {
        val label = { _: String, fallback: String -> fallback }
        assertEquals("Trailer or sample · 2:05", leftOutText("trailer", 125_000, label))
        assertEquals("Shorter than the title", leftOutText("short", null, label))
        assertNull(leftOutText(null, null, label))
    }
}

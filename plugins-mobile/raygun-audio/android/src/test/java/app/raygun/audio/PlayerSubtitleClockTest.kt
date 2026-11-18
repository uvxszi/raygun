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
    fun aDelayLongerThanTheVideoOrBeforeItsStartBehaves() {
        // Ten minutes later on a nine-second video: nothing ever shows, and the clock looks
        // again at most every quarter second (no busy loop waiting for a line ten minutes off).
        val atUs = subtitleTimeMs(9_000, 600_000) * 1000
        assertTrue(timedAt(lines, atUs).isEmpty())
        assertEquals(1_000_000L, nextEdgeUs(lines, atUs))
        assertEquals(CLOCK_MAX_WAIT_MS, waitMs(nextEdgeUs(lines, atUs), atUs, 1f))
        // Five seconds earlier, at the very start: the line at 5 s shows at 0.
        assertEquals(listOf(lines[1]), timedAt(lines, subtitleTimeMs(0, -5_000) * 1000))
        // Earlier than a line's start before the video starts: the clock at -1 s shows nothing.
        assertTrue(timedAt(lines, subtitleTimeMs(0, 1_000) * 1000).isEmpty())
        assertEquals(1_000_000L, nextEdgeUs(lines, -1_000_000))
    }

    @Test
    fun theDelayIsInTheVideosTimeAtAnySpeed() {
        // Two seconds of delay: the line at 1 s shows at 3 s of the video, whatever the speed;
        // from 2.9 s it's 100 ms of video away: 50 ms at 2×, 200 ms at 0.5×.
        val atUs = subtitleTimeMs(2_900, 2_000) * 1000
        assertTrue(timedAt(lines, atUs).isEmpty())
        assertEquals(50L, waitMs(nextEdgeUs(lines, atUs), atUs, 2f))
        assertEquals(200L, waitMs(nextEdgeUs(lines, atUs), atUs, 0.5f))
        assertEquals(listOf(lines[0]), timedAt(lines, subtitleTimeMs(3_000, 2_000) * 1000))
        // A negative delay at 2×: the line at 5 s shows at 4.5 s of the video.
        val earlyUs = subtitleTimeMs(4_400, -500) * 1000
        assertEquals(50L, waitMs(nextEdgeUs(lines, earlyUs), earlyUs, 2f))
        assertEquals(listOf(lines[1]), timedAt(lines, subtitleTimeMs(4_500, -500) * 1000))
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

    /**
     * A player as the clock sees it: its position moves at its speed while it plays (not while
     * paused or stalled), and jumps on seeks.
     */
    private class FakePlayer {
        var position = 0.0
        var speed = 1f
        var paused = false
        var stalled = false
        val playing get() = !paused && !stalled
        val positionMs get() = position.toLong()
    }

    @Test
    fun aLongPlayWithStallsSeeksSpeedAndDelayChangesKeepsEveryLineOnTime() {
        // Three hundred lines of varied lengths and gaps, some overlapping.
        val timed = (0 until 300).map { index ->
            val start = 2_000L + index * 2_317L + (index % 7) * 113L
            val length = 600L + (index % 5) * 731L
            TimedCues(start * 1000, (start + length) * 1000, emptyList())
        }
        val player = FakePlayer()
        var delay = 0L
        val core = ClockCore()
        var nextLook: Long? = null
        // What the activity does on the player's events: look at once, then as told.
        fun look(now: Long) {
            val (_, wait) = core.look(timed, player.positionMs, delay, player.playing, player.speed)
            nextLook = wait?.let { now + it }
        }
        // (wall ms, change): stalls, seeks, speeds and delays, as a long evening goes.
        val events = listOf<Pair<Long, () -> Unit>>(
            60_000L to { player.stalled = true },
            63_000L to { player.stalled = false },
            120_000L to { player.position = 300_000.0 },
            180_000L to { player.speed = 1.5f },
            240_000L to { delay = 1_500 },
            300_000L to { player.speed = 0.75f },
            360_000L to { player.stalled = true },
            365_000L to { player.stalled = false },
            400_000L to { player.paused = true },
            401_234L to { player.paused = false },
            420_000L to { player.position = 50_000.0 },
            480_000L to { delay = -800; player.speed = 2f },
            530_000L to { player.stalled = true },
            530_400L to { player.stalled = false },
        ).toMap()
        look(0)
        var late = 0
        var worst = 0
        for (now in 1L..600_000L) {
            if (player.playing) player.position += player.speed
            events[now]?.let { change ->
                change()
                look(now)
            }
            if (nextLook?.let { it <= now } == true) look(now)
            val expected = timedAt(timed, subtitleTimeMs(player.positionMs, delay) * 1000)
            if (core.shown != expected) {
                late += 1
                worst = maxOf(worst, late)
            } else {
                late = 0
            }
        }
        // Every change shows on the millisecond it's due: no line is ever late, however long
        // the play and whatever happened on the way.
        assertEquals(0, worst)
    }

    @Test
    fun theClockLooksAgainOnlyWhilePlaying() {
        val core = ClockCore()
        val (changed, wait) = core.look(lines, 1_500, 0, playing = false, speed = 1f)
        assertEquals(listOf(lines[0]), changed)
        assertNull(wait)
        // The same lines: nothing to draw again; playing, it looks again at their end.
        val (again, next) = core.look(lines, 1_500, 0, playing = true, speed = 1f)
        assertNull(again)
        assertEquals(CLOCK_MAX_WAIT_MS, next)
        assertEquals(250L, core.look(lines, 2_750, 0, playing = true, speed = 1f).second)
        assertEquals(1L, core.look(lines, 2_999, 0, playing = true, speed = 1f).second)
    }

    private data class Line(val start: Long, val end: Long, val lines: List<String>)

    private fun join(vararg entries: Line) = joinClose(
        entries.toList(),
        { it.start },
        { it.end },
        { it.lines },
    ) { start, end, _, lines -> Line(start, end, lines) }

    private fun line(startMs: Long, endMs: Long, vararg lines: String) = Line(startMs * 1000, endMs * 1000, lines.toList())

    @Test
    fun linesCloseTogetherShowAsOne() {
        // Ordinary dialogue stays as it is.
        val plain = arrayOf(line(0, 2_000, "Hello."), line(2_000, 4_500, "Hi there."), line(5_000, 7_000, "Bye."))
        assertEquals(plain.toList(), join(*plain))
        // Overlapping lines join, each once.
        assertEquals(
            listOf(line(0, 3_500, "- Where?", "- There."), line(6_000, 8_000, "Later.")),
            join(line(0, 2_000, "- Where?"), line(1_800, 3_500, "- Where?", "- There."), line(6_000, 8_000, "Later.")),
        )
        // Quick changes with short gaps join; one half a second later doesn't.
        assertEquals(
            listOf(line(10_000, 12_000, "One", "two", "three!"), line(12_500, 13_100, "Next.")),
            join(
                line(10_000, 10_600, "One"),
                line(10_700, 11_300, "two"),
                line(11_400, 12_000, "three!"),
                line(12_500, 13_100, "Next."),
            ),
        )
        // Media3's segments of overlapping cues (each with every line showing) join the same way.
        assertEquals(
            listOf(line(0, 4_000, "A", "B")),
            join(line(0, 1_800, "A"), line(1_800, 2_000, "A", "B"), line(2_000, 4_000, "B")),
        )
        // A long sign isn't stretched over the dialogue; a picture's cue never joins.
        assertEquals(
            listOf(line(0, 20_000, "CITY HALL"), line(1_000, 3_000, "Come in.")),
            join(line(0, 20_000, "CITY HALL"), line(1_000, 3_000, "Come in.")),
        )
        assertEquals(2, join(line(0, 500), line(600, 1_000, "text")).size)
    }

    @Test
    fun joinedLinesStayShortAndSmall() {
        val quick = (0 until 12).map { line(it * 700L, it * 700L + 600, "line $it") }
        val joined = join(*quick.toTypedArray())
        assertTrue(joined.size > 2)
        joined.forEach {
            assertTrue(it.end - it.start <= MAX_JOINED_US)
            assertTrue(it.lines.size <= MAX_JOINED_LINES)
        }
        assertEquals(quick.flatMap { it.lines }, joined.flatMap { it.lines })
    }

    @Test
    fun theSubtitlesShownStayThroughAReload() {
        val tracks = listOf(
            KeptTrack(null, "0:2", "en", "English"),
            KeptTrack("https://proxy/subtitle/a", "1:raygun-file-0", "en", "Addon A"),
            KeptTrack("https://proxy/subtitle/new", "1:raygun-file-1", "en", "Addon New"),
            KeptTrack("https://proxy/subtitle/b", "1:raygun-file-2", "en", "Addon B"),
        )
        // A file keeps its place by its address, whatever its id became.
        assertEquals(3, keptTrackIndex(tracks, KeptTrack("https://proxy/subtitle/b", "1:raygun-file-1", "en", "Addon B")))
        // The video's own track by its id, else by its language and name.
        assertEquals(0, keptTrackIndex(tracks, KeptTrack(null, "0:2", "en", "English")))
        assertEquals(0, keptTrackIndex(tracks, KeptTrack(null, "0:9", "en", "English")))
        assertNull(keptTrackIndex(tracks, KeptTrack("https://proxy/subtitle/gone", null, "en", "Gone")))
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

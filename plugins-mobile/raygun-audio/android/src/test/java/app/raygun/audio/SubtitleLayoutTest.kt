package app.raygun.audio

import androidx.media3.common.text.Cue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SubtitleLayoutTest {
    private val unset = Cue.DIMEN_UNSET

    private fun assertBox(expected: CueBox, actual: CueBox?) {
        requireNotNull(actual)
        assertEquals(expected.position, actual.position, 0.001f)
        assertEquals(expected.anchor, actual.anchor)
        assertEquals(expected.size, actual.size, 0.001f)
    }

    @Test
    fun aCentredCueKeepsToTheWidestShare() {
        // SubRip: no place, as wide as its text, centred by Media3.
        assertBox(CueBox(unset, Cue.TYPE_UNSET, 0.8f), narrowedBox(unset, Cue.TYPE_UNSET, unset, LINE_MIDDLE, 0.8f))
        // A narrower box of its own stays.
        assertBox(CueBox(unset, Cue.TYPE_UNSET, 0.5f), narrowedBox(unset, Cue.TYPE_UNSET, 0.5f, LINE_MIDDLE, 0.8f))
        // Left-aligned lines without a place would move: left alone.
        assertNull(narrowedBox(unset, Cue.TYPE_UNSET, unset, LINE_START, 0.8f))
    }

    @Test
    fun aBoxTheFileSizedNarrowsAroundItsText() {
        // WebVTT's usual cue: centred at the middle, the whole width.
        assertBox(
            CueBox(0.1f, Cue.ANCHOR_TYPE_START, 0.8f),
            narrowedBox(0.5f, Cue.ANCHOR_TYPE_MIDDLE, 1f, LINE_MIDDLE, 0.8f),
        )
        // Left-aligned in a box from the left edge: it keeps starting there.
        assertBox(
            CueBox(0f, Cue.ANCHOR_TYPE_START, 0.6f),
            narrowedBox(0f, Cue.ANCHOR_TYPE_START, 1f, LINE_START, 0.6f),
        )
        // Right-aligned: it keeps ending where it did.
        assertBox(
            CueBox(0.35f, Cue.ANCHOR_TYPE_START, 0.6f),
            narrowedBox(0.95f, Cue.ANCHOR_TYPE_END, 0.9f, LINE_END, 0.6f),
        )
        // Already narrow enough.
        assertNull(narrowedBox(0.5f, Cue.ANCHOR_TYPE_MIDDLE, 0.7f, LINE_MIDDLE, 0.8f))
    }

    @Test
    fun aCuePlacedAtAPointReachesAsFarAsItMay() {
        // ASS bottom centre: centred on its point, as wide as both sides allow.
        assertBox(
            CueBox(0.5f, Cue.ANCHOR_TYPE_MIDDLE, 0.8f),
            narrowedBox(0.5f, Cue.ANCHOR_TYPE_MIDDLE, unset, LINE_MIDDLE, 0.8f),
        )
        assertBox(
            CueBox(0.3f, Cue.ANCHOR_TYPE_MIDDLE, 0.6f),
            narrowedBox(0.3f, Cue.ANCHOR_TYPE_MIDDLE, unset, LINE_MIDDLE, 0.8f),
        )
        // Bottom left: from its point rightwards.
        assertBox(
            CueBox(0.05f, Cue.ANCHOR_TYPE_START, 0.8f),
            narrowedBox(0.05f, Cue.ANCHOR_TYPE_START, unset, LINE_START, 0.8f),
        )
        // Bottom right: leftwards from its point.
        assertBox(
            CueBox(0.95f, Cue.ANCHOR_TYPE_END, 0.8f),
            narrowedBox(0.95f, Cue.ANCHOR_TYPE_END, unset, LINE_END, 0.8f),
        )
        // Text that doesn't line up with its point, or a point at the edge: left as it is.
        assertNull(narrowedBox(0.4f, Cue.ANCHOR_TYPE_START, unset, LINE_MIDDLE, 0.8f))
        assertNull(narrowedBox(0.98f, Cue.ANCHOR_TYPE_START, unset, LINE_START, 0.8f))
    }

    @Test
    fun aLineSitsWhereItsAlignmentPutsIt() {
        assertEquals(10f to 30f, lineExtent(10f, 110f, 20f, LINE_START))
        assertEquals(90f to 110f, lineExtent(10f, 110f, 20f, LINE_END))
        assertEquals(50f to 70f, lineExtent(10f, 110f, 20f, LINE_MIDDLE))
    }

    @Test
    fun linesSlideClearOfTheBarsAndBack() {
        // A picture to 1000 px, lines 80 px above its foot, the bar's top at 880 with 8 px to
        // spare: they rise 48 px.
        assertEquals(48, liftAbove(pictureBottom = 1000, chosen = 80, barTop = 880, gap = 8))
        // Already above the bar: they stay.
        assertEquals(0, liftAbove(pictureBottom = 700, chosen = 80, barTop = 880, gap = 8))
        // The second subtitles start at 40 px; the top bar ends at 150.
        assertEquals(114, dropBelow(lineTop = 40, barBottom = 150, gap = 4))
        assertEquals(0, dropBelow(lineTop = 200, barBottom = 150, gap = 4))
    }

    @Test
    fun aPickedWhiteWinsOverTheFilesColours() {
        val defaults = SubtitleLook()
        assertFalse(defaults.ownText)
        assertFalse(defaults.ownBackground)
        // White picked is white, not "as the file says".
        val white = defaults.withText(DEFAULT_TEXT_COLOR)
        assertTrue(white.ownText)
        assertEquals(DEFAULT_TEXT_COLOR, white.textColor)
        // "None" picked removes a file's own boxes.
        assertTrue(defaults.withBackground(0).ownBackground)
        // A colour other than the default always did.
        assertTrue(defaults.copy(textColor = 0xFFFFFF00.toInt()).ownText)
        assertTrue(defaults.copy(backgroundColor = 0x80000000.toInt()).ownBackground)
    }

    @Test
    fun aHueMovedOnWhiteOrBlackShows() {
        val white = floatArrayOf(0f, 0f, 1f)
        white[0] = 200f
        showMovedHue(white, hueMoved = true)
        assertEquals(1f, white[1], 0f)
        assertEquals(1f, white[2], 0f)
        val black = floatArrayOf(0f, 0f, 0f)
        black[1] = 0.6f
        showMovedHue(black, hueMoved = false)
        assertEquals(0.6f, black[1], 0f)
        assertEquals(1f, black[2], 0f)
        // A colour that shows its hue keeps its saturation and brightness.
        val teal = floatArrayOf(180f, 0.4f, 0.7f)
        showMovedHue(teal, hueMoved = true)
        assertEquals(0.4f, teal[1], 0f)
        assertEquals(0.7f, teal[2], 0f)
    }

    @Test
    fun oneLiveCueShowsAtATime() {
        val track = LiveTrack("ai", "ai")
        track.cues += LiveCue(1_000, 4_000, "and then he")
        // The same stretch heard again: a newer line over the older one.
        track.cues += LiveCue(3_000, 6_000, "then he said")
        track.cues += LiveCue(8_000, 9_000, "Later")
        assertNull(track.at(500))
        assertEquals("and then he", track.at(2_000)?.text)
        // Where they overlap the newer shows, alone, and the older doesn't come back.
        assertEquals("then he said", track.at(3_500)?.text)
        assertEquals("then he said", track.at(5_000)?.text)
        assertNull(track.at(7_000))
        assertEquals("Later", track.at(8_000)?.text)
        assertNull(track.at(9_000))
        assertNull(LiveTrack("t", "translation").at(1_000))
    }
}

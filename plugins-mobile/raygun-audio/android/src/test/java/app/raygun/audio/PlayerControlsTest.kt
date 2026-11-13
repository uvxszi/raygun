package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerControlsTest {
    @Test
    fun aDragMovesFromWhereTheVideoWasByTheShareOfTheBar() {
        assertEquals(120_000L, dragPosition(60_000, 100f, 1_000, 600_000))
        assertEquals(30_000L, dragPosition(60_000, -50f, 1_000, 600_000))
    }

    @Test
    fun aDragStaysWithinTheVideo() {
        assertEquals(0L, dragPosition(10_000, -500f, 1_000, 600_000))
        assertEquals(600_000L, dragPosition(590_000, 500f, 1_000, 600_000))
        assertEquals(0L, dragPosition(10_000, 100f, 1_000, 0))
        // A bar not laid out yet doesn't divide by nothing.
        assertEquals(600_000L, dragPosition(0, 5f, 0, 600_000))
    }

    @Test
    fun aSheetPanelKeepsToTheRoomBetweenTheBars() {
        // A wide sheet (880 dp) in a landscape phone's room beside a button bar takes the room.
        assertEquals(2_100, sheetPanelSize(2_100, 2_420))
        assertEquals(1_540, sheetPanelSize(2_100, 1_540))
        assertEquals(0, sheetPanelSize(-20, 1_540))
    }

    @Test
    fun theSmallWindowGrowsOutOfThePictureNotTheWholeScreen() {
        // A 16:9 video in a landscape phone's 2280 × 1080 frame: pillarboxed in the middle.
        assertEquals(listOf(180, 0, 2_100, 1_080), pipSourceRect(0, 0, 2_280, 1_080, 1_920, 1_080))
        // A wide film in a portrait frame: letterboxed in the middle.
        assertEquals(listOf(0, 945, 1_080, 1_395), pipSourceRect(0, 0, 1_080, 2_340, 2_400, 1_000))
        // An unknown size: the whole frame, where it is.
        assertEquals(listOf(10, 20, 110, 220), pipSourceRect(10, 20, 100, 200, 0, 0))
    }

    @Test
    fun aPausedFrameIsDrawnAgainTheBestWayThereIs() {
        assertEquals(PausedRedraw.REPLAY, pausedRedrawWay(replayed = true, seekable = true))
        assertEquals(PausedRedraw.SEEK, pausedRedrawWay(replayed = false, seekable = true))
        assertEquals(PausedRedraw.NUDGE, pausedRedrawWay(replayed = false, seekable = false))
    }

    @Test
    fun speedsCompareByStep() {
        assertTrue(sameSpeed(1.25f, 125 / 100f))
        assertTrue(sameSpeed(0.3f, (25 + 1 * 5) / 100f))
        assertFalse(sameSpeed(1.25f, 1.3f))
    }
}

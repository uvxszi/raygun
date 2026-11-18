package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Test

class PlayerUiTest {
    @Test
    fun aChipPastTheEdgeScrollsIntoView() {
        // Custom at 1,150–1,400 in a 1,080 wide row: scrolled so it ends at the edge.
        assertEquals(320, chipScroll(1_150, 1_400, 1_080))
        // Never past the chip's own start, nor before the row's.
        assertEquals(100, chipScroll(100, 2_000, 1_080))
        assertEquals(0, chipScroll(0, 500, 1_080))
    }

    @Test
    fun aSheetRevealsTheChosenRowCentredOrOnlyAsFarAsNeeded() {
        // A row at 900–1,060 in a 700 tall sheet: centred, or just enough for its bottom to show.
        assertEquals(630, revealScroll(900, 160, 700, nearest = false))
        assertEquals(360, revealScroll(900, 160, 700, nearest = true))
        // Already in view: the sheet stays at its top (Audio keeps smart audio showing).
        assertEquals(0, revealScroll(400, 160, 700, nearest = true))
        // A row taller than the sheet shows from its top.
        assertEquals(900, revealScroll(900, 900, 700, nearest = true))
    }
}

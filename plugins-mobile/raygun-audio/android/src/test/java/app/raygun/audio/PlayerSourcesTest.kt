package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Test

class PlayerSourcesTest {
    private fun order(vararg options: OptionRank): List<Int> = options.toList().sortedWith(OPTION_ORDER).map { it.index }

    @Test
    fun optionsWithAndWithoutAHeightSortWithoutFailing() {
        // Some links name their resolution and others don't: the unknown ones go last.
        assertEquals(
            listOf(1, 3, 0, 2),
            order(
                OptionRank(0),
                OptionRank(1, height = 1080),
                OptionRank(2),
                OptionRank(3, height = 720),
            ),
        )
    }

    @Test
    fun aMeasuredSpeedOrABitRateOnSomeLinksOnlySortsToo() {
        // The same height: the measured one first, then the one whose bit rate is known.
        assertEquals(
            listOf(1, 2, 0),
            order(
                OptionRank(0, height = 1080),
                OptionRank(1, height = 1080, speedKbps = 9_000),
                OptionRank(2, height = 1080, bitrateKbps = 4_000),
            ),
        )
        assertEquals(
            listOf(2, 0, 1),
            order(
                OptionRank(0, speedKbps = 2_000),
                OptionRank(1),
                OptionRank(2, speedKbps = 25_000, bitrateKbps = 8_000),
            ),
        )
    }

    @Test
    fun theBetterRangeComesFirstAtOneHeight() {
        assertEquals(
            listOf(2, 1, 0, 3),
            order(
                OptionRank(0, height = 2160, range = "sdr"),
                OptionRank(1, height = 2160, range = "hdr"),
                OptionRank(2, height = 2160, range = "dv"),
                OptionRank(3, height = 1080, range = "dv"),
            ),
        )
    }

    @Test
    fun equalOptionsKeepTheirOrder() {
        assertEquals(listOf(0, 1, 2), order(OptionRank(0), OptionRank(1), OptionRank(2)))
    }

    @Test
    fun sourcesFollowRustsOrderThenTheRest() {
        assertEquals(listOf(2, 0, 1, 3), orderedSources(4, listOf(2, 0)) { true })
        // Places past the list, below zero or twice count once or not at all.
        assertEquals(listOf(1, 0, 2), orderedSources(3, listOf(1, 7, -1, 1)) { true })
        // Hidden ones (failed or empty, as set) are left out.
        assertEquals(listOf(2, 3), orderedSources(4, listOf(2, 0)) { it != 0 && it != 1 })
        assertEquals(emptyList<Int>(), orderedSources(0, listOf(0, 1)) { true })
    }

    @Test
    fun speedsAndSizesReadShort() {
        assertEquals("800 kb/s", speedText(800))
        assertEquals("25 Mb/s", speedText(25_000))
        assertEquals("2.5 Mb/s", speedText(2_500))
        assertEquals("15.2 GB", sizeText(15_200_000_000))
        assertEquals("700 MB", sizeText(700_000_000))
        assertEquals("1 MB", sizeText(20_000))
    }
}

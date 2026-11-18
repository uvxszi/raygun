package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PlayerAutoQualityTest {
    /** Ticks every 250 ms from `from` to `to`; the actions seen, acted on at once. */
    private fun run(
        watch: StallWatch,
        from: Long,
        to: Long,
        canRaise: Boolean = false,
        stalled: (Long) -> Boolean,
    ): List<Pair<Long, StallAction>> {
        val actions = mutableListOf<Pair<Long, StallAction>>()
        var now = from
        while (now <= to) {
            watch.tick(now, true, stalled(now), canRaise)?.let {
                actions += now to it
                watch.stepped(now)
            }
            now += 250
        }
        return actions
    }

    @Test
    fun twoStallsWithinThirtySecondsStepDown() {
        val watch = StallWatch().apply { reset(0) }
        val actions = run(watch, 0, 60_000) { it in 20_000 until 21_000 || it in 35_000 until 36_000 }
        assertEquals(listOf(35_000L to StallAction.LOWER), actions)
    }

    @Test
    fun stallsFarApartAreLeftAloneAndOneLongStallStepsDownOnce() {
        val apart = StallWatch().apply { reset(0) }
        assertEquals(emptyList<Pair<Long, StallAction>>(), run(apart, 0, 60_000) { it in 10_000 until 11_000 || it in 50_000 until 51_000 })
        val long = StallWatch().apply { reset(0) }
        assertEquals(
            listOf(20_000L + AutoQualityRules.LONG_STALL_MS to StallAction.LOWER),
            run(long, 0, 40_000) { it in 20_000 until 30_000 },
        )
    }

    @Test
    fun stallsWhileANewQualitySettlesDontCount() {
        val watch = StallWatch().apply { reset(0) }
        assertEquals(
            emptyList<Pair<Long, StallAction>>(),
            run(watch, 0, AutoQualityRules.SETTLE_MS + 5_000) { it < AutoQualityRules.SETTLE_MS && it % 2_000 < 1_000 },
        )
    }

    @Test
    fun aLongSmoothStretchStepsBackUpOnlyWhenItCan() {
        val smooth = AutoQualityRules.SMOOTH_MS
        assertEquals(emptyList<Pair<Long, StallAction>>(), run(StallWatch().apply { reset(0) }, 0, smooth + 1_000) { false })
        assertEquals(
            listOf(smooth to StallAction.RAISE),
            run(StallWatch().apply { reset(0) }, 0, smooth + 1_000, canRaise = true) { false },
        )
        // A stall starts the stretch again.
        assertEquals(
            emptyList<Pair<Long, StallAction>>(),
            run(StallWatch().apply { reset(0) }, 0, smooth + 1_000, canRaise = true) { it in 60_000 until 60_500 },
        )
    }

    private fun step(height: Int, state: String, sizes: List<Pair<Int, Int>> = emptyList()) =
        QualityStep("$height:sdr", height, "sdr", state, sizes)

    @Test
    fun adaptiveStreamsStepAmongTheirVariantsNeverBelowTheLowest() {
        val rows = listOf(
            step(1080, "playing"),
            step(2160, "otherSource"),
            step(720, "variant", listOf(1280 to 720)),
            step(480, "variant"),
            step(360, "thisSource"),
        )
        assertEquals(720, lowerStep(rows, adaptive = true)?.height)
        assertNull(lowerStep(listOf(step(480, "playing"), step(360, "thisSource")), adaptive = true))
    }

    @Test
    fun aSingleLinkStepsDownToAnotherLinksLowerStep() {
        val rows = listOf(step(1080, "playing"), step(720, "otherSource"), step(480, "thisSource"))
        assertEquals(720, lowerStep(rows, adaptive = false)?.height)
        assertNull(lowerStep(listOf(step(480, "playing")), adaptive = false))
    }

    @Test
    fun capsFollowTheRowsPicturesAndLiftAStepAtATime() {
        assertEquals(1080, pictureHeight(1920, 800))
        assertEquals(720, pictureHeight(1280, 720))
        assertEquals(1080, capOf(step(1080, "variant", listOf(1920 to 816))))
        val rows = listOf(step(1080, "variant"), step(720, "variant"), step(480, "playing"))
        assertEquals(720, raiseStep(rows, 480)?.height)
        assertNull(raiseStep(rows, 1080))
    }

    @Test
    fun aSourcesOptionsFollowRustsRowsElseEachLinkAlone() {
        val rust = listOf(SourceOptionRow(listOf(2, 1, 9), true), SourceOptionRow(listOf(0), false), SourceOptionRow(listOf(7), false))
        // Links the source doesn't have are left out, and a row left empty goes.
        assertEquals(
            listOf(SourceOptionRow(listOf(2, 1), true), SourceOptionRow(listOf(0), false)),
            sourceOptionRows(rust, 3) { listOf(0, 1, 2) },
        )
        assertEquals(
            listOf(SourceOptionRow(listOf(1), false), SourceOptionRow(listOf(0), false)),
            sourceOptionRows(emptyList(), 2) { listOf(1, 0) },
        )
    }
}

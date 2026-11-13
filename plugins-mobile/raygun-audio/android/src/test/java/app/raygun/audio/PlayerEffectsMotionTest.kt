package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs
import kotlin.math.sin

class PlayerEffectsMotionTest {
    /** A textured picture (smooth waves, so linear taps between pixels read it well). */
    private fun texture(width: Int, height: Int, shiftX: Double = 0.0, shiftY: Double = 0.0): LumaImage =
        LumaImage(width, height, FloatArray(width * height) { index ->
            val x = index % width - shiftX
            val y = index / width - shiftY
            (0.5 + 0.2 * sin(x * 0.45) * sin(y * 0.37) + 0.15 * sin(x * 0.13 + y * 0.21)).toFloat()
        })

    @Test
    fun theRateFollowsTheScreenEvenly() {
        assertEquals(60.0, MotionMath.targetFps(60f), 1e-9)
        assertEquals(45.0, MotionMath.targetFps(90f), 1e-9)
        assertEquals(60.0, MotionMath.targetFps(120f), 1e-9)
        assertEquals(48.0, MotionMath.targetFps(144f), 1e-9)
        assertEquals(50.0, MotionMath.targetFps(50f), 1e-9)
        // An unknown rate: 60.
        assertEquals(60.0, MotionMath.targetFps(Float.NaN), 1e-9)
        assertEquals(16_666.67, MotionMath.intervalUs(60.0, 1f), 0.01)
        // Twice as fast: twice the media time between frames shown.
        assertEquals(33_333.33, MotionMath.intervalUs(60.0, 2f), 0.01)
    }

    @Test
    fun framesComeOnAnEvenBeat() {
        val interval = MotionMath.intervalUs(60.0, 1f)
        // 24 fps: the first frame went as it is; two in-between frames before the next.
        val first = MotionMath.slots(0, 41_667, 0, interval)
        assertEquals(listOf(16_667L, 33_333L), first.map { it.pts })
        assertEquals(0.4f, first[0].t, 0.001f)
        assertEquals(0.8f, first[1].t, 0.001f)
        val second = MotionMath.slots(41_667, 83_333, 33_333, interval)
        assertEquals(listOf(50_000L, 66_667L), second.map { it.pts })
        assertEquals(0.2f, second[0].t, 0.001f)
        // A frame on the beat goes as it is (t 0), the next gap's.
        val third = MotionMath.slots(83_333, 125_000, 66_667, interval)
        assertEquals(83_333L, third[0].pts)
        assertEquals(0f, third[0].t, 0.001f)
        // Nothing twice.
        assertTrue(MotionMath.slots(0, 41_667, 41_000, interval).isEmpty())
        assertTrue(MotionMath.slots(10, 10, Long.MIN_VALUE, interval).isEmpty())
    }

    @Test
    fun onlyGapsWithRoomAreFilled() {
        val interval = MotionMath.intervalUs(60.0, 1f)
        assertTrue(MotionMath.interpolates(41_667, interval))
        assertTrue(MotionMath.interpolates(33_367, interval))
        // 50 and 60 fps come often enough; a seek or a stall is no motion.
        assertFalse(MotionMath.interpolates(16_683, interval))
        assertFalse(MotionMath.interpolates(20_000, interval))
        assertFalse(MotionMath.interpolates(400_000, interval))
        assertFalse(MotionMath.interpolates(-5, interval))
    }

    @Test
    fun theFlowKeepsQuarterPixelsInEightBits() {
        for (flow in listOf(-31.5, -7.25, 0.0, 0.25, 3.0, 12.75, 31.5)) {
            assertEquals(flow, MotionMath.decodeFlow(MotionMath.encodeFlow(flow)), 0.13)
        }
        // No motion is exact.
        assertEquals(0.0, MotionMath.decodeFlow(MotionMath.encodeFlow(0.0)), 0.0)
        // Beyond the range: held at its ends.
        assertEquals(31.87, MotionMath.decodeFlow(MotionMath.encodeFlow(80.0)), 0.01)
    }

    @Test
    fun theSearchFindsAShift() {
        val prev = texture(48, 32)
        val cur = texture(48, 32, shiftX = 2.0, shiftY = -1.0)
        val match = MotionMath.search(prev, cur, 24, 16, 0.0, 0.0, 3, 1.0)
        assertEquals(2.0, match.x, 1e-9)
        assertEquals(-1.0, match.y, 1e-9)
        assertTrue(match.cost < 0.03)
        // A still picture stays still.
        val still = MotionMath.search(prev, prev, 24, 16, 0.0, 0.0, 3, 1.0)
        assertEquals(0.0, still.x, 0.0)
        assertEquals(0.0, still.y, 0.0)
    }

    @Test
    fun coarseToFineFollowsLargerMotion() {
        // Nine pixels: past one level's reach, found through the pyramid, refined to half pixels.
        val prev = texture(128, 96)
        val cur = texture(128, 96, shiftX = 9.5, shiftY = 4.0)
        val match = MotionMath.flowAt(prev, cur, 64, 48)
        assertEquals(9.5, match.x, 0.51)
        assertEquals(4.0, match.y, 0.51)
        assertTrue(match.cost < 0.03)
    }

    @Test
    fun cutsRepeatTheNearerFrame() {
        assertFalse(MotionMath.isCut(meanCost = 0.02, badShare = 0.05))
        assertTrue(MotionMath.isCut(meanCost = 0.2, badShare = 0.3))
        assertTrue(MotionMath.isCut(meanCost = 0.05, badShare = 0.6))
    }

    @Test
    fun blendingTrustsAgreeingSamples() {
        // Agreeing samples blend by t.
        assertEquals(0.2 + 0.5 * 0.4, MotionMath.blend(0.2, 0.7, 0.4, differ = 0.0), 1e-9)
        // Disagreeing ones: the nearer frame's sample alone.
        assertEquals(0.2, MotionMath.blend(0.2, 0.7, 0.4, differ = 0.5), 1e-9)
        assertEquals(0.7, MotionMath.blend(0.2, 0.7, 0.6, differ = 0.5), 1e-9)
        // Between: some of each, more blend the closer they agree.
        val some = MotionMath.blend(0.2, 0.7, 0.4, differ = 0.1)
        assertTrue(some > 0.2 && some < 0.4)
        assertEquals(1.0, MotionMath.trust(0.0), 1e-9)
        assertEquals(0.0, MotionMath.trust(1.0), 1e-9)
    }

    @Test
    fun aSlowPhoneStepsDownThenGivesUp() {
        // 24 fps: 41.7 ms between frames, 45% of it for smooth motion.
        assertEquals(0, MotionMath.nextQuality(0, costMs = 12.0, gapMs = 41.7))
        assertEquals(1, MotionMath.nextQuality(0, costMs = 25.0, gapMs = 41.7))
        assertEquals(2, MotionMath.nextQuality(1, costMs = 25.0, gapMs = 41.7))
        assertEquals(MotionMath.GIVE_UP, MotionMath.nextQuality(2, costMs = 25.0, gapMs = 41.7))
        assertEquals(384 to 216, MotionMath.flowSize(1920, 1080, 0))
        assertEquals(160 to 90, MotionMath.flowSize(1920, 1080, 2))
        // Never wider than the video.
        assertEquals(120, MotionMath.flowSize(120, 68, 0).first)
    }

    @Test
    fun smoothMotionIsAPictureEffect() {
        val on = PictureEffects(smoothMotion = true)
        assertFalse(on.neutral)
        assertEquals(listOf(PictureEffects.SMOOTH_MOTION), on.heavy)
        assertTrue(on.without(PictureEffects.SMOOTH_MOTION).neutral)
        assertTrue(PictureEffects(sharpen = 20, smoothMotion = true).adjustments == PictureEffects(sharpen = 20))
        assertTrue(abs(MotionMath.channel(0.5) - 128 / 255.0) < 1e-9)
    }
}

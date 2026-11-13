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
    fun framesComeEvenlyAndEndOnTheDecodedOne() {
        val interval = MotionMath.intervalUs(60.0, 1f)
        // 30 fps on 60: one in-between frame half way, then the next frame itself.
        val thirty = MotionMath.steps(0, 33_333, interval)
        assertEquals(listOf(16_667L, 33_333L), thirty.map { it.pts })
        assertEquals(listOf(0.5f, 1f), thirty.map { it.t })
        // 24 fps: never more than the screen's share (48 a second, not 72).
        val film = MotionMath.steps(41_667, 83_333, interval)
        assertEquals(listOf(62_500L, 83_333L), film.map { it.pts })
        // 12 fps: five steps.
        assertEquals(listOf(0.2f, 0.4f, 0.6f, 0.8f, 1f), MotionMath.steps(0, 83_333, interval).map { it.t })
        // 90 Hz (45 a second) at 24 fps: the decoded frame alone.
        assertEquals(listOf(1f), MotionMath.steps(0, 41_667, MotionMath.intervalUs(45.0, 1f)).map { it.t })
        // The last is always the decoded frame's own time.
        assertEquals(41_667L, MotionMath.steps(0, 41_667, interval).last().pts)
        assertTrue(MotionMath.steps(10, 10, interval).isEmpty())
    }

    @Test
    fun theWatchdogTurnsAStalledPictureOff() {
        val watchdog = MotionWatchdog(limitMs = 500)
        // Watching starts with the first check while playing: no stall before any frame was due.
        assertFalse(watchdog.stalled(1_000, playing = true, on = true))
        watchdog.sent(1_200)
        assertFalse(watchdog.stalled(1_600, playing = true, on = true))
        // Frames keep coming: fine.
        watchdog.sent(2_000)
        assertFalse(watchdog.stalled(2_400, playing = true, on = true))
        // No frame for more than half a second while playing: stalled.
        assertTrue(watchdog.stalled(2_600, playing = true, on = true))
        // Paused, or smooth motion off: never, and watching starts again after.
        assertFalse(watchdog.stalled(5_000, playing = false, on = true))
        assertFalse(watchdog.stalled(5_100, playing = true, on = true))
        assertFalse(watchdog.stalled(5_500, playing = true, on = true))
        assertTrue(watchdog.stalled(5_700, playing = true, on = true))
        assertFalse(watchdog.stalled(9_000, playing = true, on = false))
        // Without any frame since watching began: stalled after the limit.
        val fresh = MotionWatchdog(limitMs = 500)
        assertFalse(fresh.stalled(100, playing = true, on = true))
        assertTrue(fresh.stalled(700, playing = true, on = true))
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

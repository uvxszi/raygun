package app.raygun.audio

import androidx.media3.common.Player
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerScreenTest {
    @Test
    fun theScreenStaysOnWhileTheVideoPlaysOrWaitsToPlay() {
        assertTrue(screenStaysOn(playWhenReady = true, Player.STATE_READY, keepWhilePaused = false))
        // Buffering after a seek or while the network catches up: the video is still playing.
        assertTrue(screenStaysOn(playWhenReady = true, Player.STATE_BUFFERING, keepWhilePaused = false))
    }

    @Test
    fun pausedEndedOrFailedTheSystemTimeoutApplies() {
        assertFalse(screenStaysOn(playWhenReady = false, Player.STATE_READY, keepWhilePaused = false))
        assertFalse(screenStaysOn(playWhenReady = false, Player.STATE_BUFFERING, keepWhilePaused = false))
        assertFalse(screenStaysOn(playWhenReady = true, Player.STATE_ENDED, keepWhilePaused = false))
        assertFalse(screenStaysOn(playWhenReady = true, Player.STATE_IDLE, keepWhilePaused = false))
    }

    @Test
    fun keepingTheScreenOnWhilePausedKeepsItOnThroughout() {
        assertTrue(screenStaysOn(playWhenReady = false, Player.STATE_READY, keepWhilePaused = true))
        assertTrue(screenStaysOn(playWhenReady = true, Player.STATE_ENDED, keepWhilePaused = true))
    }

    @Test
    fun delayStepsAreASecondAndATenthEitherWayWithinTenMinutes() {
        assertEquals(listOf(-1000L, -100L, 100L, 1000L), DELAY_STEPS.map { it.first })
        assertEquals(1_100L, steppedDelay(1_000L, 100L))
        assertEquals(-900L, steppedDelay(100L, -1000L))
        assertEquals(MAX_DELAY_MS, steppedDelay(MAX_DELAY_MS - 50L, 100L))
        assertEquals(-MAX_DELAY_MS, steppedDelay(-MAX_DELAY_MS, -1000L))
    }
}

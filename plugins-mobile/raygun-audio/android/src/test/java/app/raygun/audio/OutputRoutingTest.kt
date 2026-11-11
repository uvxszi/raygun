package app.raygun.audio

import android.media.AudioDeviceInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OutputRoutingTest {
    @Test
    fun aTrackOnThePickedOutputIsKept() {
        assertEquals(RouteVerdict.Kept, routeVerdict(onPicked = true, holds = 0, sinceHoldMs = 0))
        assertEquals(RouteVerdict.Kept, routeVerdict(onPicked = true, holds = 3, sinceHoldMs = 0))
    }

    @Test
    fun aTrackElsewhereIsHeldAtOnceThenAWhileApart() {
        assertEquals(RouteVerdict.Hold, routeVerdict(onPicked = false, holds = 0, sinceHoldMs = 0))
        assertEquals(RouteVerdict.Later, routeVerdict(onPicked = false, holds = 1, sinceHoldMs = 650))
        assertEquals(
            RouteVerdict.Hold,
            routeVerdict(onPicked = false, holds = 1, sinceHoldMs = OutputKeeper.RETRY_MS),
        )
        assertEquals(RouteVerdict.Later, routeVerdict(onPicked = false, holds = 2, sinceHoldMs = 10))
    }

    @Test
    fun androidKeepingTheSoundElsewhereIsSaidAfterTheLastHold() {
        assertEquals(
            RouteVerdict.Refused,
            routeVerdict(onPicked = false, holds = OutputKeeper.MAX_HOLDS, sinceHoldMs = 60_000),
        )
    }

    @Test
    fun outputsAreNamedByKind() {
        assertEquals("speaker", outputKindOf(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER))
        assertEquals("wired", outputKindOf(AudioDeviceInfo.TYPE_WIRED_HEADPHONES))
        assertEquals("wired", outputKindOf(AudioDeviceInfo.TYPE_WIRED_HEADSET))
        assertEquals("bluetooth", outputKindOf(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP))
        assertEquals("usb", outputKindOf(AudioDeviceInfo.TYPE_USB_HEADSET))
        assertEquals("hdmi", outputKindOf(AudioDeviceInfo.TYPE_HDMI))
        assertNull(outputKindOf(AudioDeviceInfo.TYPE_BUILTIN_EARPIECE))
        assertNull(outputKindOf(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER_SAFE))
    }

    @Test
    fun headphonesTakeTheSoundAndSpeakersDont() {
        assertTrue(takesOverSound(AudioDeviceInfo.TYPE_WIRED_HEADPHONES))
        assertTrue(takesOverSound(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP))
        assertTrue(takesOverSound(AudioDeviceInfo.TYPE_USB_HEADSET))
        assertFalse(takesOverSound(AudioDeviceInfo.TYPE_BUILTIN_SPEAKER))
        assertFalse(takesOverSound(AudioDeviceInfo.TYPE_HDMI))
        assertFalse(takesOverSound(AudioDeviceInfo.TYPE_BUILTIN_EARPIECE))
    }
}

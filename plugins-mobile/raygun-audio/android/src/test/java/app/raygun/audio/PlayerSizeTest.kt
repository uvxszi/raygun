package app.raygun.audio

import org.junit.Assert.assertEquals
import org.junit.Test

class PlayerSizeTest {
    @Test
    fun interfaceSizeKeepsToThePhonesRange() {
        assertEquals(1.25f, PlayerSize.clampInterface(1.25f))
        assertEquals(0.7f, PlayerSize.clampInterface(0.5f))
        assertEquals(1.5f, PlayerSize.clampInterface(2f))
        assertEquals(1f, PlayerSize.clampInterface(Float.NaN))
    }

    @Test
    fun textSizeKeepsToItsRange() {
        assertEquals(1.4f, PlayerSize.clampText(1.4f))
        assertEquals(0.7f, PlayerSize.clampText(0.1f))
        assertEquals(2f, PlayerSize.clampText(3f))
    }

    @Test
    fun theDensityScalesToWholeDpi() {
        // A Pixel 4 (440 dpi) at 125 % and 75 %.
        assertEquals(550, PlayerSize.scaledDensityDpi(440, 1.25f))
        assertEquals(330, PlayerSize.scaledDensityDpi(440, 0.75f))
        assertEquals(440, PlayerSize.scaledDensityDpi(440, 1f))
    }

    @Test
    fun textGrowsOverTheSystemsFontScale() {
        assertEquals(1.3f * 1.2f, PlayerSize.scaledFontScale(1.3f, 1.2f), 1e-6f)
        assertEquals(1f, PlayerSize.scaledFontScale(1f, 1f), 1e-6f)
    }
}

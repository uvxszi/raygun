package app.raygun.audio

import android.content.pm.ActivityInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerSubtitleCarryTest {
    /** Another title's subtitles, named its own way. */
    private val other = listOf(
        SubtitleCandidate("en", "Signs & Songs", SubtitleKind.FORCED),
        SubtitleCandidate("eng", "English CC", SubtitleKind.SDH),
        SubtitleCandidate("en", "English", SubtitleKind.PLAIN),
        SubtitleCandidate("fr", "Français", SubtitleKind.PLAIN),
    )

    @Test
    fun theLastChoiceFindsTheSameLanguageAndKindWhateverTheNames() {
        assertEquals(1, carriedMatch(other, TrackChoice("en", "English (SDH)", SubtitleKind.SDH)))
        assertEquals(0, carriedMatch(other, TrackChoice("English", "Forced", SubtitleKind.FORCED)))
        assertEquals(2, carriedMatch(other, TrackChoice("en", "Full", SubtitleKind.PLAIN)))
        assertEquals(3, carriedMatch(other, TrackChoice("fra", "French", null)))
        // The same name wins within a kind.
        val two = listOf(SubtitleCandidate("en", "Full", SubtitleKind.PLAIN), SubtitleCandidate("en", "English", SubtitleKind.PLAIN))
        assertEquals(1, carriedMatch(two, TrackChoice("en", "English", SubtitleKind.PLAIN)))
    }

    @Test
    fun fullAndSdhStandInForEachOtherButForcedNever() {
        assertEquals(1, carriedMatch(other.take(2), TrackChoice("en", "English", SubtitleKind.PLAIN)))
        assertEquals(2, carriedMatch(listOf(other[0], other[3], other[2]), TrackChoice("en", "SDH", SubtitleKind.SDH)))
        assertNull(carriedMatch(other.take(1), TrackChoice("en", "English", SubtitleKind.PLAIN)))
        assertNull(carriedMatch(other.drop(1), TrackChoice("en", "Forced", SubtitleKind.FORCED)))
        // No language, no match: the usual rules decide.
        assertNull(carriedMatch(other, TrackChoice(null, "English", SubtitleKind.PLAIN)))
        assertNull(carriedMatch(other, TrackChoice("de", "Deutsch", SubtitleKind.PLAIN)))
    }

    @Test
    fun aTracksKindComesFromItsFlagsOrItsName() {
        assertEquals(SubtitleKind.FORCED, SubtitleKind.of(forced = true, hearing = false, label = "English"))
        assertEquals(SubtitleKind.FORCED, SubtitleKind.of(forced = false, hearing = false, label = "English Forced"))
        assertEquals(SubtitleKind.SDH, SubtitleKind.of(forced = false, hearing = true, label = null))
        assertEquals(SubtitleKind.SDH, SubtitleKind.of(forced = false, hearing = false, label = "ENG SDH"))
        assertEquals(SubtitleKind.PLAIN, SubtitleKind.of(forced = false, hearing = false, label = "Hindi"))
        assertEquals(SubtitleKind.PLAIN, SubtitleKind.read(null))
        assertEquals(SubtitleKind.SDH, SubtitleKind.read("sdh"))
    }

    @Test
    fun theSystemBarsShowWithTheControlsOrASheet() {
        assertTrue(PlayerSystemBars.wanted(controlsShown = true, locked = false, sheetOpen = false, pip = false))
        assertTrue(PlayerSystemBars.wanted(controlsShown = false, locked = false, sheetOpen = true, pip = false))
        assertFalse(PlayerSystemBars.wanted(controlsShown = false, locked = false, sheetOpen = false, pip = false))
        // Locked, only the unlock button shows; in picture-in-picture there are none.
        assertFalse(PlayerSystemBars.wanted(controlsShown = true, locked = true, sheetOpen = false, pip = false))
        assertFalse(PlayerSystemBars.wanted(controlsShown = true, locked = false, sheetOpen = true, pip = true))
    }

    @Test
    fun thePlayerFollowsTheDeviceUnlessToldOtherwise() {
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_FULL_USER, screenOrientationFor("free"))
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_FULL_USER, screenOrientationFor(""))
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE, screenOrientationFor("sensorLandscape"))
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE, screenOrientationFor("video"))
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_PORTRAIT, screenOrientationFor("portrait"))
    }
}

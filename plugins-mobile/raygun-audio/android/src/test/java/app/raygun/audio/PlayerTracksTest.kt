package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.TrackGroup
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(UnstableApi::class)
class PlayerTracksTest {
    /** A subtitle file's track group, as Media3 lists a sideloaded file: one track. */
    private fun subtitles(id: String, language: String, selected: Boolean): Tracks.Group {
        val format = Format.Builder()
            .setId(id)
            .setSampleMimeType(MimeTypes.APPLICATION_SUBRIP)
            .setLanguage(language)
            .build()
        return Tracks.Group(TrackGroup(id, format), false, intArrayOf(C.FORMAT_HANDLED), booleanArrayOf(selected))
    }

    @Test
    fun aTrackAskedForCountsAsChosenBeforeMediaThreeReportsIt() {
        // Subtitles were off: nothing plays. English is tapped; Media3 hasn't reported it yet.
        val english = subtitles("en", "en", selected = false)
        val french = subtitles("fr", "fr", selected = false)
        val asked = TrackSelectionOverride(english.mediaTrackGroup, 0)
        assertTrue(isChosen(english, 0, disabled = false, override = asked))
        assertFalse(isChosen(french, 0, disabled = false, override = asked))
    }

    @Test
    fun theChoiceAskedForWinsOverWhatStillPlays() {
        // French plays; English is tapped: English shows ticked at once, French doesn't.
        val english = subtitles("en", "en", selected = false)
        val french = subtitles("fr", "fr", selected = true)
        val asked = TrackSelectionOverride(english.mediaTrackGroup, 0)
        assertTrue(isChosen(english, 0, disabled = false, override = asked))
        assertFalse(isChosen(french, 0, disabled = false, override = asked))
    }

    @Test
    fun offTurnsEveryTrackOffAndWithoutAChoiceMediaThreeDecides() {
        val playing = subtitles("en", "en", selected = true)
        assertFalse(isChosen(playing, 0, disabled = true, override = null))
        assertFalse(isChosen(playing, 0, disabled = true, override = TrackSelectionOverride(playing.mediaTrackGroup, 0)))
        assertTrue(isChosen(playing, 0, disabled = false, override = null))
        assertFalse(isChosen(subtitles("fr", "fr", selected = false), 0, disabled = false, override = null))
    }

    @Test
    fun tracksAreNamedByTheirLanguageAndWhatElseTheirLabelSays() {
        // A label that only names the language reads as the language's name.
        assertEquals("English", trackName("eng", "en", "English"))
        assertEquals("English", trackName("en", "en", "English"))
        // A torrent's "Film.en.forced.srt": its kind after the language.
        assertEquals("English (Forced)", trackName("Forced", "en", "English"))
        assertEquals("English (Commentary)", trackName("Commentary", "en", "English"))
        // A label naming a language stays as it is.
        assertEquals("English (Forced)", trackName("English (Forced)", "en", "English"))
        // Without a language: the label alone; without a label: the language.
        assertEquals("Sintel.srt", trackName("Sintel.srt", null, null))
        assertEquals("German", trackName(null, "de", "German"))
        assertEquals("German", trackName("  ", "de", "German"))
        assertNull(trackName(null, null, null))
    }

    @Test
    fun qualityRowsNameTheirTierAndThePicturesOwnSize() {
        assertEquals("1080p (1920×816)", qualityName(1080, "1920×816", null))
        assertEquals("2160p (3840×1600)", qualityName(2160, "3840×1600", "2160p (4K)"))
        assertEquals("2160p (4K)", qualityName(2160, null, "2160p (4K)"))
        assertEquals("720p", qualityName(720, null, null))
    }

    @Test
    fun aRowSwitchesToTheVariantsItHoldsByTheirOwnSides() {
        // An HLS ladder with a cropped 1080p at two bit rates, a 720p and an HDR 1080p.
        val tracks = listOf(
            Triple(1920 to 816, "sdr", "a"),
            Triple(1920 to 816, "sdr", "b"),
            Triple(1280 to 544, "sdr", "c"),
            Triple(1920 to 816, "hdr", "d"),
            Triple(-1 to 720, "sdr", "e"),
        )
        val held = heldVariants(tracks, listOf(1920 to 816), "sdr", { it.first }, { it.second }).map { it.third }
        assertEquals(listOf("a", "b"), held)
        // A variant whose width Media3 doesn't know was reported as 0.
        val bare = heldVariants(tracks, listOf(0 to 720), "sdr", { it.first }, { it.second }).map { it.third }
        assertEquals(listOf("e"), bare)
        assertEquals(emptyList<String>(), heldVariants(tracks, emptyList(), "sdr", { it.first }, { it.second }))
    }
}

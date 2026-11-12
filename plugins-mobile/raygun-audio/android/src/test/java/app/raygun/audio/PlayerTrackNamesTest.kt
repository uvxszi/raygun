package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

@OptIn(UnstableApi::class)
class PlayerTrackNamesTest {
    private val numbered = { name: String, number: Int -> "$name $number" }
    private val word = { mark: TrackMark -> mark.fallback }

    private fun named(vararg tracks: TrackFacts) = distinctTracks(tracks.toList(), numbered)

    @Test
    fun tracksInOneLanguageAreNumberedTheirSourceUnderThem() {
        val shown = named(
            TrackFacts("English", detail = "In the video"),
            TrackFacts("English", detail = "OpenSubtitles v3"),
            TrackFacts("English", detail = "OpenSubtitles v3"),
            TrackFacts("Français", detail = "OpenSubtitles v3"),
        )
        assertEquals(
            listOf(
                ShownTrack("English", "In the video"),
                ShownTrack("English 2", "OpenSubtitles v3"),
                ShownTrack("English 3", "OpenSubtitles v3"),
                ShownTrack("Français", "OpenSubtitles v3"),
            ),
            shown,
        )
    }

    @Test
    fun whatTheFlagsSayTellsTracksApartBeforeANumberDoes() {
        val shown = named(
            TrackFacts("English"),
            TrackFacts("English", marks = listOf("SDH")),
            TrackFacts("English", marks = listOf("Forced")),
            // A name that says it already isn't told again.
            TrackFacts("English (Forced)", marks = listOf("Forced", "SDH")),
        ).map { it.name }
        assertEquals(listOf("English", "English (SDH)", "English (Forced)", "English (Forced, SDH)"), shown)
    }

    @Test
    fun aNumberNeverGivesANameAnotherTrackHas() {
        val shown = named(TrackFacts("English"), TrackFacts("English"), TrackFacts("English 2")).map { it.name }
        assertEquals(listOf("English", "English 3", "English 2"), shown)
        // Names alike in another case are alike.
        assertEquals(listOf("english", "English 2"), named(TrackFacts("english"), TrackFacts("English")).map { it.name })
    }

    @Test
    fun aFileNamedOnlyByItsFileNameDoesNotSayItTwice() {
        val shown = named(TrackFacts("film.srt", detail = "film.srt"), TrackFacts("English", detail = "  "))
        assertEquals(listOf(ShownTrack("film.srt", null), ShownTrack("English", null)), shown)
    }

    @Test
    fun aFileNameLabelGoesUnderTheLanguagesName() {
        // A file added on the device: "English", from "Film.en.srt".
        val added = trackFacts("Film.en.srt", "en", "English", emptyList(), word, null)
        assertEquals(TrackFacts("English", emptyList(), "Film.en.srt"), added)
        // An addon's track keeps its addon under it; a bare code reads as the language.
        val addon = trackFacts("eng", "en", "English", emptyList(), word, "OpenSubtitles v3")
        assertEquals(TrackFacts("English", emptyList(), "OpenSubtitles v3"), addon)
        // Without a language, the file's name is the name.
        assertEquals(TrackFacts("Sintel.srt", emptyList(), "Sintel.srt"), trackFacts("Sintel.srt", null, null, emptyList(), word, null))
        assertNull(trackFacts(null, null, null, emptyList(), word, null))
    }

    @Test
    fun marksALabelSaysAlreadyAreLeftOut() {
        val sdh = trackFacts("English CC", "en", "English", listOf(TrackMark.SDH), word, null)
        assertEquals(emptyList<String>(), sdh?.marks)
        val forced = trackFacts("Signs", "en", "English", listOf(TrackMark.FORCED, TrackMark.FORCED), word, null)
        assertEquals(listOf("Forced"), forced?.marks)
        // Words are whole words: "Hindi" doesn't say "HI".
        assertEquals(listOf("SDH"), trackFacts("Hindi", "hi", "Hindi", listOf(TrackMark.SDH), word, null)?.marks)
    }

    @Test
    fun flagsGiveTheMarks() {
        val forcedSdh = Format.Builder()
            .setSampleMimeType(MimeTypes.APPLICATION_SUBRIP)
            .setSelectionFlags(C.SELECTION_FLAG_FORCED)
            .setRoleFlags(C.ROLE_FLAG_DESCRIBES_MUSIC_AND_SOUND)
            .build()
        assertEquals(listOf(TrackMark.FORCED, TrackMark.SDH), marksOf(forcedSdh))
        // Closed captions in the picture's stream, parsed as they load.
        val captions = Format.Builder()
            .setSampleMimeType(MimeTypes.APPLICATION_MEDIA3_CUES)
            .setCodecs(MimeTypes.APPLICATION_CEA608)
            .setRoleFlags(C.ROLE_FLAG_DESCRIBES_MUSIC_AND_SOUND)
            .build()
        assertEquals(listOf(TrackMark.CC), marksOf(captions))
        val commentary = Format.Builder().setRoleFlags(C.ROLE_FLAG_COMMENTARY or C.ROLE_FLAG_DESCRIBES_VIDEO).build()
        assertEquals(listOf(TrackMark.COMMENTARY, TrackMark.DESCRIPTION), marksOf(commentary))
        assertEquals(emptyList<TrackMark>(), marksOf(Format.Builder().build()))
    }

    @Test
    fun subtitleFilesAreKnownByTheIdTheirTrackWasGiven() {
        assertEquals(3, sidecarIndex(sidecarId(3)))
        // Media3 puts the source's place in front of a merged track's id.
        assertEquals(0, sidecarIndex("1:" + sidecarId(0)))
        assertNull(sidecarIndex("0:2"))
        assertNull(sidecarIndex(null))
    }

    @Test
    fun subtitleFontsAreTheSystemsCaptionFamilies() {
        assertEquals(
            listOf(null, "sans-serif", "sans-serif-condensed", "monospace", "serif", "serif-monospace", "casual", "cursive", "sans-serif-smallcaps"),
            SUBTITLE_FONTS.map(::subtitleFamily),
        )
        // A font from a newer version draws as the default.
        assertNull(subtitleFamily("fantasy"))
    }
}

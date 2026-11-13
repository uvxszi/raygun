package app.raygun.audio

import androidx.media3.common.C
import androidx.media3.common.MimeTypes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerAudioTest {
    private fun offer(
        id: String,
        source: String,
        languages: List<String> = emptyList(),
        several: Boolean = false,
        title: String = "1080p · WEB-DL",
        sound: String? = null,
    ) = AudioOffer(id, source, languages, several, title, sound, "https://cdn.example/$id.mkv", emptyMap(), null)

    @Test
    fun aLaterSoundIsReadFromThatMuchBeforeAndAnEarlierOneFromThePosition() {
        // Two seconds later: the period reads from 8 s for a seek to 10 s, never before the start.
        assertEquals(8_000_000L, leadPosition(10_000_000L, 2_000_000L))
        assertEquals(0L, leadPosition(1_000_000L, 2_000_000L))
        // Earlier: from the position itself (its first samples go before it, unplayed).
        assertEquals(10_000_000L, leadPosition(10_000_000L, -2_000_000L))
        assertEquals(10_000_000L, leadPosition(10_000_000L, 0L))
    }

    @Test
    fun aSeekLandsWhereItWasAskedUnlessThePeriodWentElsewhere() {
        // Sent to 8 s for 10 s and landed there: the player is at 10 s.
        assertEquals(10_000_000L, landedPosition(10_000_000L, 8_000_000L, 8_000_000L, 2_000_000L))
        // Landed at 7 s instead (a keyframe): the picture is 2 s later than that.
        assertEquals(9_000_000L, landedPosition(10_000_000L, 8_000_000L, 7_000_000L, 2_000_000L))
        assertEquals(6_000_000L, landedPosition(10_000_000L, 10_000_000L, 6_000_000L, -1_000_000L))
    }

    @Test
    fun soundSamplesMoveByTheDelayButTheEndsOfTimeStay() {
        assertEquals(3_500_000L, movedTime(3_000_000L, 500_000L))
        assertEquals(2_500_000L, movedTime(3_000_000L, -500_000L))
        assertEquals(C.TIME_END_OF_SOURCE, movedTime(C.TIME_END_OF_SOURCE, 500_000L))
        assertEquals(C.TIME_UNSET, movedTime(C.TIME_UNSET, 500_000L))
    }

    @Test
    fun aTitlesSoundComesBackFromTheSameSourceInTheSameLanguage() {
        val offers = listOf(
            offer("0:1", "Torrentio", listOf("en")),
            offer("1:0", "Comet", listOf("it")),
            offer("1:3", "Comet", listOf("es")),
            offer("2:0", "MediaFusion"),
        )
        assertEquals("1:3", rememberedOffer(offers, RememberedAudio("Comet", "es"))?.id)
        // The language as a name or a three-letter code is the same language.
        assertEquals("1:0", rememberedOffer(offers, RememberedAudio("Comet", "ita"))?.id)
        // A sound that named no language comes back from an offer that names none.
        assertEquals("2:0", rememberedOffer(offers, RememberedAudio("MediaFusion", null))?.id)
        assertNull(rememberedOffer(offers, RememberedAudio("Comet", "de")))
        assertNull(rememberedOffer(offers, RememberedAudio("Elsewhere", "it")))
    }

    @Test
    fun offersAreNamedByTheirLanguagesAndSayWhereTheyComeFrom() {
        val names = mapOf("it" to "Italian", "en" to "English")
        val name = { language: String -> names[language] }
        assertEquals("Italian, English", offerName(offer("a", "Comet", listOf("it", "en")), name, "Several", "Sound"))
        assertEquals("Several", offerName(offer("b", "Comet", several = true), name, "Several", "Sound"))
        assertEquals("Sound", offerName(offer("c", "Comet"), name, "Several", "Sound"))
        // A language without a name here shows its code.
        assertEquals("XH", offerName(offer("d", "Comet", listOf("xh")), name, "Several", "Sound"))
        assertEquals(
            "Comet · 1080p · WEB-DL · DD+ 5.1 · Loading…",
            offerDetail(offer("e", "Comet", sound = "DD+ 5.1"), "Loading…"),
        )
        assertEquals("Comet", offerDetail(offer("f", "Comet", title = "")))
    }

    @Test
    fun theSoundBesideThePictureIsTheSecondSourcesTracks() {
        assertTrue(fromSecondSource("1:0"))
        assertTrue(fromSecondSource("1:audio"))
        // The video's own tracks, its subtitle files merged in by Media3 included.
        assertFalse(fromSecondSource("0:1:raygun-file-0"))
        assertFalse(fromSecondSource("0:0"))
        assertFalse(fromSecondSource("10:0"))
    }

    @Test
    fun streamsSayTheirFormat() {
        assertEquals(MimeTypes.APPLICATION_M3U8, streamMimeType("hls"))
        assertEquals(MimeTypes.APPLICATION_MPD, streamMimeType("DASH"))
        assertNull(streamMimeType(null))
        assertNull(streamMimeType("mkv"))
    }

    @Test
    fun qualityIsAutomaticOnlyWhileNothingWasPicked() {
        assertTrue(automatic(variantsAuto = true, wanted = null))
        assertFalse(automatic(variantsAuto = true, wanted = "1080:sdr"))
        assertFalse(automatic(variantsAuto = false, wanted = null))
    }

    @Test
    fun whatPlaysSitsOnAFaintAccent() {
        assertEquals(0x38E53935, accentTint(0xFFE53935.toInt()))
        assertEquals(0x3865ADFF, accentTint(0xFF65ADFF.toInt()))
    }
}

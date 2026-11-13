package app.raygun.audio

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.log10
import kotlin.math.sin
import kotlin.math.sqrt

class PlayerEffectsTest {
    private fun close(expected: DoubleArray, actual: DoubleArray, delta: Double = 1e-5) =
        assertArrayEquals(expected, actual, delta)

    @Test
    fun noEffectsLeaveThePictureAsItIs() {
        val shader = EffectMath.shader(PictureEffects.NONE)
        close(ColourTransform.IDENTITY.matrix, shader.colour.matrix)
        close(DoubleArray(3), shader.colour.offset)
        assertEquals(1f, shader.gamma, 0f)
        assertEquals(0f, shader.sharpen, 0f)
        assertFalse(shader.deflicker || shader.denoise)
        assertEquals("final", shader.passes)
    }

    @Test
    fun lightAndContrastMapAsOnDesktop() {
        // Contrast 100 doubles the distance from mid-grey; brightness 100 adds a quarter.
        val both = EffectMath.colour(PictureEffects(contrast = 100, brightness = 100))
        close(doubleArrayOf(2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0), both.matrix)
        close(doubleArrayOf(-0.25, -0.25, -0.25), both.offset)
        // Saturation −100 is grey: every row the same weights.
        val grey = EffectMath.colour(PictureEffects(saturation = -100)).matrix
        close(doubleArrayOf(0.213, 0.715, 0.072), grey.copyOfRange(0, 3))
        close(grey.copyOfRange(0, 3), grey.copyOfRange(6, 9))
        assertEquals(0.5, EffectMath.gammaExponent(100), 1e-9)
        assertEquals(2.0, EffectMath.gammaExponent(-100), 1e-9)
    }

    /** The same numbers as `effectParams.test.ts`, so both players draw alike. */
    @Test
    fun aMixOfEffectsComposesInOrder() {
        val warm = EffectMath.colour(PictureEffects(brightness = 10, contrast = 50, saturation = 20, hue = 30, look = "warm"))
        close(
            doubleArrayOf(1.36654, -0.646366, 0.697261, 0.117189, 1.548819, -0.217274, -0.727182, 0.567029, 1.358846),
            warm.matrix,
        )
        close(doubleArrayOf(-0.182107, -0.182107, -0.182107), warm.offset)
        val sepia = EffectMath.colour(PictureEffects(look = "sepia", gamma = 40, contrast = -30))
        close(
            doubleArrayOf(0.319215, 0.624622, 0.153516, 0.283476, 0.557205, 0.136458, 0.220933, 0.433743, 0.106405),
            sepia.matrix,
        )
        close(doubleArrayOf(0.093874, 0.093874, 0.093874), sepia.offset)
        val shader = EffectMath.shader(PictureEffects(gamma = 40, deflicker = 60, denoise = 35, sharpen = 15))
        assertEquals(0.757858, shader.gamma.toDouble(), 1e-5)
        assertEquals(0.24, shader.sharpen.toDouble(), 1e-6)
        assertEquals(0.19, shader.follow.toDouble(), 1e-6)
        assertEquals(0.36, shader.spatial.toDouble(), 1e-6)
        assertEquals(0.0375, shader.spatialSigma.toDouble(), 1e-6)
        assertEquals(0.49, shader.temporal.toDouble(), 1e-6)
        assertEquals(0.034, shader.temporalSigma.toDouble(), 1e-6)
        assertEquals("stats+clean+final", shader.passes)
    }

    @Test
    fun theMatrixGoesToGlslColumnByColumn() {
        val transform = ColourTransform(doubleArrayOf(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0), DoubleArray(3))
        assertArrayEquals(floatArrayOf(1f, 4f, 7f, 2f, 5f, 8f, 3f, 6f, 9f), transform.columns(), 0f)
    }

    @Test
    fun keepsValuesWithinRangesAndDropsTheHeaviestFirst() {
        val effects = PictureEffects(brightness = 400, hue = -500, denoise = 30, look = "neon").clamped()
        assertEquals(PictureEffects(brightness = 100, hue = -180, denoise = 30), effects)
        assertEquals(null, PictureEffects.from(null))
        assertEquals(listOf("denoise", "deflicker", "sharpen"), PictureEffects(sharpen = 5, deflicker = 5, denoise = 5).heavy)
        assertEquals(PictureEffects(sharpen = 5), PictureEffects(sharpen = 5, denoise = 5).without("denoise"))
        assertTrue(PictureEffects.NONE.neutral && !PictureEffects(look = "sepia").neutral)
        assertTrue(SmartAudioPrefs().off && !SmartAudioPrefs(night = true).off)
    }

    // ---- Smart audio ----

    private val rate = 48_000

    /** `seconds` of a stereo sine at `frequency`, `level` dBFS, the same on both sides unless `side`. */
    private fun sine(seconds: Double, frequency: Double, level: Double, side: Boolean = false): DoubleArray {
        val frames = (rate * seconds).toInt()
        val amplitude = dbToGain(level)
        return DoubleArray(frames * 2) { index ->
            val value = amplitude * sin(2 * PI * frequency * (index / 2) / rate)
            if (side && index % 2 == 1) -value else value
        }
    }

    private fun rms(samples: DoubleArray, from: Int = samples.size / 2): Double {
        var sum = 0.0
        for (index in from until samples.size) sum += samples[index] * samples[index]
        return sqrt(sum / (samples.size - from))
    }

    private fun run(prefs: SmartAudioPrefs, samples: DoubleArray, dsp: SmartAudioDsp = SmartAudioDsp(rate, 2)): DoubleArray {
        dsp.prefs = prefs
        val copy = samples.copyOf()
        // In buffers of 1024 frames, as the sink hands them over.
        var start = 0
        while (start < copy.size) {
            val end = minOf(copy.size, start + 2048)
            val part = copy.copyOfRange(start, end)
            dsp.process(part, part.size / 2)
            part.copyInto(copy, start)
            start = end
        }
        return copy
    }

    @Test
    fun offLeavesTheSoundAlone() {
        val sound = sine(0.5, 440.0, -12.0)
        assertArrayEquals(sound, run(SmartAudioPrefs(), sound), 1e-12)
    }

    @Test
    fun dialogueLiftsTheVoiceBandInTheMiddleAndLowersTheSides() {
        val voice = sine(1.0, DIALOGUE_FREQUENCY, -20.0)
        val lifted = 20 * log10(rms(run(SmartAudioPrefs(dialogue = true), voice)) / rms(voice))
        assertEquals(DIALOGUE_BOOST_DB, lifted, 0.5)
        val wide = sine(1.0, 200.0, -20.0, side = true)
        val sides = rms(run(SmartAudioPrefs(dialogue = true), wide)) / rms(wide)
        assertEquals(DIALOGUE_SIDE_GAIN, sides, 0.02)
    }

    @Test
    fun nightModeBringsLoudAndQuietCloser() {
        val quiet = sine(2.0, 300.0, -45.0)
        val loud = sine(2.0, 300.0, -6.0)
        val night = SmartAudioPrefs(night = true)
        val quietOut = 20 * log10(rms(run(night, quiet)))
        val loudOut = 20 * log10(rms(run(night, loud)))
        val before = 20 * log10(rms(loud) / rms(quiet))
        assertTrue("quiet parts get louder", quietOut > 20 * log10(rms(quiet)) + 5)
        assertTrue("loud parts get quieter", loudOut < 20 * log10(rms(loud)))
        assertTrue("the range shrinks", loudOut - quietOut < before - 15)
        assertTrue(nightReductionDb(NIGHT_THRESHOLD_DB - NIGHT_KNEE_DB) == 0.0)
        assertEquals((1 / NIGHT_RATIO - 1) * 20, nightReductionDb(NIGHT_THRESHOLD_DB + 20), 1e-9)
    }

    @Test
    fun evenLoudnessTurnsAQuietVideoUpWithinItsRange() {
        val dsp = SmartAudioDsp(rate, 2)
        run(SmartAudioPrefs(normalize = true), sine(20.0, 1000.0, -40.0), dsp)
        assertTrue("turned up: ${dsp.normalizeDb}", dsp.normalizeDb > 8)
        assertTrue(dsp.normalizeDb <= NORMALIZE_RANGE_DB)
        // A sine's power on two channels is its peak's (−0.691 dB of BS.1770), give or take K-weighting.
        assertEquals(-40.0, dsp.integrated!!, 1.5)
    }

    @Test
    fun levelingHoldsThroughSilenceAndLowersLoudScenes() {
        val dsp = SmartAudioDsp(rate, 2)
        run(SmartAudioPrefs(leveling = true), DoubleArray(rate * 2 * 5), dsp)
        assertEquals(0.0, dsp.levelDb, 0.0)
        run(SmartAudioPrefs(leveling = true), sine(10.0, 1000.0, -6.0), dsp)
        assertTrue("a loud scene comes down: ${dsp.levelDb}", dsp.levelDb < -3)
    }

    @Test
    fun theLimiterKeepsPeaksUnderOneDecibelBelowFullScale() {
        val dsp = SmartAudioDsp(rate, 2)
        val out = run(SmartAudioPrefs(normalize = true, dialogue = true), sine(15.0, 2000.0, -1.0), dsp)
        val ceiling = dbToGain(-1.0)
        // After the limiter's first sample of each rise.
        val peak = out.drop(rate).maxOf { abs(it) }
        assertTrue("peak $peak", peak <= ceiling + 1e-6)
    }

    @Test
    fun integratedLoudnessGatesSilenceOut() {
        val histogram = LoudnessHistogram()
        repeat(100) { histogram.add(1e-3) }
        repeat(100) { histogram.add(1e-12) }
        assertEquals(LoudnessHistogram.lufs(1e-3), histogram.integrated()!!, 0.1)
        assertEquals(10.0, histogram.seconds, 0.0)
    }
}

package app.raygun.audio

import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/** The loudness even loudness and scene leveling aim for, LUFS (the desktop player's too). */
internal const val SMART_TARGET_LUFS = -18.0

/** How far even loudness turns a video up or down, dB. */
internal const val NORMALIZE_RANGE_DB = 12.0

/** How far scene leveling turns a scene up or down, dB, and how much of the gap it closes. */
internal const val LEVEL_RANGE_DB = 8.0
internal const val LEVEL_SHARE = 0.7

/** Below this short-term loudness a scene counts as silence: leveling holds rather than lifts it. */
internal const val LEVEL_GATE_LUFS = -50.0

/** Night mode's compressor (soft knee) and the gain it adds back. */
internal const val NIGHT_THRESHOLD_DB = -30.0
internal const val NIGHT_RATIO = 3.5
internal const val NIGHT_KNEE_DB = 10.0
internal const val NIGHT_MAKEUP_DB = 8.0

/** Clearer dialogue: the voice band's lift, and how much quieter the sides (or other channels) get. */
internal const val DIALOGUE_BOOST_DB = 5.0
internal const val DIALOGUE_FREQUENCY = 2200.0
internal const val DIALOGUE_SIDE_GAIN = 0.7

/** The limiter's ceiling: −1 dBFS. */
private val CEILING = 10.0.pow(-1.0 / 20)

internal fun dbToGain(db: Double): Double = 10.0.pow(db / 20)

/** Night mode's gain change for a level, dB (0 or less): a soft-knee downward compressor. */
internal fun nightReductionDb(levelDb: Double): Double {
    val over = levelDb - NIGHT_THRESHOLD_DB
    val slope = 1 / NIGHT_RATIO - 1
    return when {
        2 * over < -NIGHT_KNEE_DB -> 0.0
        2 * abs(over) <= NIGHT_KNEE_DB -> slope * (over + NIGHT_KNEE_DB / 2).pow(2) / (2 * NIGHT_KNEE_DB)
        else -> slope * over
    }
}

/** `current` moved toward `wanted` by at most `step`. */
internal fun approach(current: Double, wanted: Double, step: Double): Double = when {
    wanted > current -> minOf(wanted, current + step)
    else -> maxOf(wanted, current - step)
}

/** An RBJ-cookbook biquad, transposed direct form II, one state per channel. */
private class SmartBiquad(channels: Int) {
    private var b0 = 1.0
    private var b1 = 0.0
    private var b2 = 0.0
    private var a1 = 0.0
    private var a2 = 0.0
    private val z1 = DoubleArray(channels)
    private val z2 = DoubleArray(channels)

    fun process(channel: Int, x: Double): Double {
        val y = b0 * x + z1[channel]
        z1[channel] = b1 * x - a1 * y + z2[channel]
        z2[channel] = b2 * x - a2 * y
        return y
    }

    fun clear() {
        z1.fill(0.0)
        z2.fill(0.0)
    }

    private fun set(nb0: Double, nb1: Double, nb2: Double, na0: Double, na1: Double, na2: Double): SmartBiquad {
        b0 = nb0 / na0
        b1 = nb1 / na0
        b2 = nb2 / na0
        a1 = na1 / na0
        a2 = na2 / na0
        return this
    }

    fun peaking(frequency: Double, q: Double, db: Double, rate: Int): SmartBiquad {
        val a = 10.0.pow(db / 40)
        val w = 2 * PI * frequency / rate
        val alpha = sin(w) / (2 * q)
        return set(1 + alpha * a, -2 * cos(w), 1 - alpha * a, 1 + alpha / a, -2 * cos(w), 1 - alpha / a)
    }

    fun highShelf(frequency: Double, db: Double, rate: Int): SmartBiquad {
        val a = 10.0.pow(db / 40)
        val w = 2 * PI * frequency / rate
        val cosW = cos(w)
        val beta = sqrt(a) * sin(w) * sqrt(2.0)
        return set(
            a * ((a + 1) + (a - 1) * cosW + beta),
            -2 * a * ((a - 1) + (a + 1) * cosW),
            a * ((a + 1) + (a - 1) * cosW - beta),
            (a + 1) - (a - 1) * cosW + beta,
            2 * ((a - 1) - (a + 1) * cosW),
            (a + 1) - (a - 1) * cosW - beta,
        )
    }

    fun highPass(frequency: Double, q: Double, rate: Int): SmartBiquad {
        val w = 2 * PI * frequency / rate
        val cosW = cos(w)
        val alpha = sin(w) / (2 * q)
        return set((1 + cosW) / 2, -(1 + cosW), (1 + cosW) / 2, 1 + alpha, -2 * cosW, 1 - alpha)
    }
}

/**
 * BS.1770's integrated loudness over everything heard, in bounded memory: 400 ms blocks counted in
 * 0.1 LU bins (with their power), gated at −70 LUFS and 10 LU under the mean.
 */
internal class LoudnessHistogram {
    private val counts = IntArray(BINS)
    private val powers = DoubleArray(BINS)
    private var blocks = 0

    /** Seconds of audible sound counted (blocks are 100 ms apart). */
    val seconds: Double get() = blocks / 10.0

    fun add(power: Double) {
        val loudness = lufs(power)
        if (loudness <= FLOOR) return
        val bin = ((loudness - FLOOR) * 10).toInt().coerceIn(0, BINS - 1)
        counts[bin] += 1
        powers[bin] += power
        blocks += 1
    }

    fun integrated(): Double? {
        if (blocks == 0) return null
        var total = 0.0
        for (bin in 0 until BINS) total += powers[bin]
        val gate = lufs(total / blocks) - 10
        var kept = 0.0
        var count = 0
        for (bin in 0 until BINS) {
            // A bin's lower edge against the gate: whole bins in or out, 0.1 LU apart.
            if (FLOOR + bin / 10.0 >= gate) {
                kept += powers[bin]
                count += counts[bin]
            }
        }
        return if (count == 0) lufs(total / blocks) else lufs(kept / count)
    }

    fun clear() {
        counts.fill(0)
        powers.fill(0.0)
        blocks = 0
    }

    companion object {
        private const val FLOOR = -70.0
        private const val BINS = 800

        fun lufs(power: Double): Double = -0.691 + 10 * log10(max(power, 1e-12))
    }
}

/**
 * Smart audio's processing, on interleaved samples (−1 to 1): clearer dialogue, then night mode's
 * compressor, then the gains of even loudness and scene leveling (from the sound's K-weighted
 * loudness after the compressor), then a limiter at −1 dBFS. The same chain as the desktop
 * player's Web Audio graph (`video/effects/smartAudio.ts`).
 */
internal class SmartAudioDsp(val rate: Int, val channels: Int) {
    @Volatile
    var prefs = SmartAudioPrefs()

    private val presence = SmartBiquad(channels).peaking(DIALOGUE_FREQUENCY, 0.8, DIALOGUE_BOOST_DB, rate)
    /** 5.1 and up: the centre channel (where speech is mixed) a little louder still. */
    private val centreGain = dbToGain(2.0)
    private val shelfK = SmartBiquad(channels).highShelf(1682.0, 4.0, rate)
    private val highK = SmartBiquad(channels).highPass(38.0, 0.5, rate)
    /** BS.1770's channel weights: the LFE (the 4th of 5.1 and 7.1) left out. */
    private val weights = DoubleArray(channels) { channel -> if (channels >= 6 && channel == 3) 0.0 else 1.0 }

    // Night mode: the power's envelope, and the gain it sets (smoothed per sample).
    private val attack = 1 - exp(-1.0 / (rate * 0.005))
    private val release = 1 - exp(-1.0 / (rate * 0.25))
    private val glide = 1 - exp(-1.0 / (rate * 0.05))
    private var envelope = 0.0
    private var nightTarget = 1.0
    private var nightGain = 1.0
    private var sinceComputed = 0

    // Loudness, in 100 ms steps.
    private val stepLength = max(1, rate / 10)
    private var stepPower = 0.0
    private var stepFrames = 0
    private val recentSteps = ArrayDeque<Double>()
    private val history = LoudnessHistogram()

    /** Even loudness's and leveling's gains, dB; their sum glides per sample. */
    var normalizeDb = 0.0
        private set
    var levelDb = 0.0
        private set
    private var gain = 1.0

    private val limitRelease = exp(-1.0 / (rate * 0.1))
    private var limiter = 0.0

    /** Short-term loudness (the last 3 s), LUFS; null before a second of sound. */
    var shortTerm: Double? = null
        private set

    val integrated: Double? get() = history.integrated()

    /** A seek: filters and envelopes start again; what was measured of the video stays. */
    fun restart() {
        presence.clear()
        shelfK.clear()
        highK.clear()
        envelope = 0.0
        limiter = 0.0
        stepPower = 0.0
        stepFrames = 0
        recentSteps.clear()
    }

    /** Another video: its loudness is measured afresh. */
    fun newProgram() {
        restart()
        history.clear()
        shortTerm = null
    }

    /** Processes `frames` frames of `samples` in place. */
    fun process(samples: DoubleArray, frames: Int) {
        val current = prefs
        val measuring = current.normalize || current.leveling
        var index = 0
        for (frame in 0 until frames) {
            if (current.dialogue) dialogue(samples, index)
            if (current.night) {
                var power = 0.0
                for (channel in 0 until channels) {
                    val x = samples[index + channel]
                    power = max(power, x * x)
                }
                envelope += (if (power > envelope) attack else release) * (power - envelope)
                if (++sinceComputed >= 32) {
                    sinceComputed = 0
                    val levelDb = 10 * log10(max(envelope, 1e-12))
                    nightTarget = dbToGain(nightReductionDb(levelDb) + NIGHT_MAKEUP_DB)
                }
                nightGain += (nightTarget - nightGain) * glide
                for (channel in 0 until channels) samples[index + channel] *= nightGain
            } else {
                nightGain = 1.0
            }
            if (measuring) measure(samples, index)
            val wanted = dbToGainCached(normalizeDb + levelDb)
            gain += (wanted - gain) * glide
            var peak = 0.0
            for (channel in 0 until channels) {
                val y = samples[index + channel] * gain
                samples[index + channel] = y
                peak = max(peak, abs(y))
            }
            limiter = if (peak > limiter) peak else limiter * limitRelease
            val limit = if (limiter > CEILING) CEILING / limiter else 1.0
            for (channel in 0 until channels) {
                val y = samples[index + channel] * limit
                samples[index + channel] = if (y.isFinite()) y.coerceIn(-1.0, 1.0) else 0.0
            }
            index += channels
        }
        if (!measuring) {
            // Off: the gains glide back to none.
            normalizeDb = approach(normalizeDb, 0.0, 3.0 * frames / rate)
            levelDb = approach(levelDb, 0.0, 3.0 * frames / rate)
        }
    }

    private var cachedDb = 0.0
    private var cachedGain = 1.0

    private fun dbToGainCached(db: Double): Double {
        if (db != cachedDb) {
            cachedDb = db
            cachedGain = dbToGain(db)
        }
        return cachedGain
    }

    /** The voice band lifted on what's in the middle (or the centre channel), the rest lowered. */
    private fun dialogue(samples: DoubleArray, index: Int) {
        when {
            channels == 1 -> samples[index] = presence.process(0, samples[index])
            channels == 2 -> {
                val left = samples[index]
                val right = samples[index + 1]
                val mid = presence.process(0, (left + right) / 2)
                val side = (left - right) / 2 * DIALOGUE_SIDE_GAIN
                samples[index] = mid + side
                samples[index + 1] = mid - side
            }
            else -> {
                // 5.1 and up: front left, front right, centre, LFE, then the rest.
                for (channel in 0 until channels) {
                    samples[index + channel] = if (channel == 2) {
                        presence.process(0, samples[index + channel]) * centreGain
                    } else {
                        samples[index + channel] * DIALOGUE_SIDE_GAIN
                    }
                }
            }
        }
    }

    private fun measure(samples: DoubleArray, index: Int) {
        for (channel in 0 until channels) {
            val weighted = highK.process(channel, shelfK.process(channel, samples[index + channel]))
            stepPower += weighted * weighted * weights[channel]
        }
        stepFrames += 1
        if (stepFrames >= stepLength) {
            step(stepPower / stepFrames)
            stepPower = 0.0
            stepFrames = 0
        }
    }

    /** Every 100 ms: the loudness so far and the gains that follow from it. */
    private fun step(power: Double) {
        recentSteps.addLast(power)
        if (recentSteps.size > 30) recentSteps.removeFirst()
        if (recentSteps.size >= 4) {
            var block = 0.0
            for (offset in 1..4) block += recentSteps[recentSteps.size - offset]
            history.add(block / 4)
        }
        val shortTermNow = if (recentSteps.size >= 10) LoudnessHistogram.lufs(recentSteps.average()) else null
        shortTerm = shortTermNow
        val current = prefs
        normalizeDb = if (current.normalize) {
            val loudness = history.integrated()
            if (loudness != null && history.seconds >= 3) {
                val wanted = (SMART_TARGET_LUFS - loudness).coerceIn(-NORMALIZE_RANGE_DB, NORMALIZE_RANGE_DB)
                // Quick at first, then slow: a video's level settles, a scene's doesn't move it.
                val perSecond = if (history.seconds < 15) 3.0 else 0.75
                approach(normalizeDb, wanted, perSecond / 10)
            } else {
                normalizeDb
            }
        } else {
            approach(normalizeDb, 0.0, 0.3)
        }
        levelDb = if (current.leveling) {
            if (shortTermNow != null && shortTermNow > LEVEL_GATE_LUFS) {
                val heard = shortTermNow + normalizeDb
                val wanted = ((SMART_TARGET_LUFS - heard) * LEVEL_SHARE).coerceIn(-LEVEL_RANGE_DB, LEVEL_RANGE_DB)
                // Down quickly (a loud scene), up slowly (no pumping up of quiet ones).
                approach(levelDb, wanted, if (wanted < levelDb) 0.4 else 0.15)
            } else {
                levelDb
            }
        } else {
            approach(levelDb, 0.0, 0.3)
        }
    }
}

/**
 * Smart audio in the video player's audio sink ([PlayerRenderers]). Always in the chain (the
 * sink only picks its processors when a stream starts), passing the sound through untouched while
 * smart audio is off; switches apply from the next buffer.
 */
@UnstableApi
internal class SmartAudioProcessor : BaseAudioProcessor() {
    @Volatile
    private var prefs = SmartAudioPrefs()

    @Volatile
    private var programChanged = false
    private var dsp: SmartAudioDsp? = null
    private var samples = DoubleArray(0)

    fun setPrefs(next: SmartAudioPrefs) {
        prefs = next
    }

    /** Another video plays: its loudness is measured afresh. */
    fun newProgram() {
        programChanged = true
    }

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT && inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val bytes = inputBuffer.remaining()
        if (bytes == 0) return
        val format = inputAudioFormat
        val current = prefs
        val engine = engineFor(format)
        if (current.off && engine.normalizeDb == 0.0 && engine.levelDb == 0.0) {
            val output = replaceOutputBuffer(bytes)
            output.put(inputBuffer)
            output.flip()
            return
        }
        engine.prefs = current
        if (programChanged) {
            programChanged = false
            engine.newProgram()
        }
        val float = format.encoding == C.ENCODING_PCM_FLOAT
        val sampleSize = if (float) 4 else 2
        val frames = bytes / (sampleSize * format.channelCount)
        val count = frames * format.channelCount
        if (samples.size < count) samples = DoubleArray(count)
        val input = inputBuffer.order(ByteOrder.nativeOrder())
        for (index in 0 until count) {
            samples[index] = if (float) input.float.toDouble() else input.short / 32768.0
        }
        engine.process(samples, frames)
        val output = replaceOutputBuffer(count * sampleSize).order(ByteOrder.nativeOrder())
        for (index in 0 until count) {
            if (float) output.putFloat(samples[index].toFloat()) else output.putShort((samples[index] * 32767).toInt().toShort())
        }
        // Any trailing partial frame is dropped.
        inputBuffer.position(inputBuffer.limit())
        output.flip()
    }

    private fun engineFor(format: AudioProcessor.AudioFormat): SmartAudioDsp {
        val existing = dsp
        if (existing != null && existing.rate == format.sampleRate && existing.channels == format.channelCount) return existing
        return SmartAudioDsp(format.sampleRate, format.channelCount).also { dsp = it }
    }

    override fun onFlush(streamMetadata: AudioProcessor.StreamMetadata) {
        dsp?.restart()
    }

    override fun onReset() {
        dsp = null
    }
}

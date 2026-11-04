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
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/** Centre frequencies of the equalizer's bands, Hz; the same as `EQ_FREQUENCIES` in Rust. */
val EQ_FREQUENCIES = doubleArrayOf(31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0)

/** The sound settings every player follows (M46.12); set from Rust's `SetEffects`. */
data class SoundConfig(
    val eqEnabled: Boolean = false,
    /** Band gains in dB. */
    val bands: DoubleArray = DoubleArray(EQ_FREQUENCIES.size),
    val preampDb: Double = 0.0,
    /** dB at the bass shelf. */
    val bassDb: Double = 0.0,
    /** −1 (left) to 1 (right). */
    val balance: Double = 0.0,
    val mono: Boolean = false,
    val normalize: Boolean = false,
    val targetLufs: Double = -14.0,
) {
    /** RiPlay's automatic headroom: the biggest boost comes off before the equalizer. */
    val headroomDb: Double
        get() = max(if (eqEnabled) max(0.0, bands.maxOrNull() ?: 0.0) else 0.0, bassDb)
}

/** One RBJ-cookbook biquad, transposed direct form II, one per channel. */
private class Biquad(channels: Int) {
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

    private fun set(nb0: Double, nb1: Double, nb2: Double, na0: Double, na1: Double, na2: Double) {
        b0 = nb0 / na0
        b1 = nb1 / na0
        b2 = nb2 / na0
        a1 = na1 / na0
        a2 = na2 / na0
    }

    fun peaking(frequency: Double, q: Double, db: Double, rate: Int) {
        val a = 10.0.pow(db / 40)
        val w = 2 * PI * frequency / rate
        val alpha = sin(w) / (2 * q)
        set(1 + alpha * a, -2 * cos(w), 1 - alpha * a, 1 + alpha / a, -2 * cos(w), 1 - alpha / a)
    }

    fun lowShelf(frequency: Double, db: Double, rate: Int) {
        val a = 10.0.pow(db / 40)
        val w = 2 * PI * frequency / rate
        val cosW = cos(w)
        val beta = 2 * sqrt(a) * sin(w) / 2 * sqrt(2.0)
        set(
            a * ((a + 1) - (a - 1) * cosW + beta),
            2 * a * ((a - 1) - (a + 1) * cosW),
            a * ((a + 1) - (a - 1) * cosW - beta),
            (a + 1) + (a - 1) * cosW + beta,
            -2 * ((a - 1) + (a + 1) * cosW),
            (a + 1) + (a - 1) * cosW - beta,
        )
    }

    fun highShelf(frequency: Double, db: Double, rate: Int) {
        val a = 10.0.pow(db / 40)
        val w = 2 * PI * frequency / rate
        val cosW = cos(w)
        val beta = 2 * sqrt(a) * sin(w) / 2 * sqrt(2.0)
        set(
            a * ((a + 1) + (a - 1) * cosW + beta),
            -2 * a * ((a - 1) + (a + 1) * cosW),
            a * ((a + 1) + (a - 1) * cosW - beta),
            (a + 1) - (a - 1) * cosW + beta,
            2 * ((a - 1) - (a + 1) * cosW),
            (a + 1) - (a - 1) * cosW - beta,
        )
    }

    fun highPass(frequency: Double, q: Double, rate: Int) {
        val w = 2 * PI * frequency / rate
        val cosW = cos(w)
        val alpha = sin(w) / (2 * q)
        set((1 + cosW) / 2, -(1 + cosW), (1 + cosW) / 2, 1 + alpha, -2 * cosW, 1 - alpha)
    }
}

/** BS.1770 loudness: 400 ms blocks of 100 ms steps, gated at −70 LUFS and 10 LU under the mean. */
class LoudnessMeter {
    private val steps = ArrayDeque<Double>()
    private val blocks = ArrayList<Double>()

    fun add(power: Double) {
        steps.addLast(power)
        if (steps.size > 4) steps.removeFirst()
        if (steps.size == 4) blocks.add(steps.sum() / 4)
    }

    val seconds: Double get() = blocks.size / 10.0

    fun integrated(): Double? {
        val audible = blocks.filter { lufs(it) > -70 }
        if (audible.isEmpty()) return null
        val gate = lufs(audible.average()) - 10
        val kept = audible.filter { lufs(it) > gate }
        return lufs((kept.ifEmpty { audible }).average())
    }

    companion object {
        fun lufs(power: Double) = -0.691 + 10 * log10(max(power, 1e-12))
    }
}

/**
 * Raygun's sound on Android (M46.12): the same chain as the desktop's Web Audio graph, on the PCM
 * ExoPlayer decodes. The preamp (less RiPlay's automatic headroom), ten biquads (a low shelf, peaks
 * with Q = 2f / (f₊ − f₋), a high shelf), the bass shelf, balance and mono, the song's own gain
 * with fades, then a peak limiter at −1 dBFS. The fixed bands sound the same on every phone, which
 * the platform equalizer's (whatever the device offers) don't. It also measures the song's
 * K-weighted loudness for normalization. Each player has its own, so a crossfade's two songs keep
 * their own gains and fades.
 */
@UnstableApi
class SoundProcessor : BaseAudioProcessor() {
    @Volatile private var config = SoundConfig()
    private var applied: SoundConfig? = null
    private var rate = 44100
    private var channels = 2
    private var encoding = C.ENCODING_PCM_16BIT

    private var bands = emptyArray<Biquad>()
    private var bass = Biquad(2)
    private var shelfK = Biquad(2)
    private var highK = Biquad(2)
    private var preamp = 1.0

    // The song's own gain, glided toward its target; the fade from one value to another.
    @Volatile private var targetGain = 1.0
    /** A new song starts at its gain instead of gliding there. */
    @Volatile private var jumpGain = false
    private var gain = 1.0
    private var fadeFrom = 1.0
    private var fadeTo = 1.0
    private var fadeTotal = 0L
    private var fadeDone = 0L
    @Volatile private var pendingFade: Pair<Double, Long>? = null

    private var limiterEnvelope = 0.0

    // Loudness: sums over the current 100 ms step.
    @Volatile private var measuring = false
    @Volatile private var meter = LoudnessMeter()
    private var stepSquares = 0.0
    private var stepFrames = 0
    @Volatile var onMeasured: ((LoudnessMeter) -> Unit)? = null

    fun setConfig(next: SoundConfig) {
        config = next
    }

    /** A new song on this player: its gain in dB, a fade in over `fadeInMs` (0 = none), a meter. */
    fun startTrack(gainDb: Double, fadeInMs: Long, measure: Boolean) {
        targetGain = 10.0.pow(gainDb / 20)
        jumpGain = true
        meter = LoudnessMeter()
        measuring = measure
        pendingFade = if (fadeInMs > 0) -1.0 to fadeInMs else 1.0 to 0L
    }

    fun setGainDb(gainDb: Double) {
        targetGain = 10.0.pow(gainDb / 20)
    }

    /** Fades to `level` (0–1) over `ms`, on an equal-power curve. */
    fun fade(level: Double, ms: Long) {
        pendingFade = level to ms
    }

    fun stopMeasuring() {
        measuring = false
    }

    val isMeasuring: Boolean get() = measuring

    val loudness: LoudnessMeter get() = meter

    /**
     * Only checks the format: the sink configures its processors for the next song while the last
     * one's sound is still draining through them, so the new format applies at the next flush
     * ([onFlush]), when [inputAudioFormat] becomes it.
     */
    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT && inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        return inputAudioFormat
    }

    /** Sets the filters up for the format in use (the one configured last, from its flush on). */
    private fun useFormat(format: AudioProcessor.AudioFormat) {
        if (format != AudioProcessor.AudioFormat.NOT_SET) {
            if (format.sampleRate == rate && format.channelCount == channels && format.encoding == encoding &&
                bands.isNotEmpty()
            ) {
                return
            }
            rate = format.sampleRate
            channels = format.channelCount
            encoding = format.encoding
        } else if (bands.isNotEmpty()) {
            return
        }
        bands = Array(EQ_FREQUENCIES.size) { Biquad(channels) }
        bass = Biquad(channels)
        shelfK = Biquad(channels).apply { highShelf(1682.0, 4.0, rate) }
        highK = Biquad(channels).apply { highPass(38.0, 0.5, rate) }
        applied = null
    }

    private fun clearFilters() {
        for (band in bands) band.clear()
        bass.clear()
        shelfK.clear()
        highK.clear()
    }

    private fun applyConfig() {
        val current = config
        if (current === applied) return
        applied = current
        val nyquistSafe = rate * 0.45
        EQ_FREQUENCIES.forEachIndexed { index, frequency ->
            val db = if (current.eqEnabled) current.bands[index] else 0.0
            val f = min(frequency, nyquistSafe)
            when (index) {
                0 -> bands[index].lowShelf(f, db, rate)
                EQ_FREQUENCIES.size - 1 -> bands[index].highShelf(f, db, rate)
                else -> {
                    val q = 2 * frequency / abs(EQ_FREQUENCIES[index + 1] - EQ_FREQUENCIES[index - 1])
                    bands[index].peaking(f, q, db, rate)
                }
            }
        }
        bass.lowShelf(100.0, current.bassDb, rate)
        val preampDb = (if (current.eqEnabled) current.preampDb else 0.0) - current.headroomDb
        preamp = 10.0.pow(preampDb / 20)
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val bytes = inputBuffer.remaining()
        if (bytes == 0) return
        if (bands.isEmpty()) useFormat(inputAudioFormat)
        applyConfig()
        val current = applied ?: config
        takeFade()
        if (jumpGain) {
            jumpGain = false
            gain = targetGain
        }
        val sampleSize = if (encoding == C.ENCODING_PCM_FLOAT) 4 else 2
        val frames = bytes / (sampleSize * channels)
        val output = replaceOutputBuffer(frames * sampleSize * channels).order(ByteOrder.nativeOrder())
        val input = inputBuffer.order(ByteOrder.nativeOrder())
        val frame = DoubleArray(channels)
        val eq = current.eqEnabled
        val bassOn = current.bassDb != 0.0
        val leftGain = if (current.balance > 0) 1 - current.balance else 1.0
        val rightGain = if (current.balance < 0) 1 + current.balance else 1.0
        val glide = 1 - Math.exp(-1.0 / (rate * 0.05))
        val release = Math.exp(-1.0 / (rate * 0.15))
        val threshold = 10.0.pow(-1.0 / 20)
        val stepLength = rate / 10
        for (index in 0 until frames) {
            for (channel in 0 until channels) {
                frame[channel] = if (sampleSize == 4) {
                    input.float.toDouble()
                } else {
                    input.short / 32768.0
                }
            }
            if (measuring) {
                for (channel in 0 until channels) {
                    val weighted = highK.process(channel, shelfK.process(channel, frame[channel]))
                    stepSquares += weighted * weighted
                }
                stepFrames += 1
                if (stepFrames >= stepLength) {
                    meter.add(stepSquares / stepFrames)
                    stepSquares = 0.0
                    stepFrames = 0
                    onMeasured?.invoke(meter)
                }
            }
            gain += (targetGain - gain) * glide
            if (!gain.isFinite()) gain = targetGain.takeIf { it.isFinite() } ?: 1.0
            val level = gain * fadeLevel()
            var peak = 0.0
            for (channel in 0 until channels) {
                var x = frame[channel] * preamp
                if (eq) for (band in bands) x = band.process(channel, x)
                if (bassOn) x = bass.process(channel, x)
                if (!x.isFinite()) {
                    // One bad sample would stay in the filters' memory and silence everything
                    // after it, until the next seek flushed them.
                    clearFilters()
                    x = 0.0
                }
                frame[channel] = x * level
            }
            if (channels == 2) {
                if (current.mono) {
                    val mid = (frame[0] + frame[1]) / 2
                    frame[0] = mid
                    frame[1] = mid
                }
                frame[0] *= leftGain
                frame[1] *= rightGain
            }
            for (channel in 0 until channels) peak = max(peak, abs(frame[channel]))
            if (!limiterEnvelope.isFinite()) limiterEnvelope = 0.0
            limiterEnvelope = if (peak > limiterEnvelope) peak else limiterEnvelope * release
            val limit = if (limiterEnvelope > threshold) threshold / limiterEnvelope else 1.0
            for (channel in 0 until channels) {
                val y = (frame[channel] * limit).coerceIn(-1.0, 1.0)
                if (sampleSize == 4) {
                    output.putFloat(y.toFloat())
                } else {
                    output.putShort((y * 32767).toInt().toShort())
                }
            }
        }
        // Any trailing partial frame is dropped.
        inputBuffer.position(inputBuffer.limit())
        output.flip()
    }

    /** Takes a fade asked for from another thread; `-1` as the level means "from silence up". */
    private fun takeFade() {
        val request = pendingFade ?: return
        pendingFade = null
        val (level, ms) = request
        val current = fadeLevel()
        if (level < 0) {
            fadeFrom = 0.0
            fadeTo = 1.0
        } else {
            fadeFrom = current
            fadeTo = level
        }
        fadeTotal = ms * rate / 1000
        fadeDone = 0
        if (fadeTotal <= 0) fadeFrom = fadeTo
    }

    private fun fadeLevel(): Double {
        if (fadeDone >= fadeTotal) return fadeTo
        val progress = fadeDone.toDouble() / fadeTotal
        fadeDone += 1
        return if (fadeTo >= fadeFrom) {
            fadeFrom + (fadeTo - fadeFrom) * sin(progress * PI / 2)
        } else {
            fadeTo + (fadeFrom - fadeTo) * cos(progress * PI / 2)
        }
    }

    override fun onFlush() {
        // The format configured last applies from here on.
        useFormat(inputAudioFormat)
        clearFilters()
        stepSquares = 0.0
        stepFrames = 0
    }

    override fun onReset() {
        applied = null
        limiterEnvelope = 0.0
        bands = emptyArray()
    }
}

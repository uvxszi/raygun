package app.raygun.audio

import org.json.JSONObject
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.sin

/**
 * The picture's effects (`PictureEffects` in raygun-model): signed adjustments from −100 to 100,
 * the hue in degrees (−180 to 180), strengths from 0 to 100 and a look. All 0 leaves the picture
 * as it is. The values map onto the shared shaders' uniforms the same way as the desktop player's
 * (`video/effects/effectParams.ts`); the tests of both check the same numbers.
 */
internal data class PictureEffects(
    val brightness: Int = 0,
    val contrast: Int = 0,
    val saturation: Int = 0,
    val gamma: Int = 0,
    val hue: Int = 0,
    val sharpen: Int = 0,
    val look: String = LOOK_NONE,
    val deflicker: Int = 0,
    val denoise: Int = 0,
) {
    val neutral: Boolean get() = this == NONE

    /** The parts that take passes of their own or a frame's history: what a slow phone drops first. */
    val heavy: List<String>
        get() = listOfNotNull(
            "denoise".takeIf { denoise > 0 },
            "deflicker".takeIf { deflicker > 0 },
            "sharpen".takeIf { sharpen > 0 },
        )

    /** Each value kept to its range, an unknown look as none. */
    fun clamped(): PictureEffects = PictureEffects(
        brightness = brightness.coerceIn(-100, 100),
        contrast = contrast.coerceIn(-100, 100),
        saturation = saturation.coerceIn(-100, 100),
        gamma = gamma.coerceIn(-100, 100),
        hue = hue.coerceIn(-180, 180),
        sharpen = sharpen.coerceIn(0, 100),
        look = look.takeIf { it in LOOKS } ?: LOOK_NONE,
        deflicker = deflicker.coerceIn(0, 100),
        denoise = denoise.coerceIn(0, 100),
    )

    /** These effects without `kind` (one of [heavy]). */
    fun without(kind: String): PictureEffects = when (kind) {
        "denoise" -> copy(denoise = 0)
        "deflicker" -> copy(deflicker = 0)
        "sharpen" -> copy(sharpen = 0)
        else -> this
    }

    fun toJson(): JSONObject = JSONObject()
        .put("brightness", brightness)
        .put("contrast", contrast)
        .put("saturation", saturation)
        .put("gamma", gamma)
        .put("hue", hue)
        .put("sharpen", sharpen)
        .put("look", look)
        .put("deflicker", deflicker)
        .put("denoise", denoise)

    companion object {
        const val LOOK_NONE = "none"
        val NONE = PictureEffects()
        val LOOKS = listOf(LOOK_NONE, "warm", "cool", "vivid", "grayscale", "sepia")

        /** Read from Rust's JSON, each value kept to its range; null when there's none. */
        fun from(json: JSONObject?): PictureEffects? {
            json ?: return null
            return PictureEffects(
                brightness = json.optInt("brightness", 0),
                contrast = json.optInt("contrast", 0),
                saturation = json.optInt("saturation", 0),
                gamma = json.optInt("gamma", 0),
                hue = json.optInt("hue", 0),
                sharpen = json.optInt("sharpen", 0),
                look = json.optString("look", LOOK_NONE),
                deflicker = json.optInt("deflicker", 0),
                denoise = json.optInt("denoise", 0),
            ).clamped()
        }
    }
}

/** Smart audio (`SmartAudio` in raygun-model), each on or off. */
internal data class SmartAudioPrefs(
    val dialogue: Boolean = false,
    val night: Boolean = false,
    val normalize: Boolean = false,
    val leveling: Boolean = false,
) {
    val off: Boolean get() = this == SmartAudioPrefs()

    fun toJson(): JSONObject = JSONObject()
        .put("dialogue", dialogue)
        .put("night", night)
        .put("normalize", normalize)
        .put("leveling", leveling)

    companion object {
        fun from(json: JSONObject?): SmartAudioPrefs? {
            json ?: return null
            return SmartAudioPrefs(
                dialogue = json.optBoolean("dialogue"),
                night = json.optBoolean("night"),
                normalize = json.optBoolean("normalize"),
                leveling = json.optBoolean("leveling"),
            )
        }
    }
}

/** An affine colour change: out = matrix · rgb + offset, the matrix row by row. */
internal class ColourTransform(val matrix: DoubleArray, val offset: DoubleArray) {
    /** This change after `first`. */
    fun after(first: ColourTransform): ColourTransform {
        val product = DoubleArray(9)
        for (row in 0 until 3) {
            for (column in 0 until 3) {
                var sum = 0.0
                for (k in 0 until 3) sum += matrix[row * 3 + k] * first.matrix[k * 3 + column]
                product[row * 3 + column] = sum
            }
        }
        val moved = DoubleArray(3) { row ->
            var sum = offset[row]
            for (k in 0 until 3) sum += matrix[row * 3 + k] * first.offset[k]
            sum
        }
        return ColourTransform(product, moved)
    }

    /** The matrix column by column, as GLSL's `mat3` takes it. */
    fun columns(): FloatArray = FloatArray(9) { index -> matrix[(index % 3) * 3 + index / 3].toFloat() }

    companion object {
        val IDENTITY = ColourTransform(doubleArrayOf(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0), DoubleArray(3))

        fun scale(red: Double, green: Double, blue: Double) =
            ColourTransform(doubleArrayOf(red, 0.0, 0.0, 0.0, green, 0.0, 0.0, 0.0, blue), DoubleArray(3))
    }
}

/** The shaders' uniforms for a set of effects (see `glsl/fx_*.glsl`). */
internal class ShaderParams(
    val colour: ColourTransform,
    /** The gamma exponent; 1 leaves it. */
    val gamma: Float,
    /** The unsharp mask's amount; 0 leaves it. */
    val sharpen: Float,
    val deflicker: Boolean,
    /** How much of each frame the steady light takes. */
    val follow: Float,
    val denoise: Boolean,
    val spatial: Float,
    val spatialSigma: Float,
    val temporal: Float,
    val temporalSigma: Float,
) {
    /** The passes these need, for logs: "stats+clean+final". */
    val passes: String
        get() = listOfNotNull("stats".takeIf { deflicker }, "clean".takeIf { denoise }, "final").joinToString("+")
}

/** How the values map onto the shaders, the same as `effectParams.ts` on desktop. */
internal object EffectMath {
    /** Rec. 709's luma weights, as the shaders and CSS's filters use them (rounded as CSS does). */
    private const val LR = 0.2126
    private const val LG = 0.7152
    private const val LB = 0.0722

    /** Brightness adds up to a quarter of full scale either way. */
    fun brightnessOffset(value: Int): Double = value / 100.0 * 0.25

    /** Contrast halves (−100) or doubles (100) the distance from mid-grey. */
    fun contrastFactor(value: Int): Double = 2.0.pow(value / 100.0)

    /** Saturation from none (−100, grey) to twice (100). */
    fun saturationFactor(value: Int): Double = 1.0 + value / 100.0

    /** Gamma's exponent: 0.5 at 100 (lighter midtones) to 2 at −100. */
    fun gammaExponent(value: Int): Double = 2.0.pow(-value / 100.0)

    fun sharpenAmount(value: Int): Double = value / 100.0 * 1.6

    /** The deflicker's moving mean: a stronger setting follows each frame less (more frames). */
    fun deflickerFollow(value: Int): Double = 0.4 - 0.35 * (value / 100.0)

    /** CSS's `saturate()` matrix. */
    fun saturation(factor: Double): ColourTransform {
        val s = factor
        return ColourTransform(
            doubleArrayOf(
                0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s,
                0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s,
                0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s,
            ),
            DoubleArray(3),
        )
    }

    /** CSS's `hue-rotate()` matrix. */
    fun hueRotation(degrees: Int): ColourTransform {
        if (degrees == 0) return ColourTransform.IDENTITY
        val angle = degrees * PI / 180
        val c = cos(angle)
        val s = sin(angle)
        return ColourTransform(
            doubleArrayOf(
                0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928,
                0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283,
                0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072,
            ),
            DoubleArray(3),
        )
    }

    /** Contrast round mid-grey. */
    fun contrast(factor: Double): ColourTransform =
        ColourTransform(
            doubleArrayOf(factor, 0.0, 0.0, 0.0, factor, 0.0, 0.0, 0.0, factor),
            DoubleArray(3) { (1 - factor) * 0.5 },
        )

    fun look(name: String): ColourTransform = when (name) {
        "warm" -> ColourTransform.scale(1.07, 1.0, 0.89)
        "cool" -> ColourTransform.scale(0.92, 1.0, 1.09)
        "vivid" -> contrast(1.08).after(saturation(1.35))
        "grayscale" -> ColourTransform(
            doubleArrayOf(LR, LG, LB, LR, LG, LB, LR, LG, LB),
            DoubleArray(3),
        )
        // CSS's `sepia(1)`.
        "sepia" -> ColourTransform(
            doubleArrayOf(0.393, 0.769, 0.189, 0.349, 0.686, 0.168, 0.272, 0.534, 0.131),
            DoubleArray(3),
        )
        else -> ColourTransform.IDENTITY
    }

    /** The colour part, in order: the look, saturation, hue, contrast, then brightness. */
    fun colour(effects: PictureEffects): ColourTransform {
        val brightness = brightnessOffset(effects.brightness)
        return contrast(contrastFactor(effects.contrast))
            .after(hueRotation(effects.hue))
            .after(saturation(saturationFactor(effects.saturation)))
            .after(look(effects.look))
            .let { ColourTransform(it.matrix, DoubleArray(3) { row -> it.offset[row] + brightness }) }
    }

    fun shader(effects: PictureEffects): ShaderParams {
        val noise = effects.denoise / 100.0
        return ShaderParams(
            colour = colour(effects),
            gamma = gammaExponent(effects.gamma).toFloat(),
            sharpen = sharpenAmount(effects.sharpen).toFloat(),
            deflicker = effects.deflicker > 0,
            follow = deflickerFollow(effects.deflicker).toFloat(),
            denoise = effects.denoise > 0,
            spatial = (0.15 + 0.6 * noise).toFloat(),
            spatialSigma = (0.02 + 0.05 * noise).toFloat(),
            temporal = (0.35 + 0.4 * noise).toFloat(),
            temporalSigma = (0.02 + 0.04 * noise).toFloat(),
        )
    }

    /** One-tap starting points for the Effects sheet (the same as `EFFECT_PRESETS` on desktop). */
    val PRESETS: List<Pair<String, PictureEffects>> = listOf(
        "presetOff" to PictureEffects.NONE,
        "presetOldFilm" to PictureEffects(contrast = 10, sharpen = 15, deflicker = 60, denoise = 35),
        "presetCleanUp" to PictureEffects(sharpen = 20, denoise = 45),
        "presetBrighter" to PictureEffects(brightness = 5, gamma = 30),
        "presetCinema" to PictureEffects(contrast = 15, saturation = 10, gamma = -10),
    )
}

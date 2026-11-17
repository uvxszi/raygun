package app.raygun.audio

import org.json.JSONObject
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sqrt

/**
 * The watermark remover (`WatermarkRemoval` in raygun-model): a box in ten-thousandths of the
 * picture from its top left, brightness and contrast for what's left (−100 to 100) and the
 * feather (0 to 100). Off unless turned on, kept per title in its picture effects.
 */
internal data class WatermarkRemoval(
    val enabled: Boolean = false,
    val x: Int = 0,
    val y: Int = 0,
    val width: Int = 0,
    val height: Int = 0,
    val brightness: Int = 0,
    val contrast: Int = 0,
    val feather: Int = 0,
) {
    /** On, with a box to work in. */
    val active: Boolean get() = enabled && width > 0 && height > 0

    /** The same box (a slider moving doesn't start the estimate again). */
    fun sameBox(other: WatermarkRemoval): Boolean =
        x == other.x && y == other.y && width == other.width && height == other.height

    fun clamped(): WatermarkRemoval {
        val left = x.coerceIn(0, WatermarkMath.SCALE)
        val top = y.coerceIn(0, WatermarkMath.SCALE)
        return WatermarkRemoval(
            enabled = enabled,
            x = left,
            y = top,
            width = width.coerceIn(0, WatermarkMath.SCALE - left),
            height = height.coerceIn(0, WatermarkMath.SCALE - top),
            brightness = brightness.coerceIn(-100, 100),
            contrast = contrast.coerceIn(-100, 100),
            feather = feather.coerceIn(0, 100),
        )
    }

    fun toJson(): JSONObject = JSONObject()
        .put("enabled", enabled)
        .put("x", x)
        .put("y", y)
        .put("width", width)
        .put("height", height)
        .put("brightness", brightness)
        .put("contrast", contrast)
        .put("feather", feather)

    companion object {
        val NONE = WatermarkRemoval()

        fun from(json: JSONObject?): WatermarkRemoval {
            json ?: return NONE
            return WatermarkRemoval(
                enabled = json.optBoolean("enabled", false),
                x = json.optInt("x", 0),
                y = json.optInt("y", 0),
                width = json.optInt("width", 0),
                height = json.optInt("height", 0),
                brightness = json.optInt("brightness", 0),
                contrast = json.optInt("contrast", 0),
                feather = json.optInt("feather", 0),
            ).clamped()
        }
    }
}

/** A rectangle in pixels of the picture, from its top left. */
internal data class PixelRect(val left: Double, val top: Double, val width: Double, val height: Double)

/** Where the box's samples come from: the box and a ring of picture around it, on one grid. */
internal data class SampleLayout(
    val rect: PixelRect,
    val cols: Int,
    val rows: Int,
    val innerLeft: Int,
    val innerTop: Int,
    val innerCols: Int,
    val innerRows: Int,
)

/** The overlay found: opacity and colour per box cell, an RGBA8 texture's bytes, rows from the top. */
internal class WatermarkMask(val cols: Int, val rows: Int, val data: ByteArray)

/**
 * The watermark remover as numbers, the same as the desktop player's (`watermarkMath.ts`; the
 * tests of both check the same numbers). A static overlay blends as I = α·W + (1 − α)·J; over
 * many frames the video under it changes while the overlay stays, so a pixel's change is (1 − α)
 * times the video's: α comes from the box's temporal variance against the ring of picture round
 * it, W from the means. `fx_watermark.glsl` inverts the blend.
 */
internal object WatermarkMath {
    const val SCALE = 10_000
    const val MAX_CELLS = 48
    const val MIN_SAMPLES = 24
    const val MIN_REFERENCE_VARIANCE = 0.0015
    const val ALPHA_FLOOR = 0.06
    const val SAMPLE_CAP = 1_500

    fun boxPixels(box: WatermarkRemoval, frameWidth: Int, frameHeight: Int) = PixelRect(
        box.x.toDouble() / SCALE * frameWidth,
        box.y.toDouble() / SCALE * frameHeight,
        box.width.toDouble() / SCALE * frameWidth,
        box.height.toDouble() / SCALE * frameHeight,
    )

    /** At most [MAX_CELLS] cells across the box (at least 4), a ring about half its size round it. */
    fun sampleLayout(box: WatermarkRemoval, frameWidth: Int, frameHeight: Int): SampleLayout {
        val pixels = boxPixels(box, frameWidth, frameHeight)
        val scale = min(1.0, MAX_CELLS / max(max(pixels.width, pixels.height), 1.0))
        val innerCols = max(4, roundHalfUp(pixels.width * scale))
        val innerRows = max(4, roundHalfUp(pixels.height * scale))
        val cellWidth = pixels.width / innerCols
        val cellHeight = pixels.height / innerRows
        val ringCols = ceil(innerCols / 2.0).toInt()
        val ringRows = ceil(innerRows / 2.0).toInt()
        fun fits(room: Double, cell: Double): Int = if (cell > 0) floor(room / cell + 1e-6).toInt() else 0
        val left = min(ringCols, fits(pixels.left, cellWidth))
        val right = min(ringCols, fits(frameWidth - pixels.left - pixels.width, cellWidth))
        val top = min(ringRows, fits(pixels.top, cellHeight))
        val bottom = min(ringRows, fits(frameHeight - pixels.top - pixels.height, cellHeight))
        return SampleLayout(
            rect = PixelRect(
                pixels.left - left * cellWidth,
                pixels.top - top * cellHeight,
                (innerCols + left + right) * cellWidth,
                (innerRows + top + bottom) * cellHeight,
            ),
            cols = innerCols + left + right,
            rows = innerRows + top + bottom,
            innerLeft = left,
            innerTop = top,
            innerCols = innerCols,
            innerRows = innerRows,
        )
    }

    /** JavaScript's `Math.round`, so both players lay the same grid. */
    fun roundHalfUp(value: Double): Int = floor(value + 0.5).toInt()

    fun luma(red: Double, green: Double, blue: Double) = 0.2126 * red + 0.7152 * green + 0.0722 * blue

    /** The box in the shaders' texture coordinates (origin at the bottom left): x0, y0, x1, y1. */
    fun shaderBox(box: WatermarkRemoval): FloatArray = floatArrayOf(
        box.x.toFloat() / SCALE,
        1f - (box.y + box.height).toFloat() / SCALE,
        (box.x + box.width).toFloat() / SCALE,
        1f - box.y.toFloat() / SCALE,
    )

    fun brightness(value: Int): Double = value / 100.0 * 0.25

    fun contrast(value: Int): Double = 2.0.pow(value / 100.0)

    fun feather(value: Int): Double = value / 100.0 * 0.5
}

/** Sums of the box's samples over time; [estimate] gives the mask once there are enough. */
internal class WatermarkEstimator(val layout: SampleLayout) {
    var samples = 0.0
        private set
    private val inner = DoubleArray(layout.innerCols * layout.innerRows * 5)
    private val ringCells: IntArray
    private val ringLuma: DoubleArray
    private val ringSquares: DoubleArray
    private val ringColour = DoubleArray(3)

    init {
        val cells = ArrayList<Int>()
        for (row in 0 until layout.rows) {
            for (col in 0 until layout.cols) {
                val inside = col >= layout.innerLeft && col < layout.innerLeft + layout.innerCols &&
                    row >= layout.innerTop && row < layout.innerTop + layout.innerRows
                if (!inside) cells += row * layout.cols + col
            }
        }
        ringCells = cells.toIntArray()
        ringLuma = DoubleArray(ringCells.size)
        ringSquares = DoubleArray(ringCells.size)
    }

    private fun byte(pixels: ByteArray, at: Int): Double = (pixels[at].toInt() and 0xFF) / 255.0

    /** One sample: `cols × rows` RGBA bytes, rows from the top. */
    fun add(pixels: ByteArray) {
        if (samples >= WatermarkMath.SAMPLE_CAP) halve()
        for (row in 0 until layout.innerRows) {
            for (col in 0 until layout.innerCols) {
                val at = ((row + layout.innerTop) * layout.cols + col + layout.innerLeft) * 4
                val red = byte(pixels, at)
                val green = byte(pixels, at + 1)
                val blue = byte(pixels, at + 2)
                val luma = WatermarkMath.luma(red, green, blue)
                val sums = (row * layout.innerCols + col) * 5
                inner[sums] += red
                inner[sums + 1] += green
                inner[sums + 2] += blue
                inner[sums + 3] += luma
                inner[sums + 4] += luma * luma
            }
        }
        ringCells.forEachIndexed { index, cell ->
            val at = cell * 4
            val red = byte(pixels, at)
            val green = byte(pixels, at + 1)
            val blue = byte(pixels, at + 2)
            val luma = WatermarkMath.luma(red, green, blue)
            ringLuma[index] += luma
            ringSquares[index] += luma * luma
            ringColour[0] += red
            ringColour[1] += green
            ringColour[2] += blue
        }
        samples += 1
    }

    private fun halve() {
        for (sums in listOf(inner, ringLuma, ringSquares, ringColour)) {
            for (index in sums.indices) sums[index] /= 2
        }
        samples /= 2
    }

    fun referenceVariance(): Double {
        val n = samples
        if (n < 2 || ringCells.isEmpty()) return 0.0
        var total = 0.0
        for (index in ringCells.indices) {
            val mean = ringLuma[index] / n
            total += max(0.0, ringSquares[index] / n - mean * mean)
        }
        return total / ringCells.size
    }

    fun estimate(): WatermarkMask? {
        val n = samples
        val reference = referenceVariance()
        if (n < WatermarkMath.MIN_SAMPLES || reference < WatermarkMath.MIN_REFERENCE_VARIANCE) return null
        val cols = layout.innerCols
        val rows = layout.innerRows
        val ringCount = ringCells.size * n
        val background = DoubleArray(3) { ringColour[it] / ringCount }
        val raw = DoubleArray(cols * rows) { cell ->
            val mean = inner[cell * 5 + 3] / n
            val variance = max(0.0, inner[cell * 5 + 4] / n - mean * mean)
            1 - min(1.0, sqrt(variance / reference))
        }
        val data = ByteArray(cols * rows * 4)
        for (row in 0 until rows) {
            for (col in 0 until cols) {
                var sum = 0.0
                var taps = 0
                for (dy in -1..1) {
                    for (dx in -1..1) {
                        val y = row + dy
                        val x = col + dx
                        if (x < 0 || y < 0 || x >= cols || y >= rows) continue
                        sum += raw[y * cols + x]
                        taps += 1
                    }
                }
                val cell = row * cols + col
                val mean = sum / taps
                val alpha = if (mean < WatermarkMath.ALPHA_FLOOR) 0.0 else min(0.98, mean)
                val out = cell * 4
                if (alpha > 0) {
                    for (channel in 0 until 3) {
                        val colour = (inner[cell * 5 + channel] / n - (1 - alpha) * background[channel]) / alpha
                        data[out + channel] = WatermarkMath.roundHalfUp(colour.coerceIn(0.0, 1.0) * 255).toByte()
                    }
                }
                data[out + 3] = WatermarkMath.roundHalfUp(alpha * 255).toByte()
            }
        }
        return WatermarkMask(cols, rows, data)
    }
}

/** Luma of the whole picture over time, to find a static overlay ([find]). */
internal class OverlayFinder {
    var samples = 0
        private set
    private val sums = DoubleArray(COLS * ROWS)
    private val squares = DoubleArray(COLS * ROWS)

    /** One sample: `COLS × ROWS` RGBA bytes, rows from the top. */
    fun add(pixels: ByteArray) {
        for (cell in 0 until COLS * ROWS) {
            val at = cell * 4
            val luma = WatermarkMath.luma(
                (pixels[at].toInt() and 0xFF) / 255.0,
                (pixels[at + 1].toInt() and 0xFF) / 255.0,
                (pixels[at + 2].toInt() and 0xFF) / 255.0,
            )
            sums[cell] += luma
            squares[cell] += luma * luma
        }
        samples += 1
    }

    /**
     * The box round the largest still, edged patch (a logo stays while the video moves), or null
     * when there's none or the picture hardly changes.
     */
    fun find(): WatermarkRemoval? {
        val n = samples.toDouble()
        if (samples < 2) return null
        val cells = COLS * ROWS
        val means = DoubleArray(cells) { sums[it] / n }
        val deviations = DoubleArray(cells) { sqrt(max(0.0, squares[it] / n - means[it] * means[it])) }
        val typical = deviations.sorted()[cells / 2]
        if (typical < MIN_CHANGE) return null
        fun meanAt(col: Int, row: Int) = means[row.coerceIn(0, ROWS - 1) * COLS + col.coerceIn(0, COLS - 1)]
        val still = BooleanArray(cells)
        for (row in 0 until ROWS) {
            for (col in 0 until COLS) {
                val here = meanAt(col, row)
                val edge = maxOf(
                    abs(meanAt(col + 1, row) - here),
                    abs(meanAt(col - 1, row) - here),
                    abs(meanAt(col, row + 1) - here),
                    abs(meanAt(col, row - 1) - here),
                )
                val cell = row * COLS + col
                if (deviations[cell] < typical * STILL_SHARE && edge > MIN_EDGE) still[cell] = true
            }
        }
        val group = IntArray(cells) { -1 }
        var best: IntArray? = null // count, left, top, right, bottom
        var groups = 0
        for (start in 0 until cells) {
            if (!still[start] || group[start] >= 0) continue
            val found = intArrayOf(0, COLS, ROWS, -1, -1)
            val stack = ArrayDeque<Int>()
            stack.addLast(start)
            group[start] = groups
            while (stack.isNotEmpty()) {
                val cell = stack.removeLast()
                val col = cell % COLS
                val row = cell / COLS
                found[0] += 1
                found[1] = min(found[1], col)
                found[3] = max(found[3], col)
                found[2] = min(found[2], row)
                found[4] = max(found[4], row)
                for (dy in -2..2) {
                    for (dx in -2..2) {
                        val x = col + dx
                        val y = row + dy
                        if (x < 0 || y < 0 || x >= COLS || y >= ROWS) continue
                        val next = y * COLS + x
                        if (!still[next] || group[next] >= 0) continue
                        group[next] = groups
                        stack.addLast(next)
                    }
                }
            }
            groups += 1
            val area = (found[3] - found[1] + 1).toDouble() * (found[4] - found[2] + 1) / cells
            // A line across most of the picture is a letterbox's edge, not a logo.
            val narrow = found[3] - found[1] + 1 <= COLS * MAX_SPAN && found[4] - found[2] + 1 <= ROWS * MAX_SPAN
            if (found[0] >= 3 && area <= MAX_AREA && narrow && (best == null || found[0] > best[0])) best = found
        }
        val box = best ?: return null
        val left = max(0, box[1] - 1)
        val top = max(0, box[2] - 1)
        val right = min(COLS, box[3] + 2)
        val bottom = min(ROWS, box[4] + 2)
        fun scaleX(col: Int) = WatermarkMath.roundHalfUp(col.toDouble() / COLS * WatermarkMath.SCALE)
        fun scaleY(row: Int) = WatermarkMath.roundHalfUp(row.toDouble() / ROWS * WatermarkMath.SCALE)
        return WatermarkRemoval(
            x = scaleX(left),
            y = scaleY(top),
            width = scaleX(right) - scaleX(left),
            height = scaleY(bottom) - scaleY(top),
        )
    }

    companion object {
        const val COLS = 64
        const val ROWS = 36
        const val SAMPLES = 40
        private const val STILL_SHARE = 0.35
        private const val MIN_CHANGE = 0.02
        private const val MIN_EDGE = 0.035
        private const val MAX_AREA = 0.2
        private const val MAX_SPAN = 0.6
    }
}

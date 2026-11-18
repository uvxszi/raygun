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
    const val ALPHA_FLOOR = 0.08

    /** The most opacity the remover takes out: more would amplify noise (and casts) past use. */
    const val ALPHA_MAX = 0.6

    /** How far the overlay's colour may stray from grey (per channel from its luma). */
    const val CHROMA_LIMIT = 0.08
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

    /**
     * One mask cell (RGBA bytes: the overlay's colour times its opacity, then the opacity) from its
     * estimated opacity, mean colour and the background's; unsure cells are left alone and the
     * colour stays near grey, so no cast comes from it (`maskTexel` in `watermarkMath.ts`).
     */
    /** A box cell this still (estimated opacity) may be part of the overlay… */
    const val SUPPORT_STILL = 0.25

    /** …which has edges: its mean luma differs from a neighbour's (box or ring) by this much. */
    const val SUPPORT_EDGE = 0.05

    /**
     * The box's cells that are the overlay, as `overlaySupport` in `watermarkMath.ts`: still cells
     * joined side by side into a patch with an edge somewhere (a cell standing out from a
     * neighbour's mean picture, the ring included). `raw` is the box's opacity per cell, `lumas`
     * the whole grid's mean luma.
     */
    fun overlaySupport(raw: DoubleArray, lumas: DoubleArray, layout: SampleLayout): BooleanArray {
        val cols = layout.innerCols
        val rows = layout.innerRows
        val sides = arrayOf(intArrayOf(1, 0), intArrayOf(-1, 0), intArrayOf(0, 1), intArrayOf(0, -1))
        fun still(col: Int, row: Int) = raw[row * cols + col] >= SUPPORT_STILL
        fun luma(gridCol: Int, gridRow: Int) = lumas[gridRow * layout.cols + gridCol]
        fun edged(col: Int, row: Int): Boolean {
            val here = luma(col + layout.innerLeft, row + layout.innerTop)
            return sides.any { (dx, dy) ->
                val gridCol = col + dx + layout.innerLeft
                val gridRow = row + dy + layout.innerTop
                gridCol >= 0 && gridRow >= 0 && gridCol < layout.cols && gridRow < layout.rows &&
                    abs(luma(gridCol, gridRow) - here) > SUPPORT_EDGE
            }
        }
        val support = BooleanArray(cols * rows)
        val seen = BooleanArray(cols * rows)
        for (start in 0 until cols * rows) {
            if (seen[start] || !still(start % cols, start / cols)) continue
            val patch = ArrayList<Int>()
            var hasEdge = false
            val stack = ArrayDeque<Int>().apply { addLast(start) }
            seen[start] = true
            while (stack.isNotEmpty()) {
                val cell = stack.removeLast()
                val col = cell % cols
                val row = cell / cols
                patch += cell
                if (!hasEdge && edged(col, row)) hasEdge = true
                for ((dx, dy) in sides) {
                    val x = col + dx
                    val y = row + dy
                    if (x < 0 || y < 0 || x >= cols || y >= rows) continue
                    val next = y * cols + x
                    if (seen[next] || !still(x, y)) continue
                    seen[next] = true
                    stack.addLast(next)
                }
            }
            if (hasEdge) patch.forEach { support[it] = true }
        }
        return support
    }

    fun maskTexel(rawAlpha: Double, mean: DoubleArray, background: DoubleArray): IntArray {
        val texel = IntArray(4)
        if (rawAlpha < ALPHA_FLOOR) return texel
        val alpha = min(ALPHA_MAX, rawAlpha)
        val markLuma = (luma(mean[0], mean[1], mean[2]) - (1 - alpha) * luma(background[0], background[1], background[2])) / alpha
        if (markLuma < -0.1 || markLuma > 1.1) return texel
        for (channel in 0 until 3) {
            val colour = (mean[channel] - (1 - alpha) * background[channel]) / alpha
            val near = markLuma + (colour - markLuma).coerceIn(-CHROMA_LIMIT, CHROMA_LIMIT)
            texel[channel] = roundHalfUp(alpha * near.coerceIn(0.0, 1.0) * 255)
        }
        texel[3] = roundHalfUp(alpha * 255)
        return texel
    }

    /** What `fx_watermark.glsl` does with a pixel (0–1 RGB) under a mask texel (bytes). */
    fun restorePixel(pixel: DoubleArray, texel: IntArray): DoubleArray {
        val alpha = texel[3] / 255.0
        return DoubleArray(3) { ((pixel[it] - texel[it] / 255.0) / max(1 - alpha, 0.4)).coerceIn(0.0, 1.0) }
    }
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
        // Only the overlay itself: still cells making up a mark with edges of its own. Still parts
        // of the picture without one stay as they are, never darkened or lightened.
        val support = WatermarkMath.overlaySupport(raw, meanLumas(), layout)
        if (support.none { it }) return null
        val data = ByteArray(cols * rows * 4)
        for (row in 0 until rows) {
            for (col in 0 until cols) {
                val cell = row * cols + col
                if (!support[cell]) continue
                var sum = 0.0
                var taps = 0
                for (dy in -1..1) {
                    for (dx in -1..1) {
                        val y = row + dy
                        val x = col + dx
                        if (x < 0 || y < 0 || x >= cols || y >= rows || !support[y * cols + x]) continue
                        sum += raw[y * cols + x]
                        taps += 1
                    }
                }
                val texel = WatermarkMath.maskTexel(sum / taps, DoubleArray(3) { inner[cell * 5 + it] / n }, background)
                for (index in 0 until 4) data[cell * 4 + index] = texel[index].toByte()
            }
        }
        return WatermarkMask(cols, rows, data)
    }

    /** The mean luma of the box's cells, and of the ring round it in its cells, rows from the top. */
    private fun meanLumas(): DoubleArray {
        val n = max(1.0, samples)
        val lumas = DoubleArray(layout.cols * layout.rows)
        for (row in 0 until layout.innerRows) {
            for (col in 0 until layout.innerCols) {
                lumas[(row + layout.innerTop) * layout.cols + col + layout.innerLeft] = inner[(row * layout.innerCols + col) * 5 + 3] / n
            }
        }
        // The ring as the picture round the box: its mean over all its cells (one cell's own mean
        // is too noisy to tell an edge by).
        val ring = ringLuma.sum() / n / max(1, ringCells.size)
        ringCells.forEach { cell -> lumas[cell] = ring }
        return lumas
    }
}

/** What finding concluded: a box ([found]), none, or that the picture moved too little to tell. */
internal class FindVerdict(val found: WatermarkRemoval?, val unsure: Boolean)

/**
 * Luma of the whole picture over time, to find a static overlay ([verdict]). Samples go on until
 * the picture has changed shot at least [CUTS] times (each a sample far from its shot's first),
 * since within one held shot scenery with edges looks just like an overlay; a patch that stays the
 * same across the shots is one (`OverlayFinder` in `watermarkMath.ts`).
 */
internal class OverlayFinder {
    var samples = 0
        private set

    /** Scene changes seen so far. */
    var cuts = 0
        private set
    private val sums = DoubleArray(COLS * ROWS)
    private val squares = DoubleArray(COLS * ROWS)
    private var shotStart: DoubleArray? = null

    /** The first samples of the latest shots, to check a patch looks the same across each cut. */
    private val shotStarts = ArrayDeque<DoubleArray>()

    /** Enough to answer: the scene changes and samples needed, or as many samples as are taken. */
    val ready: Boolean get() = (cuts >= CUTS && samples >= SAMPLES) || samples >= MAX_SAMPLES

    /** One sample: `COLS × ROWS` RGBA bytes, rows from the top. */
    fun add(pixels: ByteArray) {
        val lumas = DoubleArray(COLS * ROWS)
        val start = shotStart
        var difference = 0.0
        for (cell in 0 until COLS * ROWS) {
            val at = cell * 4
            val luma = WatermarkMath.luma(
                (pixels[at].toInt() and 0xFF) / 255.0,
                (pixels[at + 1].toInt() and 0xFF) / 255.0,
                (pixels[at + 2].toInt() and 0xFF) / 255.0,
            )
            lumas[cell] = luma
            if (start != null) difference += abs(luma - start[cell])
            sums[cell] += luma
            squares[cell] += luma * luma
        }
        if (start == null || difference / (COLS * ROWS) > CUT_DIFFERENCE) {
            if (start != null) cuts += 1
            shotStart = lumas
            shotStarts.addLast(lumas)
            if (shotStarts.size > KEPT_SHOTS) shotStarts.removeFirst()
        }
        samples += 1
    }

    fun find(): WatermarkRemoval? = verdict().found

    /**
     * A box round the largest patch that stays put while the picture round it moves a lot, stands
     * out from its neighbours, is logo-sized and stays the same across [CUTS] scene changes
     * (nearer the picture's edges wins a tie); nothing unless the evidence is strong; unsure when
     * too little moved or the scene didn't change often enough (`verdict` in `watermarkMath.ts`).
     */
    fun verdict(): FindVerdict {
        val unsure = FindVerdict(null, true)
        if (samples < 2 || cuts < CUTS) return unsure
        val n = samples.toDouble()
        val cells = COLS * ROWS
        val means = DoubleArray(cells) { sums[it] / n }
        val deviations = DoubleArray(cells) { sqrt(max(0.0, squares[it] / n - means[it] * means[it])) }
        val typical = deviations.sorted()[cells / 2]
        if (typical < MIN_CHANGE) return unsure
        val stillLimit = min(typical * STILL_SHARE, STILL_MOST)
        if (deviations.count { it < stillLimit } > cells * MOST_STILL) return unsure
        fun inside(col: Int, row: Int) = col >= 0 && row >= 0 && col < COLS && row < ROWS
        val still = BooleanArray(cells)
        for (row in 0 until ROWS) {
            for (col in 0 until COLS) {
                val cell = row * COLS + col
                if (deviations[cell] >= stillLimit) continue
                // The same before and after each cut, not just on average.
                if (shotStarts.any { abs(it[cell] - means[cell]) > SHOT_MATCH }) continue
                var edge = 0.0
                for ((dx, dy) in NEIGHBOURS) {
                    if (!inside(col + dx, row + dy)) continue
                    edge = max(edge, abs(means[(row + dy) * COLS + col + dx] - means[cell]))
                }
                if (edge > MIN_EDGE) still[cell] = true
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
                        if (!inside(x, y)) continue
                        val next = y * COLS + x
                        if (!still[next] || group[next] >= 0) continue
                        group[next] = groups
                        stack.addLast(next)
                    }
                }
            }
            groups += 1
            val width = found[3] - found[1] + 1
            val height = found[4] - found[2] + 1
            val plausible = found[0] >= MIN_CELLS && width.toDouble() * height / cells <= MAX_AREA &&
                width <= COLS * MAX_SPAN && height <= ROWS * MAX_SPAN
            // Larger wins; of two the same size, the one nearer the picture's edge (where logos sit).
            fun fromEdge(patch: IntArray) = minOf(patch[1], patch[2], COLS - 1 - patch[3], ROWS - 1 - patch[4])
            val better = best == null || found[0] > best[0] || (found[0] == best[0] && fromEdge(found) < fromEdge(best))
            if (!plausible || !better) continue
            // The picture just round the patch must move: a still patch in a still corner is scenery.
            val ring = ArrayList<Double>()
            for (row in found[2] - 2..found[4] + 2) {
                for (col in found[1] - 2..found[3] + 2) {
                    val onRing = row < found[2] || row > found[4] || col < found[1] || col > found[3]
                    if (onRing && inside(col, row)) ring += deviations[row * COLS + col]
                }
            }
            ring.sort()
            val ringTypical = ring.getOrNull(ring.size / 2) ?: 0.0
            if (ringTypical < typical * RING_SHARE) continue
            val members = (0 until cells).filter { group[it] == groups - 1 }
            if (structureHolds(members, found, means)) best = found
        }
        val box = best ?: return FindVerdict(null, false)
        val left = max(0, box[1] - 1)
        val top = max(0, box[2] - 1)
        val right = min(COLS, box[3] + 2)
        val bottom = min(ROWS, box[4] + 2)
        fun scaleX(col: Int) = WatermarkMath.roundHalfUp(col.toDouble() / COLS * WatermarkMath.SCALE)
        fun scaleY(row: Int) = WatermarkMath.roundHalfUp(row.toDouble() / ROWS * WatermarkMath.SCALE)
        return FindVerdict(
            WatermarkRemoval(
                x = scaleX(left),
                y = scaleY(top),
                width = scaleX(right) - scaleX(left),
                height = scaleY(bottom) - scaleY(top),
            ),
            false,
        )
    }

    private class Spread(val mean: Double, val deviation: Double)

    private fun spread(values: List<Double>): Spread {
        val mean = values.sum() / max(1, values.size)
        val variance = values.sumOf { (it - mean) * (it - mean) } / max(1, values.size)
        return Spread(mean, sqrt(variance))
    }

    /**
     * A patch's own look holds across the shots: its box isn't uniform (grass, sky and walls never
     * count), it stands out from the cells round it in every shot's first sample, and its pattern
     * there correlates with its pattern over all samples (NCC, the patch's mean removed).
     * `found` is count, left, top, right, bottom.
     */
    private fun structureHolds(members: List<Int>, found: IntArray, means: DoubleArray): Boolean {
        val boxed = ArrayList<Double>()
        for (row in found[2]..found[4]) for (col in found[1]..found[3]) boxed += means[row * COLS + col]
        if (spread(boxed).deviation < MIN_STRUCTURE) return false
        val member = members.toHashSet()
        val pattern = members.map { means[it] }
        val shape = spread(pattern)
        for (start in shotStarts) {
            var contrast = 0.0
            var pairs = 0
            for (cell in members) {
                val col = cell % COLS
                val row = cell / COLS
                for ((dx, dy) in NEIGHBOURS) {
                    val x = col + dx
                    val y = row + dy
                    val next = y * COLS + x
                    if (x < 0 || y < 0 || x >= COLS || y >= ROWS || next in member) continue
                    contrast += abs(start[cell] - start[next])
                    pairs += 1
                }
            }
            if (pairs == 0 || contrast / pairs < MIN_EDGE) return false
            if (shape.deviation >= PATTERN_SPREAD) {
                val seen = members.map { start[it] }
                val shot = spread(seen)
                var product = 0.0
                seen.forEachIndexed { index, value -> product += (value - shot.mean) * (pattern[index] - shape.mean) }
                val scale = members.size * shot.deviation * shape.deviation
                val ncc = if (scale > 0) product / scale else 0.0
                if (ncc < MIN_NCC) return false
            }
        }
        return true
    }

    companion object {
        const val COLS = 64
        const val ROWS = 36
        const val SAMPLES = 40

        /** The most samples taken (about a minute at one every 4th frame) before giving up. */
        const val MAX_SAMPLES = 450

        /** Scene changes an overlay must stay through. */
        const val CUTS = 2
        private const val CUT_DIFFERENCE = 0.08
        private const val SHOT_MATCH = 0.04
        private const val KEPT_SHOTS = 16
        private const val MIN_STRUCTURE = 0.06
        private const val PATTERN_SPREAD = 0.03
        private const val MIN_NCC = 0.9
        private val NEIGHBOURS = listOf(1 to 0, -1 to 0, 0 to 1, 0 to -1)
        private const val STILL_SHARE = 0.15
        private const val STILL_MOST = 0.02
        private const val MIN_CHANGE = 0.04
        private const val MOST_STILL = 0.15
        private const val MIN_EDGE = 0.08
        private const val RING_SHARE = 0.5
        private const val MIN_CELLS = 6
        private const val MAX_AREA = 0.1
        private const val MAX_SPAN = 0.4
    }
}
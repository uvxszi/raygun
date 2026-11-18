package app.raygun.audio

import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.roundToLong
import kotlin.math.sqrt

/** A frame smooth motion sends on: at `pts`, `t` of the way from the frame before to the next. */
internal data class MotionSlot(val pts: Long, val t: Float)

/** A luma picture for checking the flow's search on the CPU, a value per pixel (row by row). */
internal class LumaImage(val width: Int, val height: Int, val values: FloatArray) {
    private fun pixel(x: Int, y: Int): Double =
        values[y.coerceIn(0, height - 1) * width + x.coerceIn(0, width - 1)].toDouble()

    /** Linear between pixels (their centres at whole numbers), the edges repeated, as the GPU reads. */
    fun at(x: Double, y: Double): Double {
        val left = floor(x).toInt()
        val top = floor(y).toInt()
        val fx = x - left
        val fy = y - top
        val upper = pixel(left, top) * (1 - fx) + pixel(left + 1, top) * fx
        val lower = pixel(left, top + 1) * (1 - fx) + pixel(left + 1, top + 1) * fx
        return upper * (1 - fy) + lower * fy
    }

    /** Half the size, each pixel the mean of four (`fx_motion_luma.glsl` from a level). */
    fun half(): LumaImage {
        val w = max(1, width / 2)
        val h = max(1, height / 2)
        return LumaImage(w, h, FloatArray(w * h) { index ->
            val x = (index % w) * 2
            val y = (index / w) * 2
            ((pixel(x, y) + pixel(x + 1, y) + pixel(x, y + 1) + pixel(x + 1, y + 1)) / 4).toFloat()
        })
    }
}

/** A vector found by the search: the motion in a level's pixels and how badly its patch matched. */
internal data class FlowMatch(val x: Double, val y: Double, val cost: Double)

/** A flow field on the CPU: a vector and cost per pixel of the flow's level (row by row). */
internal class FlowField(val width: Int, val height: Int, val matches: List<FlowMatch>) {
    fun at(x: Int, y: Int): FlowMatch = matches[y.coerceIn(0, height - 1) * width + x.coerceIn(0, width - 1)]

    /** The vector between pixels (centres at whole numbers), as the GPU reads the flow linearly. */
    fun sample(x: Double, y: Double): Pair<Double, Double> {
        val left = floor(x).toInt()
        val top = floor(y).toInt()
        val fx = x - left
        val fy = y - top
        fun lerp(pick: (FlowMatch) -> Double): Double =
            (pick(at(left, top)) * (1 - fx) + pick(at(left + 1, top)) * fx) * (1 - fy) +
                (pick(at(left, top + 1)) * (1 - fx) + pick(at(left + 1, top + 1)) * fx) * fy
        return lerp { it.x } to lerp { it.y }
    }
}

/** What a model searches: radii coarsest first, then the refinement steps. */
internal data class MotionPlan(val radii: IntArray, val refine: DoubleArray) {
    override fun equals(other: Any?): Boolean =
        other is MotionPlan && radii.contentEquals(other.radii) && refine.contentEquals(other.refine)

    override fun hashCode(): Int = radii.contentHashCode() * 31 + refine.contentHashCode()
}

/**
 * Smooth motion's numbers (`glsl/fx_motion_*.glsl`, and `motionMath.ts` on desktop): the flow's
 * encoding, its search (the same as the shader's, for tests), when frames are sent, how in-between
 * frames blend, and when the work is too much for the phone.
 */
internal object MotionMath {
    const val FLOW_RANGE = 64.0
    const val COST_RANGE = 0.5
    const val FLOW_ZERO = 128.0 / 255.0
    const val SEARCH_PENALTY = 0.004
    const val ZERO_BIAS = 0.003
    const val BAD_COST = 0.12
    const val CUT_COST = 0.09
    const val CUT_SHARE = 0.45
    const val TRUST_FROM = 0.05
    const val TRUST_TO = 0.18

    /** The finest flow level's widths, best first: a phone that can't keep up steps down. */
    val LEVEL_WIDTHS = intArrayOf(384, 256, 160)

    /** Levels in each pyramid, finest first. */
    const val LEVELS = 3

    /** Steps searched either way at each level, coarsest first; the finest is then refined by half steps. */
    val RADII = intArrayOf(3, 1, 1)

    /** A longer gap between two frames is no motion to follow (a seek, a stall). */
    const val MAX_GAP_US = 250_000L

    /** Frames this close (in target intervals) come often enough already: sent as they are. */
    const val OFTEN_ENOUGH = 1.2

    /** The share of a source frame's time smooth motion may take on the GPU. */
    const val BUDGET_SHARE = 0.45

    /** Stepping down a level, or giving up after the last. */
    const val GIVE_UP = -1

    /** The rate to make: the screen's, or an even share of it at most 60 (90 Hz → 45, 120 → 60). */
    fun targetFps(displayHz: Float): Double {
        val hz = if (displayHz.isFinite() && displayHz >= 20f) displayHz.toDouble() else 60.0
        return hz / ceil(hz / 60.0 - 1e-6)
    }

    /** The media time between two frames sent: faster playback covers more of it per frame shown. */
    fun intervalUs(fps: Double, speed: Float): Double = 1_000_000.0 / fps * max(speed, 0.1f)

    /**
     * The frames to send from the frame before (`p0`) to the next (`p1`): the gap cut into as many
     * equal steps as fit at `interval` (never more frames than the screen's share), the last being
     * the next frame itself (`t` 1). Ending on a decoded frame keeps Media3's replay cache going: it
     * lets the frame after in only once one at or past this one has been shown.
     */
    fun steps(p0: Long, p1: Long, interval: Double): List<MotionSlot> {
        if (p1 <= p0 || interval <= 0.0) return emptyList()
        val gap = (p1 - p0).toDouble()
        val count = max(1, floor(gap / interval + 0.01).toInt())
        return (1..count).map { step ->
            MotionSlot(if (step == count) p1 else p0 + (gap * step / count).roundToLong(), step.toFloat() / count)
        }
    }

    /** Whether frames this far apart are made smoother, or already come often enough. */
    fun interpolates(gapUs: Long, interval: Double): Boolean =
        gapUs in 1..MAX_GAP_US && gapUs > interval * OFTEN_ENOUGH

    /** A value as an 8-bit channel keeps it. */
    fun channel(value: Double): Double = (value.coerceIn(0.0, 1.0) * 255).roundToInt() / 255.0

    fun encodeFlow(flow: Double): Double = channel(flow / FLOW_RANGE + FLOW_ZERO)

    fun decodeFlow(stored: Double): Double = (stored - FLOW_ZERO) * FLOW_RANGE

    /** A cut between scenes: the in-between frames repeat the nearer frame. */
    fun isCut(meanCost: Double, badShare: Double): Boolean = meanCost > CUT_COST || badShare > CUT_SHARE

    fun smoothstep(edge0: Double, edge1: Double, value: Double): Double {
        val x = ((value - edge0) / (edge1 - edge0)).coerceIn(0.0, 1.0)
        return x * x * (3 - 2 * x)
    }

    /** How much of the blend a pixel gets: 1 where the two samples agree, 0 where they don't. */
    fun trust(differ: Double): Double = 1 - smoothstep(TRUST_FROM, TRUST_TO, differ)

    /** One channel of an in-between frame (`fx_motion_warp.glsl`). */
    fun blend(before: Double, after: Double, t: Double, differ: Double): Double {
        val nearer = if (t < 0.5) before else after
        val both = before + (after - before) * t
        return nearer + (both - nearer) * trust(differ)
    }

    /** A patch with fewer taps inside both frames than this matches nothing ([NO_MATCH]). */
    const val FEWEST_TAPS = 3
    const val NO_MATCH = 1.0

    /** Whether a point (in pixels, centres at whole numbers) lies on the picture. */
    fun onPicture(image: LumaImage, x: Double, y: Double): Boolean =
        x >= -0.5 && y >= -0.5 && x <= image.width - 0.5 && y <= image.height - 0.5

    /**
     * The mean difference of 3 × 3 patches, the frame before at -flow/2 and the next at +flow/2;
     * taps outside either frame don't count.
     */
    fun patchCost(prev: LumaImage, cur: LumaImage, x: Double, y: Double, flowX: Double, flowY: Double): Double {
        var sum = 0.0
        var taps = 0
        for (dy in -1..1) {
            for (dx in -1..1) {
                val bx = x - flowX / 2 + dx
                val by = y - flowY / 2 + dy
                val ax = x + flowX / 2 + dx
                val ay = y + flowY / 2 + dy
                if (!onPicture(prev, bx, by) || !onPicture(cur, ax, ay)) continue
                sum += abs(prev.at(bx, by) - cur.at(ax, ay))
                taps += 1
            }
        }
        return if (taps < FEWEST_TAPS) NO_MATCH else sum / taps
    }

    /** The search at one pixel of a level (`fx_motion_search.glsl`), from `start`. */
    fun search(
        prev: LumaImage,
        cur: LumaImage,
        x: Int,
        y: Int,
        startX: Double,
        startY: Double,
        radius: Int,
        step: Double,
    ): FlowMatch {
        var best = FlowMatch(startX, startY, patchCost(prev, cur, x.toDouble(), y.toDouble(), startX, startY))
        for (sy in -radius..radius) {
            for (sx in -radius..radius) {
                if (sx == 0 && sy == 0) continue
                val fx = startX + sx * step
                val fy = startY + sy * step
                val cost = patchCost(prev, cur, x.toDouble(), y.toDouble(), fx, fy) +
                    SEARCH_PENALTY * sqrt((sx * step) * (sx * step) + (sy * step) * (sy * step))
                if (cost < best.cost) best = FlowMatch(fx, fy, cost)
            }
        }
        val still = patchCost(prev, cur, x.toDouble(), y.toDouble(), 0.0, 0.0)
        return if (still <= best.cost + ZERO_BIAS) FlowMatch(0.0, 0.0, still) else best
    }

    /** The flow at a pixel of the finest level, coarse to fine as the passes find it (without the median). */
    fun flowAt(prev: LumaImage, cur: LumaImage, x: Int, y: Int): FlowMatch =
        flowField(prev, cur, plan(MOTION_BALANCED)!!).at(x, y)

    // ---- The models, the steadied flow and the in-between frames (fx_motion_smooth/warp.glsl) ----

    const val MOTION_BLEND = "blend"
    const val MOTION_BALANCED = "balanced"
    const val MOTION_SHARP = "sharp"

    /** Smooth motion's models (`MotionModel` in raygun-model), in `uModel`'s order. */
    val MODELS = listOf(MOTION_BLEND, MOTION_BALANCED, MOTION_SHARP)

    fun modelNumber(model: String): Int = MODELS.indexOf(model).takeIf { it >= 0 } ?: 1

    /** What each model searches; none for blend, which makes its frames without the flow. */
    fun plan(model: String): MotionPlan? = when (model) {
        MOTION_BLEND -> null
        MOTION_SHARP -> MotionPlan(intArrayOf(3, 2, 2), doubleArrayOf(0.5, 0.25))
        else -> MotionPlan(RADII.copyOf(), doubleArrayOf(0.5))
    }

    const val MAX_MOTION = 40.0
    const val EDGE_SIGMA = 0.06
    const val VECTOR_SIGMA = 2.0
    const val COST_SIGMA = 0.08
    const val SPREAD_WEIGHT = 0.02
    const val BLEND_STILL = 0.03
    const val UNSURE_FROM = 0.1
    const val UNSURE_TO = 0.2
    const val AGREE_FROM = 1.0
    const val AGREE_TO = 3.0

    /**
     * One search pass over a level (`fx_motion_search.glsl`): each pixel starts from the best of
     * the init field's vector there and a coarse texel to each side, and no motion, then searches
     * round it. `init` is in the finest level's pixels; `scale` this level's size in them.
     */
    fun searchLevel(prev: LumaImage, cur: LumaImage, init: FlowField?, scale: Int, radius: Int, step: Double): FlowField {
        val matches = ArrayList<FlowMatch>(prev.width * prev.height)
        val sides = arrayOf(intArrayOf(0, 0), intArrayOf(-1, 0), intArrayOf(1, 0), intArrayOf(0, -1), intArrayOf(0, 1))
        for (y in 0 until prev.height) {
            for (x in 0 until prev.width) {
                var startX = 0.0
                var startY = 0.0
                var startCost = patchCost(prev, cur, x.toDouble(), y.toDouble(), 0.0, 0.0)
                if (init != null) {
                    val ratio = init.width.toDouble() / prev.width
                    val ix = (x + 0.5) * ratio - 0.5
                    val iy = (y + 0.5) * ratio - 0.5
                    for ((dx, dy) in sides) {
                        // Two of this level's pixels away: a coarser level's neighbour.
                        val (fx, fy) = init.sample(ix + dx * 2 * ratio, iy + dy * 2 * ratio)
                        val cx = fx / scale
                        val cy = fy / scale
                        val cost = patchCost(prev, cur, x.toDouble(), y.toDouble(), cx, cy)
                        if (cost < startCost) {
                            startX = cx
                            startY = cy
                            startCost = cost
                        }
                    }
                }
                var best = FlowMatch(startX, startY, startCost)
                for (sy in -radius..radius) {
                    for (sx in -radius..radius) {
                        if (sx == 0 && sy == 0) continue
                        val fx = startX + sx * step
                        val fy = startY + sy * step
                        val cost = patchCost(prev, cur, x.toDouble(), y.toDouble(), fx, fy) +
                            SEARCH_PENALTY * sqrt((sx * step) * (sx * step) + (sy * step) * (sy * step))
                        if (cost < best.cost) best = FlowMatch(fx, fy, cost)
                    }
                }
                val still = patchCost(prev, cur, x.toDouble(), y.toDouble(), 0.0, 0.0)
                if (still <= best.cost + ZERO_BIAS) best = FlowMatch(0.0, 0.0, still)
                matches += FlowMatch(best.x * scale, best.y * scale, best.cost)
            }
        }
        return FlowField(prev.width, prev.height, matches)
    }

    /** The whole field as the search passes find it: coarse to fine, then the model's refinements. */
    fun flowField(prev: LumaImage, cur: LumaImage, plan: MotionPlan): FlowField {
        val prevs = generateSequence(prev) { it.half() }.take(LEVELS).toList()
        val curs = generateSequence(cur) { it.half() }.take(LEVELS).toList()
        var field: FlowField? = null
        for (level in LEVELS - 1 downTo 0) {
            field = searchLevel(prevs[level], curs[level], field, 1 shl level, plan.radii[LEVELS - 1 - level], 1.0)
        }
        for (step in plan.refine) field = searchLevel(prev, cur, field, 1, 1, step)
        return field!!
    }

    /**
     * `fx_motion_smooth.glsl`: the vector median over 3 × 3 (also judged by how well it carries the
     * pixel), then an edge-aware mean round it, cut to [MAX_MOTION].
     */
    fun smoothFlow(field: FlowField, prev: LumaImage, cur: LumaImage): FlowField {
        fun picture(x: Int, y: Int): Double = 0.5 * (prev.at(x.toDouble(), y.toDouble()) + cur.at(x.toDouble(), y.toDouble()))
        val matches = field.matches.mapIndexed { index, own ->
            val x = index % field.width
            val y = index / field.width
            val around = ArrayList<FlowMatch>(9)
            val pictures = ArrayList<Double>(9)
            for (dy in -1..1) {
                for (dx in -1..1) {
                    around += field.at(x + dx, y + dy)
                    pictures += picture((x + dx).coerceIn(0, field.width - 1), (y + dy).coerceIn(0, field.height - 1))
                }
            }
            var median = around[4]
            var least = Double.MAX_VALUE
            for (each in around) {
                val spread = around.sumOf { other -> hypot(each.x - other.x, each.y - other.y) }
                val carried = abs(prev.at(x - each.x / 2, y - each.y / 2) - cur.at(x + each.x / 2, y + each.y / 2))
                val score = spread / 9 * SPREAD_WEIGHT + carried
                if (score < least) {
                    least = score
                    median = each
                }
            }
            var sumX = 0.0
            var sumY = 0.0
            var weights = 0.0
            around.forEachIndexed { at, each ->
                val alike = (pictures[at] - pictures[4]) / EDGE_SIGMA
                val near = hypot(each.x - median.x, each.y - median.y) / VECTOR_SIGMA
                val weight = exp(-(alike * alike) - near * near - each.cost / COST_SIGMA)
                sumX += each.x * weight
                sumY += each.y * weight
                weights += weight
            }
            var flowX = if (weights > 1e-4) sumX / weights else median.x
            var flowY = if (weights > 1e-4) sumY / weights else median.y
            val size = hypot(flowX, flowY)
            if (size > MAX_MOTION) {
                flowX *= MAX_MOTION / size
                flowY *= MAX_MOTION / size
            }
            FlowMatch(flowX, flowY, own.cost)
        }
        return FlowField(field.width, field.height, matches)
    }

    private fun agrees(field: FlowField, flowX: Double, flowY: Double, x: Double, y: Double): Double {
        val (thereX, thereY) = field.sample(x, y)
        return 1 - smoothstep(AGREE_FROM, AGREE_TO, hypot(thereX - flowX, thereY - flowY))
    }

    /**
     * One pixel of an in-between frame (`fx_motion_warp.glsl`) on luma pictures at the flow's size,
     * the model's way; `cut` for a scene cut.
     */
    fun warpPixel(
        prev: LumaImage,
        cur: LumaImage,
        field: FlowField?,
        x: Int,
        y: Int,
        t: Double,
        model: String,
        cut: Boolean = false,
    ): Double {
        val here0 = prev.at(x.toDouble(), y.toDouble())
        val here1 = cur.at(x.toDouble(), y.toDouble())
        val nearer = if (t < 0.5) here0 else here1
        if (model == MOTION_BLEND || field == null) {
            val still = 1 - smoothstep(BLEND_STILL, 2 * BLEND_STILL, abs(here0 - here1))
            return nearer + (here0 + (here1 - here0) * t - nearer) * still
        }
        if (cut) return nearer
        val own = field.at(x, y)
        var flowX = own.x
        var flowY = own.y
        val size = hypot(flowX, flowY)
        if (size > MAX_MOTION) {
            flowX *= MAX_MOTION / size
            flowY *= MAX_MOTION / size
        }
        val fromX = x - t * flowX
        val fromY = y - t * flowY
        val toX = x + (1 - t) * flowX
        val toY = y + (1 - t) * flowY
        val before = prev.at(fromX, fromY)
        val after = cur.at(toX, toY)
        val path = agrees(field, flowX, flowY, x + (0.5 - t) * flowX, y + (0.5 - t) * flowY)
        val weightBefore = (1 - t) * (if (onPicture(prev, fromX, fromY)) 1.0 else 0.0) * agrees(field, flowX, flowY, fromX, fromY)
        val weightAfter = t * (if (onPicture(prev, toX, toY)) 1.0 else 0.0) * agrees(field, flowX, flowY, toX, toY)
        val weights = weightBefore + weightAfter
        var warped = if (weights > 0) (before * weightBefore + after * weightAfter) / weights else nearer
        val both = if (min(weightBefore, weightAfter) > 0) 1 - smoothstep(TRUST_FROM, TRUST_TO, abs(before - after)) else 1.0
        var nearerSide = if (weightBefore >= weightAfter) before else after
        if (weightBefore > 0 && weightAfter > 0) nearerSide = if (t < 0.5) before else after
        warped = nearerSide + (warped - nearerSide) * both
        // A badly matched block is believed where its two samples still agree.
        val agreed = if (min(weightBefore, weightAfter) > 0) 1 - smoothstep(TRUST_FROM, TRUST_TO, abs(before - after)) else 0.0
        val sure = path * max(1 - smoothstep(UNSURE_FROM, UNSURE_TO, own.cost), agreed) * (if (weights >= 0.05) 1.0 else 0.0)
        return nearer + (warped - nearer) * sure
    }

    /** A whole in-between frame on the CPU: the model's flow (steadied), then each pixel warped. */
    fun interpolateFrame(prev: LumaImage, cur: LumaImage, t: Double, model: String): LumaImage {
        val field = plan(model)?.let { smoothFlow(flowField(prev, cur, it), prev, cur) }
        return LumaImage(prev.width, prev.height, FloatArray(prev.width * prev.height) { index ->
            warpPixel(prev, cur, field, index % prev.width, index / prev.width, t, model).toFloat()
        })
    }

    /** The finest level's size for a frame at `quality` (an index of [LEVEL_WIDTHS]). */
    fun flowSize(width: Int, height: Int, quality: Int): Pair<Int, Int> {
        val w = min(LEVEL_WIDTHS[quality.coerceIn(0, LEVEL_WIDTHS.size - 1)], max(width, 16))
        val h = max(8, (w.toDouble() * height / max(width, 1)).roundToInt())
        return w to h
    }

    /**
     * The quality to use after a measurement: the next lower one when the work took more than the
     * budget ([BUDGET_SHARE] of the time between source frames), [GIVE_UP] after the lowest.
     */
    fun nextQuality(quality: Int, costMs: Double, gapMs: Double): Int = when {
        costMs <= gapMs * BUDGET_SHARE -> quality
        quality + 1 < LEVEL_WIDTHS.size -> quality + 1
        else -> GIVE_UP
    }
}

/**
 * Turns smooth motion off when frames stop going on while the video plays: the GL thread stamps
 * each frame sent ([sent]); the player's frame check asks [stalled] about once a second.
 */
internal class MotionWatchdog(private val limitMs: Long = STALL_MS) {
    @Volatile
    private var lastSentMs = 0L

    /** When watching began (playing with smooth motion on); 0 while not watching. */
    private var sinceMs = 0L

    fun sent(nowMs: Long) {
        lastSentMs = nowMs
    }

    /** No frame for longer than the limit while playing with smooth motion on. */
    fun stalled(nowMs: Long, playing: Boolean, on: Boolean): Boolean {
        if (!playing || !on) {
            sinceMs = 0L
            return false
        }
        if (sinceMs == 0L) {
            sinceMs = nowMs
            return false
        }
        return nowMs - max(lastSentMs, sinceMs) > limitMs
    }

    companion object {
        const val STALL_MS = 500L
    }
}
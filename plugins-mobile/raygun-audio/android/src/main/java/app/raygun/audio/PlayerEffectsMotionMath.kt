package app.raygun.audio

import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
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

    /** The mean difference of 3 × 3 patches, the frame before at -flow/2 and the next at +flow/2. */
    fun patchCost(prev: LumaImage, cur: LumaImage, x: Double, y: Double, flowX: Double, flowY: Double): Double {
        var sum = 0.0
        for (dy in -1..1) {
            for (dx in -1..1) {
                sum += abs(prev.at(x - flowX / 2 + dx, y - flowY / 2 + dy) - cur.at(x + flowX / 2 + dx, y + flowY / 2 + dy))
            }
        }
        return sum / 9
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

    /**
     * The flow at a pixel of the finest level, coarse to fine as the passes find it (without the
     * median), in the finest level's pixels.
     */
    fun flowAt(prev: LumaImage, cur: LumaImage, x: Int, y: Int): FlowMatch {
        val prevs = generateSequence(prev) { it.half() }.take(LEVELS).toList()
        val curs = generateSequence(cur) { it.half() }.take(LEVELS).toList()
        var flowX = 0.0
        var flowY = 0.0
        var match = FlowMatch(0.0, 0.0, 0.0)
        for (level in LEVELS - 1 downTo 0) {
            val scale = 1 shl level
            val lx = min(x / scale, prevs[level].width - 1)
            val ly = min(y / scale, prevs[level].height - 1)
            match = search(prevs[level], curs[level], lx, ly, flowX / scale, flowY / scale, RADII[LEVELS - 1 - level], 1.0)
            flowX = match.x * scale
            flowY = match.y * scale
        }
        match = search(prevs[0], curs[0], x, y, flowX, flowY, 1, 0.5)
        return match
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
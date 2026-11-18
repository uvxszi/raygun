package app.raygun.audio

/**
 * Automatic quality that keeps a video playing: while the quality is automatic, playback that
 * keeps stalling to buffer (two stalls within 30 s, or one longer than 4 s) steps down to the next
 * lower quality; after two minutes without a stall an adaptive stream steps back up one at a time.
 * The same rules as the desktop player's (`video/autoQuality.ts`).
 */
internal object AutoQualityRules {
    /** Stalls within this long of each other count together. */
    const val STALL_WINDOW_MS = 30_000L
    /** This many stalls within the window step down. */
    const val STALLS_TO_LOWER = 2
    /** A stall this long steps down by itself. */
    const val LONG_STALL_MS = 4_000L
    /** After a step, stalls while the new quality settles in don't count. */
    const val SETTLE_MS = 8_000L
    /** Playing this long without a stall steps an adaptive stream back up. */
    const val SMOOTH_MS = 120_000L
}

internal enum class StallAction { LOWER, RAISE }

/**
 * Watches the player's stalls, sampled on a timer: [tick] with whether it plays and whether it's
 * stalled (buffering while playing, not after a seek or before the first frame) says when to step
 * down or back up; [stepped] after acting starts the count again.
 */
internal class StallWatch {
    private val stalls = ArrayDeque<Long>()
    private var stallStart: Long? = null
    private var longActed = false
    private var settleUntil = 0L
    private var smooth = 0L
    private var lastTick: Long? = null

    /** A new video: nothing counts from before. */
    fun reset(now: Long) {
        stalls.clear()
        stallStart = null
        longActed = false
        settleUntil = now + AutoQualityRules.SETTLE_MS
        smooth = 0
        lastTick = now
    }

    /** After a step down or up: the count starts again once the new quality settles. */
    fun stepped(now: Long) = reset(now)

    /** `canRaise`: a step down is in effect that could be lifted (adaptive streams only). */
    fun tick(now: Long, playing: Boolean, stalled: Boolean, canRaise: Boolean): StallAction? {
        val since = lastTick?.let { (now - it).coerceAtLeast(0) } ?: 0L
        lastTick = now
        val settling = now < settleUntil
        if (stalled) {
            smooth = 0
            val start = stallStart ?: now.also {
                stallStart = it
                longActed = false
                if (!settling) stalls.addLast(it)
            }
            while (stalls.isNotEmpty() && now - stalls.first() > AutoQualityRules.STALL_WINDOW_MS) stalls.removeFirst()
            if (settling) return null
            if (!longActed && now - start >= AutoQualityRules.LONG_STALL_MS) {
                longActed = true
                return StallAction.LOWER
            }
            return if (stalls.size >= AutoQualityRules.STALLS_TO_LOWER) StallAction.LOWER else null
        }
        stallStart = null
        if (playing) smooth += since
        if (canRaise && smooth >= AutoQualityRules.SMOOTH_MS) {
            smooth = 0
            return StallAction.RAISE
        }
        return null
    }
}

/** A row of the Quality menu as the steps see it (Rust's `qualities` items). */
internal data class QualityStep(
    val key: String,
    val height: Int,
    val range: String,
    val state: String,
    /** The variants' own sides it holds. */
    val sizes: List<Pair<Int, Int>> = emptyList(),
)

/** The tier a picture counts as for capping: its long side against a 16:9 one's, or its height. */
internal fun pictureHeight(width: Int, height: Int): Int {
    val short = minOf(width, height)
    val long = maxOf(width, height)
    return maxOf(short, (long * 9 + 8) / 16)
}

/** The cap for a row's variants: the largest picture among them, else its tier. */
internal fun capOf(step: QualityStep): Int =
    (step.sizes.map { (width, height) -> pictureHeight(width, height) } + step.height).max()

/**
 * The row to step down to from the one playing: the largest below it. An adaptive stream steps
 * only among its own variants (never below its lowest); otherwise another link plays. The same
 * range as the one playing first, then SDR. Null when there's nothing lower.
 */
internal fun lowerStep(rows: List<QualityStep>, adaptive: Boolean): QualityStep? {
    val playing = rows.firstOrNull { it.state == "playing" } ?: return null
    val lower = rows.filter { row ->
        row.height < playing.height &&
            if (adaptive) row.state == "variant" else row.state == "thisSource" || row.state == "otherSource"
    }
    val top = lower.maxOfOrNull { it.height } ?: return null
    val atTop = lower.filter { it.height == top }
    return atTop.firstOrNull { it.range == playing.range } ?: atTop.firstOrNull { it.range == "sdr" } ?: atTop.first()
}

/** One step back up from `cap`: the next variant row above it, or null when none is. */
internal fun raiseStep(rows: List<QualityStep>, cap: Int): QualityStep? =
    rows.filter { it.state == "variant" && it.height > cap }.minByOrNull { it.height }

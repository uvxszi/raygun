package app.raygun.audio

import android.content.Context
import android.opengl.GLES20
import android.os.SystemClock
import android.util.Log
import androidx.media3.common.GlObjectsProvider
import androidx.media3.common.GlTextureInfo
import androidx.media3.common.VideoFrameProcessingException
import androidx.media3.common.util.UnstableApi
import androidx.media3.effect.GlEffect
import androidx.media3.effect.GlShaderProgram
import java.util.ArrayDeque
import java.util.Locale
import java.util.concurrent.Executor
import java.io.File

/** In-between frames ready to go at once (each full size); more wait for one to come back. */
private const val OUTPUTS = 4

/** The work is timed once in this many source frames (each timing waits for the GPU). */
private const val MEASURE_EVERY = 60L

/** Measurements averaged before the quality changes. */
private const val MEASURES_TO_JUDGE = 3

/** Smooth motion's switch and what its timing follows, set on the main thread. */
internal data class MotionParams(
    val on: Boolean = false,
    /** The screen's refresh rate. */
    val displayHz: Float = 60f,
    val speed: Float = 1f,
    /** The Smooth Motion chip's folder, which holds its shaders; none while it isn't installed and on. */
    val dir: String? = null,
    /** How in-between frames are made ([MotionMath.MODELS]). */
    val model: String = MotionMath.MOTION_BALANCED,
)

/** How smooth motion does on this phone, written on the GL thread. */
internal class MotionStatus {
    /** An index of [MotionMath.LEVEL_WIDTHS]. */
    @Volatile
    var quality = 0

    /** Too slow even at the lowest quality, or its passes failed here: frames go on as they are. */
    @Volatile
    var gaveUp = false

    /** Asked for again (main thread): back to the best quality. */
    @Volatile
    var retry = false

    @Volatile
    var flowMs = 0.0

    @Volatile
    var madeMs = 0.0

    /** When frames last went on (stamped on the GL thread, checked on the main thread). */
    val watchdog = MotionWatchdog()

    /** The video's frames already come as often as the screen's rate: nothing is made between them. */
    @Volatile
    var alreadySmooth = false

    fun again() {
        retry = true
        gaveUp = false
    }
}

/**
 * Smooth motion as a Media3 effect, after the picture's ([PlayerEffects] adds both): between two
 * decoded frames it makes as many evenly spaced frames as the screen's rate allows (at most 60 a
 * second, or 45 on a 90 Hz screen; [MotionMath.steps]), each at its own presentation time, then
 * sends the decoded frame itself, so Media3's video sink shows them in turn like decoded ones (and
 * its replay cache, which lets a frame in once the one before has been shown, keeps going). If no
 * frame goes on for a while as the video plays, [MotionWatchdog] turns it off. The motion is found by block matching on luma pyramids
 * (`fx_motion_*.glsl`): a coarse search, two finer ones (each starting from the best of the coarser
 * vectors round it) and the model's refinements ([MotionMath.plan]: sharp searches wider and refines
 * to quarter pixels, blend makes no flow at all), a vector median judged by the pixel it carries and
 * an edge-aware mean, then each in-between frame follows it back and forth, leaving out a side that
 * falls outside the picture or on other motion (forward/backward consistency), and repeats the
 * nearer frame where the match is unsure; after a scene cut the nearer frame repeats. It times itself now and then, steps down the flow's size when it takes too
 * long and passes frames through as they are when even the smallest is too slow
 * ([MotionStatus.gaveUp], logged as `smooth motion:` lines under `RaygunEffects`).
 */
@UnstableApi
internal class SmoothMotionGl(private val params: EffectParams, private val status: MotionStatus) : GlEffect {
    override fun toGlShaderProgram(context: Context, useHdr: Boolean): GlShaderProgram =
        SmoothMotionProgram(context, params, status, useHdr)
}

/** One frame to send: a kept frame as it is, or an in-between one at `t`. */
private class MotionSend(val pts: Long, val kept: Int, val t: Float)

/** A texture sent on, and whether the next program still has it. */
private class MotionOutput(val target: GlTarget) {
    val info = GlTextureInfo(target.texture, target.framebuffer, -1, target.width, target.height)
    var busy = false
}

@UnstableApi
private class SmoothMotionProgram(
    private val context: Context,
    private val params: EffectParams,
    private val status: MotionStatus,
    private val hdr: Boolean,
) : GlShaderProgram {
    private var inputListener: GlShaderProgram.InputListener = object : GlShaderProgram.InputListener {}
    private var outputListener: GlShaderProgram.OutputListener = object : GlShaderProgram.OutputListener {}
    private var errorListener: GlShaderProgram.ErrorListener = GlShaderProgram.ErrorListener {}
    private var errorExecutor: Executor = Executor { it.run() }

    private var passes: MotionPasses? = null
    private var width = 0
    private var height = 0
    private var outputs: List<MotionOutput> = emptyList()

    // The two kept frames (the one before and the latest), their luma pyramids and times.
    private val kept = arrayOfNulls<GlTarget>(2)
    private val pyramids = Array(2) { arrayOfNulls<GlTarget>(MotionMath.LEVELS) }
    private val keptPts = LongArray(2)
    private var latest = 0
    private var keptCount = 0

    // The flow: a search target per level, the refined finest level, its median and the cut test.
    private val search = arrayOfNulls<GlTarget>(MotionMath.LEVELS)
    private var refined: GlTarget? = null
    /** Sharp's second refinement; the two refinements are used in turn. */
    private var refinedMore: GlTarget? = null
    private var smoothed: GlTarget? = null
    private var error: GlTarget? = null
    private var pyramidQuality = -1

    private val sends = ArrayDeque<MotionSend>()
    private val waiting = ArrayDeque<Pair<GlTextureInfo, Long>>()
    /** Frames passed straight through, by texture: given back to the program before when returned. */
    private val passed = HashMap<Int, GlTextureInfo>()
    private var lastInputPts = Long.MIN_VALUE
    private var lastSentPts = Long.MIN_VALUE
    private var ended = false

    // Timing.
    private var frames = 0L
    private var measureMade = false
    private var flowNs = 0L
    private val costs = ArrayList<Double>()

    override fun setInputListener(listener: GlShaderProgram.InputListener) {
        inputListener = listener
        listener.onReadyToAcceptInputFrame()
    }

    override fun setOutputListener(listener: GlShaderProgram.OutputListener) {
        outputListener = listener
    }

    override fun setErrorListener(executor: Executor, listener: GlShaderProgram.ErrorListener) {
        errorExecutor = executor
        errorListener = listener
    }

    private fun smoothing(): Boolean {
        if (status.retry) {
            status.retry = false
            status.quality = 0
            costs.clear()
        }
        return !hdr && params.motion.on && !status.gaveUp
    }

    override fun queueInputFrame(provider: GlObjectsProvider, input: GlTextureInfo, presentationTimeUs: Long) {
        try {
            if (!smoothing()) {
                passThrough(input, presentationTimeUs)
                return
            }
            // Back in time (a frame drawn again while paused, a seek without a flush): what was
            // waiting is for another place.
            if (presentationTimeUs <= lastInputPts) restart()
            lastInputPts = presentationTimeUs
            waiting.addLast(input to presentationTimeUs)
            inputListener.onReadyToAcceptInputFrame()
            pump()
        } catch (failure: Exception) {
            fail(failure, presentationTimeUs)
        }
    }

    /** Off (or given up): the frame goes on as it came, and back to the program before when done. */
    private fun passThrough(input: GlTextureInfo, pts: Long) {
        if (sends.isNotEmpty() || waiting.isNotEmpty() || keptCount > 0) restart()
        lastInputPts = pts
        lastSentPts = pts
        passed[input.texId] = input
        outputListener.onOutputFrameAvailable(input, pts)
        inputListener.onReadyToAcceptInputFrame()
    }

    /** What was kept and waiting is dropped (a seek, a frame drawn again, smooth motion off). */
    private fun restart() {
        sends.clear()
        while (waiting.isNotEmpty()) inputListener.onInputFrameProcessed(waiting.removeFirst().first)
        keptCount = 0
        lastInputPts = Long.MIN_VALUE
        lastSentPts = Long.MIN_VALUE
    }

    override fun releaseOutputFrame(texture: GlTextureInfo) {
        try {
            val through = passed.remove(texture.texId)
            if (through != null) {
                inputListener.onInputFrameProcessed(through)
                return
            }
            outputs.firstOrNull { it.info.texId == texture.texId }?.busy = false
            pump()
        } catch (failure: Exception) {
            fail(failure, lastSentPts)
        }
    }

    /** Sends what's ready while there's a texture for it, then takes the next frame waiting. */
    private fun pump() {
        while (true) {
            if (sends.isNotEmpty()) {
                val output = outputs.firstOrNull { !it.busy } ?: return
                send(sends.removeFirst(), output)
                continue
            }
            val (input, pts) = waiting.pollFirst() ?: break
            take(input, pts)
        }
        if (ended) {
            // The last frame kept was waiting for the next; there's none.
            if (keptCount > 0 && keptPts[latest] > lastSentPts) {
                val output = outputs.firstOrNull { !it.busy } ?: return
                send(MotionSend(keptPts[latest], latest, 0f), output)
            }
            ended = false
            outputListener.onCurrentOutputStreamEnded()
        }
    }

    /** A new frame: kept, its luma pyramid made, then the frames up to it planned. */
    private fun take(input: GlTextureInfo, pts: Long) {
        val passes = passes ?: loadPasses().also { passes = it }
        if (input.width != width || input.height != height) resize(input.width, input.height)
        if (status.quality != pyramidQuality) makePyramids()
        val slot = if (keptCount == 0) 0 else 1 - latest
        val frame = kept[slot]!!
        frame.focus()
        passes.copy.use()
        passes.copy.texture("uCopyFrom", 0, input.texId, GLES20.GL_NEAREST)
        passes.copy.draw()
        inputListener.onInputFrameProcessed(input)

        val previous = latest
        latest = slot
        keptPts[slot] = pts
        keptCount = minOf(keptCount + 1, 2)
        val interval = MotionMath.intervalUs(MotionMath.targetFps(params.motion.displayHz), params.motion.speed)
        val gap = if (keptCount == 2) pts - keptPts[previous] else 0L
        val making = keptCount == 2 && MotionMath.interpolates(gap, interval)
        val measuring = making && frames++ % MEASURE_EVERY == 5L
        val started = if (measuring) finishedAt() else 0L
        drawPyramid(slot, frame)
        if (keptCount == 2) noteFrequency(gap, interval, making)
        if (!making) {
            // The first frame, or frames that come often enough (or too far apart): as they are.
            if (pts > lastSentPts) sends.addLast(MotionSend(pts, slot, 0f))
            return
        }
        // Blend makes its frames without the flow.
        if (MotionMath.plan(params.motion.model) != null) drawFlow(previous, slot)
        if (measuring) {
            flowNs = finishedAt() - started
            measureMade = true
        }
        // The frames between, then this one as it is: Media3's replay cache lets the next frame in
        // only once one at or after this one has been shown.
        for (each in MotionMath.steps(keptPts[previous], pts, interval)) {
            if (each.pts <= lastSentPts) continue
            sends.addLast(if (each.t >= 1f) MotionSend(each.pts, slot, 0f) else MotionSend(each.pts, previous, each.t))
        }
    }

    /** Smooth motion's programs from the chip's folder; a file or shader that fails is logged by name. */
    private fun loadPasses(): MotionPasses {
        val dir = params.motion.dir
        return try {
            MotionPasses(context, dir).also {
                Log.i(EFFECTS_TAG, "smooth motion: shaders loaded from $dir")
            }
        } catch (failure: Exception) {
            Log.e(EFFECTS_TAG, "smooth motion: shaders from $dir failed to load or compile: ${failure.message}", failure)
            throw failure
        }
    }

    /**
     * Says once (until that changes) when the video's frames already come as often as the screen's
     * rate (a 60 fps video), so no frame is made between them: smooth motion is on but has nothing
     * to add.
     */
    private fun noteFrequency(gapUs: Long, interval: Double, making: Boolean) {
        val often = !making && gapUs in 1..MotionMath.MAX_GAP_US
        if (often == status.alreadySmooth) return
        status.alreadySmooth = often
        if (often) {
            Log.i(
                EFFECTS_TAG,
                String.format(
                    Locale.ROOT,
                    "smooth motion: nothing to make, frames already come every %.1f ms (%.0f fps) for a %.0f fps screen",
                    gapUs / 1000.0, 1e6 / gapUs, 1e6 / interval,
                ),
            )
        } else {
            Log.i(EFFECTS_TAG, String.format(Locale.ROOT, "smooth motion: making frames, the video's come every %.1f ms", gapUs / 1000.0))
        }
    }

    private fun finishedAt(): Long {
        GLES20.glFinish()
        return SystemClock.elapsedRealtimeNanos()
    }

    private fun send(next: MotionSend, output: MotionOutput) {
        val passes = passes!!
        output.target.focus()
        if (next.t <= 0.01f) {
            passes.copy.use()
            passes.copy.texture("uCopyFrom", 0, kept[next.kept]!!.texture, GLES20.GL_NEAREST)
            passes.copy.draw()
        } else {
            val timing = measureMade
            val started = if (timing) finishedAt() else 0L
            val flow = smoothed!!
            val warp = passes.warp
            warp.use()
            warp.texture("uPrev", 0, kept[next.kept]!!.texture, GLES20.GL_LINEAR)
            warp.texture("uCur", 1, kept[1 - next.kept]!!.texture, GLES20.GL_LINEAR)
            warp.texture("uFlow", 2, flow.texture, GLES20.GL_LINEAR)
            warp.texture("uError", 3, error!!.texture, GLES20.GL_NEAREST)
            warp.set("uT", next.t)
            warp.set("uModel", MotionMath.modelNumber(params.motion.model).toFloat())
            warp.set("uFlowTexel", 1f / flow.width, 1f / flow.height)
            warp.draw()
            if (timing) {
                measureMade = false
                judge(finishedAt() - started)
            }
        }
        output.busy = true
        lastSentPts = next.pts
        status.watchdog.sent(SystemClock.elapsedRealtime())
        outputListener.onOutputFrameAvailable(output.info, next.pts)
    }

    /** One measurement: the flow and one in-between frame against the time between source frames. */
    private fun judge(madeNs: Long) {
        val interval = MotionMath.intervalUs(MotionMath.targetFps(params.motion.displayHz), params.motion.speed)
        val gapUs = keptPts[latest] - keptPts[1 - latest]
        val made = (MotionMath.steps(0, gapUs, interval).size - 1).coerceAtLeast(1).toDouble()
        val flowMs = flowNs / 1e6
        val madeMs = madeNs / 1e6
        status.flowMs = flowMs
        status.madeMs = madeMs
        costs += flowMs + madeMs * made
        val quality = status.quality
        val (flowWidth, flowHeight) = MotionMath.flowSize(width, height, quality)
        Log.i(
            EFFECTS_TAG,
            String.format(
                Locale.ROOT,
                "smooth motion: %.2f ms flow + %.2f ms a made frame (%.1f made a frame) at %dx%d, flow %dx%d, %.0f fps",
                flowMs, madeMs, made, width, height, flowWidth, flowHeight, MotionMath.targetFps(params.motion.displayHz),
            ),
        )
        if (costs.size < MEASURES_TO_JUDGE) return
        val mean = costs.average()
        costs.clear()
        when (val next = MotionMath.nextQuality(quality, mean, gapUs / 1000.0)) {
            quality -> Unit
            MotionMath.GIVE_UP -> {
                Log.i(EFFECTS_TAG, String.format(Locale.ROOT, "smooth motion off: %.2f ms a source frame is too slow", mean))
                status.gaveUp = true
            }
            else -> {
                Log.i(EFFECTS_TAG, String.format(Locale.ROOT, "smooth motion: %.2f ms a source frame, smaller flow", mean))
                status.quality = next
            }
        }
    }

    private fun resize(newWidth: Int, newHeight: Int) {
        width = newWidth
        height = newHeight
        deleteFrames()
        outputs = List(OUTPUTS) { MotionOutput(GlTarget(width, height)) }
        for (index in 0..1) kept[index] = GlTarget(width, height)
        keptCount = 0
        pyramidQuality = -1
    }

    private fun makePyramids() {
        deletePyramids()
        pyramidQuality = status.quality
        val (flowWidth, flowHeight) = MotionMath.flowSize(width, height, pyramidQuality)
        for (level in 0 until MotionMath.LEVELS) {
            val levelWidth = maxOf(4, flowWidth shr level)
            val levelHeight = maxOf(4, flowHeight shr level)
            for (index in 0..1) pyramids[index][level] = GlTarget(levelWidth, levelHeight)
            search[level] = GlTarget(levelWidth, levelHeight)
        }
        refined = GlTarget(flowWidth, flowHeight)
        refinedMore = GlTarget(flowWidth, flowHeight)
        smoothed = GlTarget(flowWidth, flowHeight)
        error = GlTarget(1, 1)
        // The frame kept has no pyramid at this size: the next pair starts afresh.
        keptCount = 0
    }

    private fun drawPyramid(slot: Int, frame: GlTarget) {
        val luma = passes!!.luma
        var from = frame
        for (level in 0 until MotionMath.LEVELS) {
            val into = pyramids[slot][level]!!
            into.focus()
            luma.use()
            luma.texture("uLumaFrom", 0, from.texture, GLES20.GL_LINEAR)
            luma.set("uSpread", 0.25f / into.width, 0.25f / into.height)
            luma.set("uPicture", if (level == 0) 1f else 0f)
            luma.draw()
            from = into
        }
    }

    private fun drawFlow(previous: Int, latest: Int) {
        val passes = passes!!
        val plan = MotionMath.plan(params.motion.model) ?: return
        val searchPass = passes.search
        var init: GlTarget? = null
        for (level in MotionMath.LEVELS - 1 downTo 0) {
            val into = search[level]!!
            searchStep(searchPass, into, previous, latest, level, init, plan.radii[MotionMath.LEVELS - 1 - level], 1f)
            init = into
        }
        // The refinements in turn (sharp takes another, at quarter steps).
        plan.refine.forEachIndexed { index, step ->
            val into = if (index % 2 == 0) refined!! else refinedMore!!
            searchStep(searchPass, into, previous, latest, 0, init, 1, step.toFloat())
            init = into
        }

        val flow = smoothed!!
        flow.focus()
        passes.smooth.use()
        passes.smooth.texture("uFlowIn", 0, (init ?: refined!!).texture, GLES20.GL_NEAREST)
        passes.smooth.texture("uPrevLuma", 1, pyramids[previous][0]!!.texture, GLES20.GL_LINEAR)
        passes.smooth.texture("uCurLuma", 2, pyramids[latest][0]!!.texture, GLES20.GL_LINEAR)
        passes.smooth.set("uTexel", 1f / flow.width, 1f / flow.height)
        passes.smooth.draw()

        error!!.focus()
        passes.error.use()
        passes.error.texture("uCosts", 0, flow.texture, GLES20.GL_NEAREST)
        passes.error.draw()
    }

    private fun searchStep(
        pass: GlPass,
        into: GlTarget,
        previous: Int,
        latest: Int,
        level: Int,
        init: GlTarget?,
        radius: Int,
        step: Float,
    ) {
        into.focus()
        pass.use()
        pass.texture("uPrev", 0, pyramids[previous][level]!!.texture, GLES20.GL_LINEAR)
        pass.texture("uCur", 1, pyramids[latest][level]!!.texture, GLES20.GL_LINEAR)
        // Media3's frames have a texture on every unit the pass reads: an empty init reads its own target's.
        pass.texture("uInit", 2, (init ?: pyramids[latest][level]!!).texture, GLES20.GL_LINEAR)
        pass.set("uHasInit", if (init != null) 1f else 0f)
        pass.set("uTexel", 1f / into.width, 1f / into.height)
        pass.set("uScale", (1 shl level).toFloat())
        pass.set("uStep", step)
        pass.set("uRadius", radius.toFloat())
        pass.draw()
    }

    override fun signalEndOfCurrentInputStream() {
        ended = true
        try {
            pump()
        } catch (failure: Exception) {
            fail(failure, lastSentPts)
        }
    }

    override fun flush() {
        sends.clear()
        waiting.clear()
        passed.clear()
        for (output in outputs) output.busy = false
        keptCount = 0
        lastInputPts = Long.MIN_VALUE
        lastSentPts = Long.MIN_VALUE
        ended = false
        inputListener.onFlush()
        inputListener.onReadyToAcceptInputFrame()
    }

    /**
     * The passes failed: from here frames go through as they are (a playback error only when even
     * that fails).
     */
    private fun fail(failure: Exception, pts: Long) {
        if (status.gaveUp) {
            errorExecutor.execute { errorListener.onError(VideoFrameProcessingException.from(failure, pts)) }
            return
        }
        Log.w(EFFECTS_TAG, "smooth motion failed; frames go on as they are", failure)
        status.gaveUp = true
        // What was waiting goes on as it is.
        val held = ArrayList(waiting)
        waiting.clear()
        sends.clear()
        keptCount = 0
        for ((input, at) in held) {
            passed[input.texId] = input
            outputListener.onOutputFrameAvailable(input, at)
        }
    }

    private fun deletePyramids() {
        for (index in 0..1) {
            for (level in 0 until MotionMath.LEVELS) {
                pyramids[index][level]?.delete()
                pyramids[index][level] = null
            }
        }
        for (level in 0 until MotionMath.LEVELS) {
            search[level]?.delete()
            search[level] = null
        }
        refined?.delete()
        refinedMore?.delete()
        smoothed?.delete()
        error?.delete()
        refined = null
        refinedMore = null
        smoothed = null
        error = null
        pyramidQuality = -1
    }

    private fun deleteFrames() {
        for (output in outputs) output.target.delete()
        outputs = emptyList()
        for (index in 0..1) {
            kept[index]?.delete()
            kept[index] = null
        }
        deletePyramids()
    }

    override fun release() {
        try {
            deleteFrames()
            passes?.delete()
            passes = null
        } catch (failure: Exception) {
            throw VideoFrameProcessingException(failure)
        }
    }
}

/**
 * Smooth motion's programs: its own passes from the Smooth Motion chip's folder, the shared parts
 * (vertex, common) from the plugin's assets (the shared `glsl/` folder).
 */
private class MotionPasses(context: Context, dir: String?) {
    val copy: GlPass
    val luma: GlPass
    val search: GlPass
    val smooth: GlPass
    val error: GlPass
    val warp: GlPass

    init {
        val assets = context.assets
        fun read(name: String) = assets.open(name).bufferedReader().use { it.readText() }
        val folder = File(dir ?: throw IllegalStateException("the Smooth Motion chip isn't installed"))
        fun chip(name: String): String {
            val file = File(folder, name)
            if (!file.canRead()) throw java.io.FileNotFoundException("the Smooth Motion chip has no readable $name in $folder")
            return file.readText()
        }
        val vertex = read("fx_vertex.glsl")
        val common = read("fx_common.glsl") + "\n" + chip("fx_motion_common.glsl")
        fun fragment(name: String) = common + "\n" + chip(name)
        copy = GlPass(vertex, fragment("fx_motion_copy.glsl"))
        luma = GlPass(vertex, fragment("fx_motion_luma.glsl"))
        search = GlPass(vertex, fragment("fx_motion_search.glsl"))
        smooth = GlPass(vertex, fragment("fx_motion_smooth.glsl"))
        error = GlPass(vertex, fragment("fx_motion_error.glsl"))
        warp = GlPass(vertex, fragment("fx_motion_warp.glsl"))
    }

    fun delete() {
        for (pass in listOf(copy, luma, search, smooth, error, warp)) pass.delete()
    }
}

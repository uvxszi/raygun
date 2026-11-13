package app.raygun.audio

import android.content.Context
import android.opengl.GLES20
import android.os.SystemClock
import android.util.Log
import androidx.media3.common.VideoFrameProcessingException
import androidx.media3.common.util.GlUtil
import androidx.media3.common.util.Size
import androidx.media3.common.util.UnstableApi
import androidx.media3.effect.BaseGlShaderProgram
import androidx.media3.effect.GlEffect
import androidx.media3.effect.GlShaderProgram
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.util.Locale

internal const val EFFECTS_TAG = "RaygunEffects"

/** The deflicker's grid of blocks (`fx_stats_blocks.glsl`). */
private const val BLOCKS_X = 16
private const val BLOCKS_Y = 9

/** A frame's cost is measured once in this many frames (about every 3 s at 30 fps). */
private const val MEASURE_EVERY = 90L

/** HDR's linear light is passed through as it is, never clipped at 1. */
private const val HDR_LIMIT = 10_000f

/**
 * What the effect passes read each frame. Set on the main thread (a slider moves, a title loads),
 * read on Media3's GL thread, so changes apply from the next frame without rebuilding the chain.
 */
internal class EffectParams {
    @Volatile
    var shader: ShaderParams = EffectMath.shader(PictureEffects.NONE)
}

/**
 * How the effects do on this phone, written on the GL thread. Now and then a frame's passes are
 * timed between two `glFinish` calls (the work before them done first), so the time is the GPU's
 * for these passes; the first measurements and every 20th after are logged
 * (`adb logcat -s RaygunEffects`).
 */
internal class EffectStatus {
    /** A moving mean of the measured frames, in ms; 0 before the first. */
    @Volatile
    var averageMs: Double = 0.0
        private set

    @Volatile
    var samples: Int = 0
        private set

    /** The video is HDR: its frames pass through unchanged (the effects are made for SDR). */
    @Volatile
    var hdr: Boolean = false

    /** The extra passes failed on this GPU: only the colours apply. */
    @Volatile
    var limited: Boolean = false

    private var frames = 0L

    /**
     * Whether to time this frame: the 6th, then once in [MEASURE_EVERY], and after the first few
     * once in ten times that (each timing waits for the GPU twice).
     */
    fun measureNow(): Boolean {
        val frame = frames++
        val every = if (frame < MEASURE_EVERY * 4) MEASURE_EVERY else MEASURE_EVERY * 10
        return frame % every == 5L
    }

    fun record(nanos: Long, width: Int, height: Int, passes: String) {
        val ms = nanos / 1_000_000.0
        averageMs = if (samples == 0) ms else averageMs * 0.7 + ms * 0.3
        samples += 1
        if (samples <= 3 || samples % 20 == 0) {
            Log.i(
                EFFECTS_TAG,
                String.format(Locale.ROOT, "effects: %.2f ms a frame (mean %.2f) at %dx%d, passes %s", ms, averageMs, width, height, passes),
            )
        }
    }

    /** Other effects or another video: measured afresh. */
    fun reset() {
        frames = 0
        samples = 0
        averageMs = 0.0
    }
}

/** One full-frame quad drawn with a program of the shared shaders. */
private class GlPass(vertex: String, fragment: String) {
    private val program: Int = link(vertex, fragment)
    private val position = GLES20.glGetAttribLocation(program, "aFramePosition")
    private val locations = HashMap<String, Int>()

    fun use() {
        GLES20.glUseProgram(program)
        // Client-side arrays: no buffer may be bound, or the pointer would read as an offset.
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, 0)
        GLES20.glEnableVertexAttribArray(position)
        GLES20.glVertexAttribPointer(position, 4, GLES20.GL_FLOAT, false, 0, QUAD)
    }

    private fun at(name: String): Int = locations.getOrPut(name) { GLES20.glGetUniformLocation(program, name) }

    fun set(name: String, value: Float) = GLES20.glUniform1f(at(name), value)

    fun set(name: String, x: Float, y: Float) = GLES20.glUniform2f(at(name), x, y)

    fun set(name: String, x: Float, y: Float, z: Float) = GLES20.glUniform3f(at(name), x, y, z)

    fun matrix3(name: String, columns: FloatArray) = GLES20.glUniformMatrix3fv(at(name), 1, false, columns, 0)

    /** Binds `texture` to `unit` for `name`, sampled `filter` (linear for taps between pixels). */
    fun texture(name: String, unit: Int, texture: Int, filter: Int) {
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0 + unit)
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texture)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, filter)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, filter)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glUniform1i(at(name), unit)
    }

    fun draw() {
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        GlUtil.checkGlError()
    }

    fun delete() = GLES20.glDeleteProgram(program)

    private companion object {
        val QUAD: FloatBuffer = ByteBuffer.allocateDirect(16 * 4).order(ByteOrder.nativeOrder()).asFloatBuffer().apply {
            put(floatArrayOf(-1f, -1f, 0f, 1f, 1f, -1f, 0f, 1f, -1f, 1f, 0f, 1f, 1f, 1f, 0f, 1f))
            position(0)
        }

        fun compile(type: Int, source: String): Int {
            val shader = GLES20.glCreateShader(type)
            GLES20.glShaderSource(shader, source)
            GLES20.glCompileShader(shader)
            val status = IntArray(1)
            GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, status, 0)
            if (status[0] != GLES20.GL_TRUE) {
                val log = GLES20.glGetShaderInfoLog(shader)
                GLES20.glDeleteShader(shader)
                throw GlUtil.GlException("effect shader: $log")
            }
            return shader
        }

        fun link(vertex: String, fragment: String): Int {
            val program = GLES20.glCreateProgram()
            val vertexShader = compile(GLES20.GL_VERTEX_SHADER, vertex)
            val fragmentShader = compile(GLES20.GL_FRAGMENT_SHADER, fragment)
            GLES20.glAttachShader(program, vertexShader)
            GLES20.glAttachShader(program, fragmentShader)
            GLES20.glLinkProgram(program)
            GLES20.glDeleteShader(vertexShader)
            GLES20.glDeleteShader(fragmentShader)
            val status = IntArray(1)
            GLES20.glGetProgramiv(program, GLES20.GL_LINK_STATUS, status, 0)
            if (status[0] != GLES20.GL_TRUE) {
                val log = GLES20.glGetProgramInfoLog(program)
                GLES20.glDeleteProgram(program)
                throw GlUtil.GlException("effect program: $log")
            }
            return program
        }
    }
}

/** A texture the passes draw into, and its framebuffer. */
private class Target(val width: Int, val height: Int, highPrecision: Boolean = false) {
    val texture: Int = GlUtil.createTexture(width, height, highPrecision)
    val framebuffer: Int = GlUtil.createFboForTexture(texture)

    fun focus() {
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, framebuffer)
        GLES20.glViewport(0, 0, width, height)
    }

    fun delete() {
        GlUtil.deleteFbo(framebuffer)
        GlUtil.deleteTexture(texture)
    }
}

/**
 * The picture's effects as one Media3 effect ([PlayerEffects] adds it with `setVideoEffects`). Its
 * passes are the shared GLSL in the plugin's assets (`apps/raygun/src/video/effects/glsl`, the
 * desktop player's too): the deflicker's light (16 × 9 blocks, then 1 × 1), denoising into a
 * history kept for the next frame, then the final look into Media3's output texture. The values
 * are read from [EffectParams] each frame, so the chain is built once per player.
 */
@UnstableApi
internal class PictureEffectsGl(private val params: EffectParams, private val status: EffectStatus) : GlEffect {
    override fun toGlShaderProgram(context: Context, useHdr: Boolean): GlShaderProgram =
        PictureEffectsProgram(context, params, status, useHdr)
}

@UnstableApi
private class PictureEffectsProgram(
    context: Context,
    private val params: EffectParams,
    private val status: EffectStatus,
    private val hdr: Boolean,
) : BaseGlShaderProgram(hdr, 1) {
    private val blocks: GlPass
    private val mean: GlPass
    private val clean: GlPass
    private val final: GlPass
    private var width = 0
    private var height = 0

    // The deflicker's light: two 1 × 1 targets used in turn (the last frame's read, this one's drawn).
    private var blockTarget: Target? = null
    private val stats = arrayOfNulls<Target>(2)
    private var statsIndex = 0
    private var statsKnown = false

    // Denoising's frames, used in turn: the one drawn is the next frame's history.
    private val cleaned = arrayOfNulls<Target>(2)
    private var cleanIndex = 0
    private var historyKnown = false

    /** The last frame's time: the same again is the same frame drawn again. */
    private var lastTimeUs = Long.MIN_VALUE

    /** The extra passes failed here: the colours only from then on. */
    private var limited = false

    init {
        status.hdr = hdr
        try {
            val assets = context.assets
            fun read(name: String) = assets.open(name).bufferedReader().use { it.readText() }
            val vertex = read("fx_vertex.glsl")
            val common = read("fx_common.glsl")
            fun fragment(name: String) = common + "\n" + read(name)
            blocks = GlPass(vertex, fragment("fx_stats_blocks.glsl"))
            mean = GlPass(vertex, fragment("fx_stats_mean.glsl"))
            clean = GlPass(vertex, fragment("fx_clean.glsl"))
            final = GlPass(vertex, fragment("fx_final.glsl"))
        } catch (error: Exception) {
            throw VideoFrameProcessingException(error)
        }
    }

    override fun configure(inputWidth: Int, inputHeight: Int): Size {
        if (inputWidth != width || inputHeight != height) {
            width = inputWidth
            height = inputHeight
            deleteFrames()
        }
        return Size(inputWidth, inputHeight)
    }

    override fun drawFrame(inputTexId: Int, presentationTimeUs: Long) {
        // HDR passes through: the effects' light and colour are made for SDR.
        val shader = if (hdr) PASS_THROUGH else params.shader
        try {
            // Where Media3 wants the result: its output texture's framebuffer, focused for us.
            val output = IntArray(1)
            GLES20.glGetIntegerv(GLES20.GL_FRAMEBUFFER_BINDING, output, 0)
            val again = presentationTimeUs == lastTimeUs
            lastTimeUs = presentationTimeUs
            val measuring = !hdr && status.measureNow()
            val started = if (measuring) {
                GLES20.glFinish()
                SystemClock.elapsedRealtimeNanos()
            } else {
                0L
            }
            var source = inputTexId
            var gainLeft = shader.deflicker && !limited
            if (!limited) {
                if (shader.deflicker) drawLight(inputTexId, shader, again) else statsKnown = false
                if (shader.denoise) {
                    source = drawClean(inputTexId, shader, again)
                    gainLeft = false
                } else {
                    historyKnown = false
                }
            }
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, output[0])
            GLES20.glViewport(0, 0, width, height)
            drawFinal(source, shader, gainLeft)
            if (measuring) {
                GLES20.glFinish()
                status.record(SystemClock.elapsedRealtimeNanos() - started, width, height, shader.passes)
            }
        } catch (error: Exception) {
            if (limited) throw VideoFrameProcessingException(error, presentationTimeUs)
            // The extra passes failed (a driver without something they need): from here on the
            // frames get the colours only.
            Log.w(EFFECTS_TAG, "effect passes failed; colours only from here", error)
            limited = true
            status.limited = true
            drawFrame(inputTexId, presentationTimeUs)
        }
    }

    /** The deflicker's light: this frame's mean luma and the steady light it's held to. */
    private fun drawLight(frame: Int, shader: ShaderParams, again: Boolean) {
        val grid = blockTarget ?: Target(BLOCKS_X, BLOCKS_Y).also { blockTarget = it }
        if (stats[0] == null) {
            stats[0] = Target(1, 1)
            stats[1] = Target(1, 1)
            statsKnown = false
        }
        // The same frame again goes from the same light before, not from its own.
        if (!again) statsIndex = 1 - statsIndex
        val drawn = stats[statsIndex]!!
        val before = stats[1 - statsIndex]!!

        grid.focus()
        blocks.use()
        blocks.texture("uFrame", 0, frame, GLES20.GL_LINEAR)
        blocks.draw()

        drawn.focus()
        mean.use()
        mean.texture("uBlocks", 0, grid.texture, GLES20.GL_NEAREST)
        mean.texture("uBefore", 1, before.texture, GLES20.GL_NEAREST)
        mean.set("uFollow", shader.follow)
        mean.set("uFresh", if (statsKnown) 0f else 1f)
        mean.draw()
        statsKnown = true
    }

    /** Denoising into this frame's target, which the final pass then reads. */
    private fun drawClean(frame: Int, shader: ShaderParams, again: Boolean): Int {
        if (cleaned[0] == null) {
            cleaned[0] = Target(width, height, hdr)
            cleaned[1] = Target(width, height, hdr)
            historyKnown = false
        }
        if (!again) cleanIndex = 1 - cleanIndex
        val drawn = cleaned[cleanIndex]!!
        val history = cleaned[1 - cleanIndex]!!
        val light = statsTexture().takeIf { shader.deflicker }
        drawn.focus()
        clean.use()
        clean.texture("uFrame", 0, frame, GLES20.GL_LINEAR)
        clean.texture("uHistory", 1, history.texture, GLES20.GL_NEAREST)
        light?.let { clean.texture("uStats", 2, it, GLES20.GL_NEAREST) }
        clean.set("uTexel", 1f / width, 1f / height)
        clean.set("uDeflicker", if (light != null) 1f else 0f)
        clean.set("uSpatial", shader.spatial)
        clean.set("uSpatialSigma", shader.spatialSigma)
        clean.set("uTemporal", if (historyKnown) shader.temporal else 0f)
        clean.set("uTemporalSigma", shader.temporalSigma)
        clean.draw()
        historyKnown = true
        return drawn.texture
    }

    private fun drawFinal(source: Int, shader: ShaderParams, gain: Boolean) {
        final.use()
        final.texture("uFrame", 0, source, GLES20.GL_LINEAR)
        val light = statsTexture().takeIf { gain }
        light?.let { final.texture("uStats", 1, it, GLES20.GL_NEAREST) }
        final.set("uTexel", 1f / width, 1f / height)
        final.set("uDeflicker", if (light != null) 1f else 0f)
        final.set("uSharpen", if (limited) 0f else shader.sharpen)
        final.matrix3("uColour", shader.colour.columns())
        val offset = shader.colour.offset
        final.set("uOffset", offset[0].toFloat(), offset[1].toFloat(), offset[2].toFloat())
        final.set("uGamma", shader.gamma)
        final.set("uLimit", if (hdr) HDR_LIMIT else 1f)
        final.draw()
    }

    private fun statsTexture(): Int? = stats[statsIndex]?.texture?.takeIf { statsKnown }

    override fun flush() {
        // A seek: the frames before it are no history for the ones after.
        statsKnown = false
        historyKnown = false
        lastTimeUs = Long.MIN_VALUE
        super.flush()
    }

    private fun deleteFrames() {
        for (index in 0..1) {
            cleaned[index]?.delete()
            cleaned[index] = null
        }
        historyKnown = false
    }

    override fun release() {
        super.release()
        try {
            deleteFrames()
            blockTarget?.delete()
            blockTarget = null
            for (index in 0..1) {
                stats[index]?.delete()
                stats[index] = null
            }
            blocks.delete()
            mean.delete()
            clean.delete()
            final.delete()
        } catch (error: GlUtil.GlException) {
            throw VideoFrameProcessingException(error)
        }
    }

    private companion object {
        val PASS_THROUGH: ShaderParams = EffectMath.shader(PictureEffects.NONE)
    }
}

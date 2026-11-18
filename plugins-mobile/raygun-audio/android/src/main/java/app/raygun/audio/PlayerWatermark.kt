package app.raygun.audio

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.opengl.GLES20
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.widget.FrameLayout
import android.widget.TextView
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/** Box samples are taken once in this many new frames. */
private const val SAMPLE_EVERY = 3

/** The mask is estimated again once in this many samples. */
private const val ESTIMATE_EVERY = 8

/** Finding the overlay samples the whole picture once in this many new frames. */
private const val FIND_EVERY = 4

/**
 * The watermark remover's shared state: set on the main thread (the sheet), read on Media3's GL
 * thread ([WatermarkGl]), which reports back through the callbacks.
 */
internal class WatermarkState {
    /** Finding was asked for: the GL thread samples the picture until it knows. */
    @Volatile
    var finding = false

    /** A mask was estimated for the box in use. */
    @Volatile
    var masked = false

    /** What finding concluded, called on the GL thread. */
    @Volatile
    var onFound: ((FindVerdict) -> Unit)? = null

    /** Scene changes finding has seen (an answer needs two). */
    @Volatile
    var cuts = 0

    /** Another scene change seen while finding, called on the GL thread. */
    @Volatile
    var onProgress: (() -> Unit)? = null

    /** The first mask for a box, called on the GL thread. */
    @Volatile
    var onMasked: (() -> Unit)? = null
}

/**
 * The watermark remover's passes on Media3's GL thread, called by the picture's effects before
 * their own: every few frames the box and a ring round it are drawn small and read back for
 * [WatermarkEstimator], whose mask (`fx_watermark.glsl`'s `uMask`) then inverts the overlay's
 * blend into a frame-sized target the other passes read. Finding samples the whole picture for
 * [OverlayFinder].
 */
internal class WatermarkGl(context: Context, private val state: WatermarkState) {
    private val remove: GlPass
    private val sample: GlPass

    private var output: GlTarget? = null
    private var sampleTarget: GlTarget? = null
    private var mask = 0
    private var hasMask = false

    private var box = WatermarkRemoval.NONE
    private var layout: SampleLayout? = null
    private var estimator: WatermarkEstimator? = null
    private var frameWidth = 0
    private var frameHeight = 0
    private var frames = 0L
    private var finder: OverlayFinder? = null

    init {
        val assets = context.assets
        fun read(name: String) = assets.open(name).bufferedReader().use { it.readText() }
        val vertex = read("fx_vertex.glsl")
        val common = read("fx_common.glsl")
        remove = GlPass(vertex, common + "\n" + read("fx_watermark.glsl"))
        sample = GlPass(vertex, common + "\n" + read("fx_watermark_sample.glsl"))
    }

    /**
     * The frame with the watermark taken out (its own texture), or `input` when there's nothing
     * to do yet. `again` is the same frame drawn again (nothing new to learn from).
     */
    fun apply(input: Int, width: Int, height: Int, removal: WatermarkRemoval, again: Boolean): Int {
        if (!again) frames += 1
        if (state.finding) find(input, again) else finder = null
        if (!removal.active) {
            forget()
            return input
        }
        if (!removal.sameBox(box) || width != frameWidth || height != frameHeight || layout == null) {
            forget()
            box = removal
            frameWidth = width
            frameHeight = height
            val next = WatermarkMath.sampleLayout(removal, width, height)
            layout = next
            estimator = WatermarkEstimator(next)
            output?.delete()
            output = null
        }
        val sampling = layout!!
        val sums = estimator!!
        if (!again && frames % SAMPLE_EVERY == 0L) {
            val rect = sampling.rect
            sums.add(
                readBack(
                    input,
                    floatArrayOf(
                        (rect.left / width).toFloat(),
                        (1 - (rect.top + rect.height) / height).toFloat(),
                        ((rect.left + rect.width) / width).toFloat(),
                        (1 - rect.top / height).toFloat(),
                    ),
                    sampling.cols,
                    sampling.rows,
                ),
            )
            if (sums.samples.toInt() % ESTIMATE_EVERY == 0) sums.estimate()?.let(::upload)
        }
        if (!hasMask) return input
        box = removal
        val target = output ?: GlTarget(width, height).also { output = it }
        target.focus()
        remove.use()
        remove.texture("uFrame", 0, input, GLES20.GL_LINEAR)
        remove.texture("uMask", 1, mask, GLES20.GL_LINEAR)
        remove.set("uTexel", 1f / width, 1f / height)
        val shaderBox = WatermarkMath.shaderBox(removal)
        remove.set4("uBox", shaderBox)
        remove.set("uFeather", WatermarkMath.feather(removal.feather).toFloat())
        remove.set("uBright", WatermarkMath.brightness(removal.brightness).toFloat())
        remove.set("uContrast", WatermarkMath.contrast(removal.contrast).toFloat())
        remove.draw()
        return target.texture
    }

    /** The whole picture, small, for [OverlayFinder]; the answer once there are enough samples. */
    private fun find(input: Int, again: Boolean) {
        val looking = finder ?: OverlayFinder().also { finder = it }
        if (again || frames % FIND_EVERY != 0L) return
        val before = looking.cuts
        looking.add(readBack(input, floatArrayOf(0f, 0f, 1f, 1f), OverlayFinder.COLS, OverlayFinder.ROWS))
        if (looking.cuts != before) {
            state.cuts = looking.cuts
            state.onProgress?.invoke()
        }
        if (!looking.ready) return
        state.finding = false
        finder = null
        state.onFound?.invoke(looking.verdict())
    }

    /** `rect` of the frame drawn into a `cols × rows` grid and read back, rows from the top. */
    private fun readBack(input: Int, rect: FloatArray, cols: Int, rows: Int): ByteArray {
        val target = sampleTarget?.takeIf { it.width == cols && it.height == rows }
            ?: GlTarget(cols, rows).also {
                sampleTarget?.delete()
                sampleTarget = it
            }
        target.focus()
        sample.use()
        sample.texture("uFrame", 0, input, GLES20.GL_LINEAR)
        sample.set4("uRect", rect)
        sample.draw()
        val buffer = ByteBuffer.allocateDirect(cols * rows * 4).order(ByteOrder.nativeOrder())
        GLES20.glReadPixels(0, 0, cols, rows, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, buffer)
        // OpenGL's rows start at the bottom; the estimators read from the top.
        val bottomUp = ByteArray(cols * rows * 4)
        buffer.get(bottomUp)
        val topDown = ByteArray(bottomUp.size)
        val stride = cols * 4
        for (row in 0 until rows) System.arraycopy(bottomUp, row * stride, topDown, (rows - 1 - row) * stride, stride)
        return topDown
    }

    private fun upload(found: WatermarkMask) {
        if (mask == 0) {
            val ids = IntArray(1)
            GLES20.glGenTextures(1, ids, 0)
            mask = ids[0]
        }
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, mask)
        val buffer = ByteBuffer.allocateDirect(found.data.size).order(ByteOrder.nativeOrder())
        buffer.put(found.data).position(0)
        GLES20.glTexImage2D(
            GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, found.cols, found.rows, 0,
            GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, buffer,
        )
        val first = !hasMask
        hasMask = true
        state.masked = true
        if (first) state.onMasked?.invoke()
    }

    /** Another box (or none): what was learnt goes. */
    private fun forget() {
        layout = null
        estimator = null
        hasMask = false
        state.masked = false
        box = WatermarkRemoval.NONE
    }

    fun release() {
        output?.delete()
        sampleTarget?.delete()
        if (mask != 0) GLES20.glDeleteTextures(1, intArrayOf(mask), 0)
        remove.delete()
        sample.delete()
    }
}

/**
 * Drawing the watermark's box over the picture, the player's controls and bars hidden meanwhile:
 * a drag draws it (another drag draws it again), Done keeps it (in ten-thousandths of the picture
 * as shown), Cancel or Back leaves the box as it was.
 */
@SuppressLint("ViewConstructor")
internal class WatermarkDrawView(
    context: Context,
    hint: String,
    doneLabel: String,
    cancelLabel: String,
    /** The picture's place, in screen pixels. */
    private val picture: () -> RectF?,
    private val done: (WatermarkRemoval?) -> Unit,
) : FrameLayout(context) {
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = context.dp(2).toFloat()
        color = playerAccent
    }
    private val fill = Paint().apply { color = 0x33FFFFFF }
    private var startX = 0f
    private var startY = 0f
    private var box: RectF? = null
    private var answered = false
    private val doneButton = context.pill(doneLabel, filled = true) { finish(kept()) }

    init {
        setWillNotDraw(false)
        setBackgroundColor(0x55000000)
        isClickable = true
        addView(
            TextView(context).apply {
                text = hint
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                setPadding(context.dp(16), context.dp(10), context.dp(16), context.dp(10))
                background = rounded(0x99000000.toInt(), context.dp(18).toFloat())
            },
            LayoutParams(-2, -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply { topMargin = context.dp(32) },
        )
        addView(
            android.widget.LinearLayout(context).apply {
                orientation = android.widget.LinearLayout.HORIZONTAL
                addView(context.pill(cancelLabel) { finish(null) })
                addView(doneButton)
            },
            LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL).apply { bottomMargin = context.dp(32) },
        )
        enableDone()
    }

    private fun enableDone() {
        val ready = kept() != null
        doneButton.isEnabled = ready
        doneButton.alpha = if (ready) 1f else 0.4f
    }

    /** The box drawn, in ten-thousandths of the picture; none while it's too small to keep. */
    private fun kept(): WatermarkRemoval? {
        val drawn = box ?: return null
        val shown = picture() ?: return null
        val small = context.dp(12)
        if (drawn.width() < small || drawn.height() < small) return null
        val origin = IntArray(2)
        getLocationOnScreen(origin)
        val onScreen = RectF(drawn).apply { offset(origin[0].toFloat(), origin[1].toFloat()) }
        return WatermarkDraw.toBox(onScreen, shown)
    }

    /** Back: the box stays as it was. */
    fun cancel() = finish(null)

    private fun finish(result: WatermarkRemoval?) {
        if (answered) return
        answered = true
        done(result)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                startX = event.x
                startY = event.y
            }
            MotionEvent.ACTION_MOVE -> {
                box = RectF(min(startX, event.x), min(startY, event.y), max(startX, event.x), max(startY, event.y))
                invalidate()
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> enableDone()
        }
        return true
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        box?.let {
            canvas.drawRect(it, fill)
            canvas.drawRect(it, stroke)
        }
    }
}

/**
 * What Find it found, outlined over the picture with "Remove this?": only Remove turns the remover
 * on, Not this leaves the title as it was.
 */
@SuppressLint("ViewConstructor")
internal class WatermarkProposalView(
    context: Context,
    private val box: WatermarkRemoval,
    /** The picture's place, in screen pixels. */
    private val picture: () -> RectF?,
    question: String,
    remove: String,
    notThis: String,
    answer: (Boolean) -> Unit,
) : FrameLayout(context) {
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = context.dp(3).toFloat()
        color = playerAccent
    }

    init {
        setWillNotDraw(false)
        isClickable = true
        val row = android.widget.LinearLayout(context).apply {
            orientation = android.widget.LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(16), context.dp(6), context.dp(8), context.dp(6))
            background = rounded(0xCC000000.toInt(), context.dp(24).toFloat())
            addView(TextView(context).apply {
                text = question
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            })
            addView(context.pill(remove, filled = true) { answer(true) })
            addView(context.pill(notThis) { answer(false) })
        }
        addView(row, LayoutParams(-2, -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply { topMargin = context.dp(32) })
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val shown = picture() ?: return
        val origin = IntArray(2)
        getLocationOnScreen(origin)
        val scale = WatermarkMath.SCALE.toFloat()
        val left = shown.left + box.x / scale * shown.width() - origin[0]
        val top = shown.top + box.y / scale * shown.height() - origin[1]
        canvas.drawRect(
            left,
            top,
            left + box.width / scale * shown.width(),
            top + box.height / scale * shown.height(),
            stroke,
        )
    }
}

/** Screen boxes as the watermark's (the same as the desktop's `watermarkBox.ts`). */
internal object WatermarkDraw {
    /** `drawn` (screen pixels) within the picture shown at `picture`, in ten-thousandths of it. */
    fun toBox(drawn: RectF, picture: RectF): WatermarkRemoval? {
        if (picture.width() <= 0 || picture.height() <= 0) return null
        fun x(value: Float) = ((value - picture.left) / picture.width()).coerceIn(0f, 1f)
        fun y(value: Float) = ((value - picture.top) / picture.height()).coerceIn(0f, 1f)
        val scale = WatermarkMath.SCALE.toFloat()
        val left = WatermarkMath.roundHalfUp((x(drawn.left) * scale).toDouble())
        val top = WatermarkMath.roundHalfUp((y(drawn.top) * scale).toDouble())
        val right = WatermarkMath.roundHalfUp((x(drawn.right) * scale).toDouble())
        val bottom = WatermarkMath.roundHalfUp((y(drawn.bottom) * scale).toDouble())
        if (abs(right - left) < 20 || abs(bottom - top) < 20) return null
        return WatermarkRemoval(enabled = true, x = left, y = top, width = right - left, height = bottom - top)
    }
}

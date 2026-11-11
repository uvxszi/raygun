package app.raygun.audio

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.RadialGradient
import android.graphics.Shader
import android.graphics.drawable.GradientDrawable
import android.view.View
import android.view.ViewGroup
import kotlin.math.roundToInt
import kotlin.math.sqrt

/** How dark the shade gets at the top edge, the bottom edge and the corners (0 to 1). */
private const val TOP_ALPHA = 0.6f
private const val BOTTOM_ALPHA = 0.7f
private const val CORNER_ALPHA = 0.25f
/** Stops along each gradient: enough that the eased curve shows no bands. */
private const val STOPS = 16
/** Where the corners start to darken, as a share of the way from the middle to a corner. */
private const val CORNER_START = 0.5f
private const val FADE_MS = 150L

/** 1 at `t` = 0, 0 at 1, flat at both ends: no line shows where a shade stops. */
private fun easedOut(t: Float): Float {
    val x = t.coerceIn(0f, 1f)
    return 1f - x * x * (3f - 2f * x)
}

private fun black(alpha: Float): Int = Color.argb((alpha.coerceIn(0f, 1f) * 255).roundToInt(), 0, 0, 0)

/**
 * The shade behind the player's controls, so they read over a bright picture: it darkens towards
 * the top and bottom edges and a little towards the corners. Each part fades out along an eased
 * curve that ends flat, so no line shows where it stops, and each is sized to the window (the
 * edges' shades by its height, the corners' by its shape), so it stretches with the view in
 * portrait, landscape and any aspect. It is laid out on the whole window, edge to edge, ignoring
 * insets: under the cutout and the navigation bar too. Only the controls keep to the safe area.
 * It shows with the controls.
 */
internal class PlayerVignette(context: Context) : View(context) {
    private val top = Paint()
    private val bottom = Paint()
    private val corners = Paint()
    private var topLength = 0f
    private var bottomLength = 0f

    init {
        visibility = GONE
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
        fitsSystemWindows = false
    }

    /** Shown with the controls; fades in and out where animations are on. */
    var shown = false
        set(value) {
            if (field == value) return
            field = value
            animate().cancel()
            if (!DeviceState.animatorsEnabled()) {
                alpha = 1f
                visibility = if (value) VISIBLE else GONE
                return
            }
            if (value) {
                if (visibility != VISIBLE) {
                    alpha = 0f
                    visibility = VISIBLE
                }
                animate().alpha(1f).setDuration(FADE_MS).start()
            } else {
                animate().alpha(0f).setDuration(FADE_MS).withEndAction { if (!shown) visibility = GONE }.start()
            }
        }

    override fun onSizeChanged(width: Int, height: Int, oldWidth: Int, oldHeight: Int) {
        super.onSizeChanged(width, height, oldWidth, oldHeight)
        if (width <= 0 || height <= 0) return
        val h = height.toFloat()
        topLength = (h * 0.28f).coerceIn(context.dp(96).toFloat(), context.dp(320).toFloat()).coerceAtMost(h)
        bottomLength = (h * 0.36f).coerceIn(context.dp(128).toFloat(), context.dp(400).toFloat()).coerceAtMost(h)
        val steps = FloatArray(STOPS) { it / (STOPS - 1f) }
        top.shader = LinearGradient(
            0f, 0f, 0f, topLength,
            IntArray(STOPS) { black(TOP_ALPHA * easedOut(steps[it])) }, steps, Shader.TileMode.CLAMP,
        )
        bottom.shader = LinearGradient(
            0f, h, 0f, h - bottomLength,
            IntArray(STOPS) { black(BOTTOM_ALPHA * easedOut(steps[it])) }, steps, Shader.TileMode.CLAMP,
        )
        // An ellipse through the corners with the window's own shape: a circle as wide as the
        // window's diagonal reach, squeezed to its height.
        val centreX = width / 2f
        val centreY = h / 2f
        val radius = width / sqrt(2f)
        val cornerColors = IntArray(STOPS) { index ->
            val reach = steps[index]
            val past = ((reach - CORNER_START) / (1f - CORNER_START)).coerceIn(0f, 1f)
            black(CORNER_ALPHA * (1f - easedOut(past)))
        }
        corners.shader = RadialGradient(centreX, centreY, radius, cornerColors, steps, Shader.TileMode.CLAMP).apply {
            setLocalMatrix(Matrix().apply { setScale(1f, h / width, centreX, centreY) })
        }
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat()
        val h = height.toFloat()
        canvas.drawRect(0f, 0f, w, h, corners)
        canvas.drawRect(0f, 0f, w, topLength, top)
        canvas.drawRect(0f, h - bottomLength, w, h, bottom)
    }

    companion object {
        /**
         * The controls' bars drop the short gradients behind them (they stopped at the bars'
         * edges and inside the cutout's inset): this shade covers the window instead.
         */
        fun takeOverShades(controls: ViewGroup) {
            for (index in 0 until controls.childCount) {
                val child = controls.getChildAt(index)
                val shade = child.background as? GradientDrawable ?: continue
                // A gradient (not a plain rounded backing, which has no colours of its own).
                if (shade.colors != null) child.background = null
            }
        }
    }
}

package app.raygun.audio

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.HorizontalScrollView
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.content.ContextCompat
import java.util.Locale
import kotlin.math.roundToLong

// The subtitle sheets' parts (M45, after v1.6.0): the tools that add subtitles, the typed delay,
// colour swatches and a colour of the viewer's own.

/** A colour offered for subtitles: 0xAARRGGBB, and its name's key in the player's words. */
internal class Swatch(val color: Int, val key: String, val fallback: String)

/** As `TEXT_COLORS`, `EDGE_COLORS` and `BACKGROUND_COLORS` in the app's `settings/playerOptions.ts`. */
internal val TEXT_SWATCHES = listOf(
    Swatch(0xFFFFFFFF.toInt(), "colorWhite", "White"),
    Swatch(0xFFFFEB3B.toInt(), "colorYellow", "Yellow"),
    Swatch(0xFF80DEEA.toInt(), "colorCyan", "Cyan"),
    Swatch(0xFFA5D6A7.toInt(), "colorGreen", "Green"),
    Swatch(0xFFBDBDBD.toInt(), "colorGrey", "Grey"),
    Swatch(0xFF000000.toInt(), "colorBlack", "Black"),
)
internal val EDGE_SWATCHES = listOf(
    Swatch(0xFF000000.toInt(), "colorBlack", "Black"),
    Swatch(0xFF424242.toInt(), "colorDarkGrey", "Dark grey"),
    Swatch(0xFFFFFFFF.toInt(), "colorWhite", "White"),
)
internal val BACKGROUND_SWATCHES = listOf(
    Swatch(0x00000000, "colorNone", "None"),
    Swatch(0x80000000.toInt(), "colorBlackHalf", "Half see-through"),
    Swatch(0xBF000000.toInt(), "colorBlackMost", "Slightly see-through"),
    Swatch(0xFF000000.toInt(), "colorBlack", "Black"),
    Swatch(0x80FFFFFF.toInt(), "colorWhiteHalf", "White, half see-through"),
)

/** Every hue around once, for the hue slider and the Custom pill. */
private val HUES = intArrayOf(
    0xFFFF0000.toInt(),
    0xFFFFFF00.toInt(),
    0xFF00FF00.toInt(),
    0xFF00FFFF.toInt(),
    0xFF0000FF.toInt(),
    0xFFFF00FF.toInt(),
    0xFFFF0000.toInt(),
)

/** The largest subtitle delay either way (ten minutes), as `setSubtitleDelay` keeps it. */
internal const val MAX_DELAY_MS = 600_000L

/** A typed delay in seconds ("1.25", "-0,5", "+2 s") as milliseconds; null when it isn't one. */
internal fun parseDelay(text: String): Long? {
    val plain = text.trim().removeSuffix("s").trim().replace(',', '.').replace('−', '-')
    val seconds = plain.toDoubleOrNull()?.takeIf { it.isFinite() } ?: return null
    return (seconds * 1000).roundToLong().coerceIn(-MAX_DELAY_MS, MAX_DELAY_MS)
}

/** A delay as the field shows it: "0.00", "-1.25" (seconds). */
internal fun formatDelay(ms: Long): String = String.format(Locale.ROOT, "%.2f", ms / 1000.0)

/** `#RRGGBB` (the colour without its opacity). */
internal fun hexColor(color: Int): String = String.format(Locale.ROOT, "#%06X", color and 0xFFFFFF)

/** A typed `#RRGGBB` (the # may be left out) as 0xRRGGBB; null when it isn't one. */
internal fun parseHexColor(text: String): Int? {
    val digits = text.trim().removePrefix("#")
    if (digits.length != 6 || digits.any { Character.digit(it, 16) < 0 }) return null
    return digits.toInt(16)
}

/**
 * A hue or saturation moved on a colour that can't show it (white, a grey, black): the colour
 * takes it at full saturation and brightness where it had none, so the sample and the subtitles
 * change as the slider moves. `hsv` is changed in place.
 */
internal fun showMovedHue(hsv: FloatArray, hueMoved: Boolean) {
    if (hueMoved && hsv[1] < 0.05f) hsv[1] = 1f
    if (hsv[2] < 0.05f) hsv[2] = 1f
}

/** A round sample of `color` with a faint ring, so black and see-through colours show too. */
private fun Context.swatchDot(color: Int, onLight: Boolean): GradientDrawable = GradientDrawable().apply {
    shape = GradientDrawable.OVAL
    setColor(color)
    setStroke(dp(1).coerceAtLeast(1), if (onLight) 0x66000000 else 0x99FFFFFF.toInt())
    setSize(dp(16), dp(16))
}

/** The Custom pill's sample: every hue around the ring. */
private fun Context.hueDot(): GradientDrawable = GradientDrawable().apply {
    shape = GradientDrawable.OVAL
    gradientType = GradientDrawable.SWEEP_GRADIENT
    colors = HUES
    setSize(dp(16), dp(16))
}

private fun TextView.withDot(dot: Drawable): TextView = apply {
    setCompoundDrawablesRelativeWithIntrinsicBounds(dot, null, null, null)
    compoundDrawablePadding = context.dp(8)
}

private fun Context.hideKeyboard(field: View) {
    (getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager)
        ?.hideSoftInputFromWindow(field.windowToken, 0)
    field.clearFocus()
}

/**
 * A row of colour pills, each with its sample beside its name, then Custom, which opens a colour
 * of the viewer's own ([ColorPicker]). The pill of the colour in use is filled; a colour that is
 * none of the swatches fills Custom, with its own sample.
 */
internal fun PlayerSheet.swatches(
    host: VideoPlayerActivity,
    swatches: List<Swatch>,
    current: Int,
    onPick: (Int) -> Unit,
    onCustom: () -> Unit,
) {
    val context: Context = host
    val line = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(context.dp(12), context.dp(6), context.dp(20), context.dp(6))
    }
    val known = swatches.any { it.color == current }
    swatches.forEach { swatch ->
        val selected = swatch.color == current
        line.addView(
            context.pill(host.label(swatch.key, swatch.fallback), filled = selected) { onPick(swatch.color) }
                .withDot(context.swatchDot(swatch.color, onLight = selected)),
        )
    }
    val sample = if (known) context.hueDot() else context.swatchDot(current, onLight = true)
    line.addView(context.pill(host.label("colorCustom", "Custom"), filled = !known) { onCustom() }.withDot(sample))
    custom(HorizontalScrollView(context).apply {
        isHorizontalScrollBarEnabled = false
        addView(line)
    })
}

/** One of the subtitle sheet's tools: adding a file, transcribing, translating. */
internal class SubtitleTool(val icon: Int, val name: String, val detail: String?, val onClick: () -> Unit)

/**
 * The tools that add subtitles, as buttons side by side at the top of the subtitle sheet (each
 * opens its own sheet), apart from the list of subtitles to show.
 */
internal fun PlayerSheet.subtitleTools(host: VideoPlayerActivity, tools: List<SubtitleTool>) {
    val context: Context = host
    val ripple = TypedValue().also { context.theme.resolveAttribute(android.R.attr.selectableItemBackground, it, true) }
    val row = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        isBaselineAligned = false
        setPadding(context.dp(16), context.dp(4), context.dp(16), context.dp(8))
    }
    tools.forEachIndexed { index, tool ->
        val tile = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(context.dp(8), context.dp(12), context.dp(8), context.dp(10))
            background = rounded(0x1FFFFFFF, context.dp(14).toFloat())
            foreground = ContextCompat.getDrawable(context, ripple.resourceId)
            isClickable = true
            isFocusable = true
            contentDescription = listOfNotNull(tool.name, tool.detail).joinToString(". ")
            setOnClickListener { tool.onClick() }
            addView(ImageView(context).apply {
                setImageResource(tool.icon)
                setColorFilter(Color.WHITE)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(context.dp(24), context.dp(24)))
            addView(TextView(context).apply {
                text = tool.name
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                gravity = Gravity.CENTER
                maxLines = 2
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(6) })
            tool.detail?.let { detail ->
                addView(TextView(context).apply {
                    text = detail
                    setTextColor(0xB3FFFFFF.toInt())
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
                    gravity = Gravity.CENTER
                    maxLines = 2
                    importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
                }, LinearLayout.LayoutParams(-1, -2).apply { topMargin = context.dp(2) })
            }
        }
        row.addView(tile, LinearLayout.LayoutParams(0, -1, 1f).apply { if (index > 0) marginStart = context.dp(8) })
    }
    custom(row)
}

/**
 * The subtitle delay: typed in seconds (decimals; negative shows them earlier), or stepped with
 * the pills under it. `set` gets the new delay in milliseconds.
 */
internal fun PlayerSheet.delayField(host: VideoPlayerActivity, delayMs: Long, set: (Long) -> Unit) {
    val context: Context = host
    val field = EditText(context).apply {
        setText(formatDelay(delayMs))
        contentDescription = host.label("delaySeconds", "Subtitle delay in seconds")
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        gravity = Gravity.CENTER
        inputType = InputType.TYPE_CLASS_NUMBER or
            InputType.TYPE_NUMBER_FLAG_DECIMAL or
            InputType.TYPE_NUMBER_FLAG_SIGNED
        imeOptions = EditorInfo.IME_ACTION_DONE
        isSingleLine = true
        setSelectAllOnFocus(true)
        backgroundTintList = android.content.res.ColorStateList.valueOf(playerAccent)
    }
    val commit = {
        val typed = parseDelay(field.text.toString())
        context.hideKeyboard(field)
        if (typed != null && typed != delayMs) set(typed) else field.setText(formatDelay(delayMs))
    }
    field.setOnEditorActionListener { _, action, _ ->
        val done = action == EditorInfo.IME_ACTION_DONE
        if (done) commit()
        done
    }
    custom(LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(context.dp(20), context.dp(4), context.dp(20), 0)
        addView(TextView(context).apply {
            text = host.label("delay", "Delay")
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
        }, LinearLayout.LayoutParams(0, -2, 1f))
        addView(field, LinearLayout.LayoutParams(context.dp(96), -2))
        addView(TextView(context).apply {
            text = host.label("secondsUnit", "s")
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(context.dp(6), 0, 0, 0)
        })
    })
    val steps = listOf(-1000L to "−1", -100L to "−0.1", 100L to "+0.1", 1000L to "+1")
    val line = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(context.dp(12), context.dp(2), context.dp(20), context.dp(6))
        steps.forEach { (step, name) ->
            addView(context.pill(name) { set((delayMs + step).coerceIn(-MAX_DELAY_MS, MAX_DELAY_MS)) })
        }
        addView(context.pill(host.label("reset", "Reset")) { set(0) }.apply { isEnabled = delayMs != 0L; alpha = if (delayMs != 0L) 1f else 0.4f })
    }
    custom(HorizontalScrollView(context).apply {
        isHorizontalScrollBarEnabled = false
        addView(line)
    })
}

/**
 * A slider drawn over a gradient (a colour's hue, saturation, brightness or opacity): `onMove`
 * as it moves, `onLet` once let go.
 */
private class GradientSlider(
    context: Context,
    private val onMove: (Float) -> Unit,
    private val onLet: () -> Unit,
) : View(context) {
    private val track = Paint(Paint.ANTI_ALIAS_FLAG)
    private val thumb = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val ring = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = context.dp(2).toFloat()
        color = 0x66000000
    }
    private val bar = RectF()
    private val inset = context.dp(16).toFloat()

    /** From 0 to 1. */
    var value = 0f
        set(next) {
            field = next.coerceIn(0f, 1f)
            invalidate()
        }

    var colors: IntArray = intArrayOf(Color.BLACK, Color.WHITE)
        set(next) {
            field = next
            track.shader = null
            invalidate()
        }

    init {
        isClickable = true
        isFocusable = true
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        setMeasuredDimension(
            getDefaultSize(suggestedMinimumWidth, widthMeasureSpec),
            resolveSize(context.dp(40), heightMeasureSpec),
        )
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val middle = height / 2f
        val half = context.dp(5).toFloat()
        bar.set(inset, middle - half, width - inset, middle + half)
        if (track.shader == null) {
            track.shader = LinearGradient(bar.left, 0f, bar.right, 0f, colors, null, Shader.TileMode.CLAMP)
        }
        canvas.drawRoundRect(bar, half, half, track)
        val x = bar.left + value * bar.width()
        val radius = context.dp(11).toFloat()
        canvas.drawCircle(x, middle, radius, thumb)
        canvas.drawCircle(x, middle, radius, ring)
    }

    override fun onSizeChanged(width: Int, height: Int, oldWidth: Int, oldHeight: Int) {
        super.onSizeChanged(width, height, oldWidth, oldHeight)
        track.shader = null
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // The sheet scrolls up and down; a slider moved sideways keeps the finger.
                parent?.requestDisallowInterceptTouchEvent(true)
                move(event.x)
            }
            MotionEvent.ACTION_MOVE -> move(event.x)
            MotionEvent.ACTION_UP -> {
                move(event.x)
                onLet()
            }
            MotionEvent.ACTION_CANCEL -> onLet()
        }
        return true
    }

    private fun move(x: Float) {
        val span = (width - 2 * inset).coerceAtLeast(1f)
        value = (x - inset) / span
        onMove(value)
    }
}

/**
 * Custom colour: a colour of the viewer's own for the subtitles' text, edge or background, picked
 * by hue, saturation and brightness (and opacity, for a background), or typed as #RRGGBB. The
 * subtitles show it as it changes; it is kept (`apply` with `save`) when a slider is let go, a
 * typed colour is done, or the sheet closes. Done goes `back` to the style sheet.
 */
internal class ColorPicker(
    private val host: VideoPlayerActivity,
    title: String,
    initial: Int,
    private val withOpacity: Boolean,
    private val apply: (color: Int, save: Boolean) -> Unit,
    private val back: () -> Unit,
) {
    private val context: Context = host
    private val sheet = PlayerSheet(host, title)
    private val hsv = FloatArray(3)
    /** A background starts half see-through when it was none. */
    private var alpha = if (!withOpacity) 0xFF else Color.alpha(initial).takeIf { it > 0 } ?: 0x80
    private var saved = initial
    private val sample = View(context)
    private val hex = EditText(context)
    private val hue = GradientSlider(context, {
        hsv[0] = it * 360f
        showMovedHue(hsv, hueMoved = true)
        changed()
    }, ::keep)
    private val saturation = GradientSlider(context, {
        hsv[1] = it
        showMovedHue(hsv, hueMoved = false)
        changed()
    }, ::keep)
    private val brightness = GradientSlider(context, { hsv[2] = it; changed() }, ::keep)
    private val opacity = GradientSlider(context, { alpha = (it * 255).toInt(); changed() }, ::keep)

    private val color: Int get() = Color.HSVToColor(alpha, hsv)

    init {
        Color.colorToHSV(initial, hsv)
        hue.colors = HUES
        hue.contentDescription = host.label("colorHue", "Hue")
        saturation.contentDescription = host.label("colorSaturation", "Saturation")
        brightness.contentDescription = host.label("colorBrightness", "Brightness")
        opacity.contentDescription = host.label("colorOpacity", "Opacity")
        hex.apply {
            contentDescription = host.label("colorCode", "Colour code")
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = android.graphics.Typeface.MONOSPACE
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            imeOptions = EditorInfo.IME_ACTION_DONE
            isSingleLine = true
            backgroundTintList = android.content.res.ColorStateList.valueOf(playerAccent)
            setOnEditorActionListener { _, action, _ ->
                val done = action == EditorInfo.IME_ACTION_DONE
                if (done) typed()
                done
            }
        }
        sheet.custom(LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(20), context.dp(4), context.dp(12), context.dp(4))
            addView(sample, LinearLayout.LayoutParams(context.dp(36), context.dp(36)))
            addView(hex, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = context.dp(16) })
            addView(context.pill(host.label("done", "Done")) {
                keep()
                back()
            })
        })
        slider(host.label("colorHue", "Hue"), hue)
        slider(host.label("colorSaturation", "Saturation"), saturation)
        slider(host.label("colorBrightness", "Brightness"), brightness)
        if (withOpacity) slider(host.label("colorOpacity", "Opacity"), opacity)
        // Closed with a tap outside or Back: what shows is kept.
        sheet.onDismiss = { keep() }
        refresh(updateHex = true)
    }

    fun show() = sheet.show()

    private fun slider(name: String, slider: GradientSlider) {
        sheet.header(name)
        sheet.custom(slider)
    }

    /** A slider moved: the subtitles show the colour, not kept yet. */
    private fun changed() {
        refresh(updateHex = true)
        apply(color, false)
    }

    /** Keeps the colour shown, once. */
    private fun keep() {
        val now = color
        if (now == saved) return
        saved = now
        apply(now, true)
    }

    private fun typed() {
        context.hideKeyboard(hex)
        val rgb = parseHexColor(hex.text.toString())
        if (rgb == null) {
            hex.setText(hexColor(color))
            return
        }
        Color.colorToHSV(rgb or (0xFF shl 24), hsv)
        refresh(updateHex = true)
        apply(color, false)
        keep()
    }

    /** The sample, the sliders' places and gradients, and the typed code follow the colour. */
    private fun refresh(updateHex: Boolean) {
        sample.background = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color)
            setStroke(context.dp(1).coerceAtLeast(1), 0x99FFFFFF.toInt())
        }
        hue.value = hsv[0] / 360f
        saturation.value = hsv[1]
        brightness.value = hsv[2]
        opacity.value = alpha / 255f
        saturation.colors = intArrayOf(
            Color.HSVToColor(floatArrayOf(hsv[0], 0f, hsv[2])),
            Color.HSVToColor(floatArrayOf(hsv[0], 1f, hsv[2])),
        )
        brightness.colors = intArrayOf(Color.BLACK, Color.HSVToColor(floatArrayOf(hsv[0], hsv[1], 1f)))
        val opaque = Color.HSVToColor(hsv)
        opacity.colors = intArrayOf(opaque and 0x00FFFFFF, opaque)
        if (updateHex && !hex.hasFocus()) hex.setText(hexColor(color))
    }
}

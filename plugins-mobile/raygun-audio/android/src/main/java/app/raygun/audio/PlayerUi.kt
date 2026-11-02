package app.raygun.audio

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.util.Locale

/** The player's accent (Raygun's red) and its panels' colour. */
internal const val ACCENT = 0xFFE53935.toInt()
internal const val PANEL = 0xF2202124.toInt()
internal const val SCRIM = 0x66000000

internal fun Context.dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

internal fun rounded(color: Int, radius: Float): GradientDrawable = GradientDrawable().apply {
    setColor(color)
    cornerRadius = radius
}

/** `1:02:03` or `2:03`. */
internal fun formatTime(ms: Long): String {
    val total = (ms.coerceAtLeast(0) / 1000)
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val seconds = total % 60
    return if (hours > 0) {
        String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, seconds)
    } else {
        String.format(Locale.ROOT, "%d:%02d", minutes, seconds)
    }
}

/** `1.25×`. */
internal fun formatSpeed(speed: Float): String {
    val text = String.format(Locale.ROOT, "%.2f", speed).trimEnd('0').trimEnd('.')
    return "$text×"
}

/** A round icon button with a borderless ripple. */
internal fun Context.iconButton(
    icon: Int,
    description: String,
    size: Int = 44,
    onClick: () -> Unit,
): ImageButton = ImageButton(this).apply {
    setImageResource(icon)
    contentDescription = description
    setColorFilter(Color.WHITE)
    scaleType = ImageView.ScaleType.CENTER_INSIDE
    val padding = dp(size / 4)
    setPadding(padding, padding, padding, padding)
    val ripple = TypedValue()
    context.theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, ripple, true)
    setBackgroundResource(ripple.resourceId)
    layoutParams = LinearLayout.LayoutParams(dp(size), dp(size))
    setOnClickListener { onClick() }
}

/** A rounded text button over the video. */
internal fun Context.pill(text: String, filled: Boolean = false, onClick: () -> Unit): TextView =
    TextView(this).apply {
        this.text = text
        setTextColor(if (filled) Color.BLACK else Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
        typeface = Typeface.DEFAULT_BOLD
        gravity = Gravity.CENTER
        setPadding(dp(14), dp(8), dp(14), dp(8))
        background = rounded(if (filled) Color.WHITE else 0x66000000, dp(20).toFloat())
        isClickable = true
        isFocusable = true
        setOnClickListener { onClick() }
        layoutParams = LinearLayout.LayoutParams(-2, -2).apply { marginStart = dp(8) }
    }

/** A vertical level (brightness or volume) with its icon and percentage; past 1 is boost. */
internal class LevelBar(context: Context, icon: Int) {
    private val trackHeight = context.dp(140)
    private val fill = View(context).apply { background = rounded(Color.WHITE, context.dp(3).toFloat()) }
    private val text = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
        gravity = Gravity.CENTER
    }
    val root = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        setPadding(context.dp(10), context.dp(12), context.dp(10), context.dp(12))
        background = rounded(0x99000000.toInt(), context.dp(24).toFloat())
        addView(text, LinearLayout.LayoutParams(context.dp(48), -2))
        val track = FrameLayout(context).apply {
            background = rounded(0x44FFFFFF, context.dp(3).toFloat())
            addView(fill, FrameLayout.LayoutParams(-1, 0, Gravity.BOTTOM))
        }
        addView(track, LinearLayout.LayoutParams(context.dp(6), trackHeight).apply {
            topMargin = context.dp(8)
            bottomMargin = context.dp(8)
        })
        addView(ImageView(context).apply {
            setImageResource(icon)
            setColorFilter(Color.WHITE)
        }, LinearLayout.LayoutParams(context.dp(22), context.dp(22)))
        visibility = View.GONE
    }

    /** `level` from 0 to 1; `boosted` colours it as past 100 %. */
    fun set(level: Float, label: String, boosted: Boolean) {
        val params = fill.layoutParams
        params.height = (trackHeight * level.coerceIn(0f, 1f)).toInt()
        fill.layoutParams = params
        (fill.background as? GradientDrawable)?.setColor(if (boosted) ACCENT else Color.WHITE)
        text.text = label
        root.visibility = View.VISIBLE
    }
}

/** A ScrollView no taller than `max()` pixels. */
private class CappedScroll(context: Context, private val max: () -> Int) : ScrollView(context) {
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(max(), MeasureSpec.AT_MOST))
    }
}

/**
 * A bottom sheet in the player's dark style: a title, then rows, chips and buttons. One shows at
 * a time; back or a tap outside closes it.
 */
internal class PlayerSheet(private val host: VideoPlayerActivity, title: String) {
    private val context: Context = host
    private val body = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private val panel = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        background = GradientDrawable().apply {
            setColor(PANEL)
            val radius = context.dp(20).toFloat()
            cornerRadii = floatArrayOf(radius, radius, radius, radius, 0f, 0f, 0f, 0f)
        }
        setPadding(0, context.dp(16), 0, context.dp(12))
        isClickable = true
        addView(TextView(context).apply {
            text = title
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            typeface = Typeface.DEFAULT_BOLD
            setPadding(context.dp(20), 0, context.dp(20), context.dp(8))
        })
        val scroll = CappedScroll(context) { (context.resources.displayMetrics.heightPixels * 0.72f).toInt() }
        scroll.addView(body)
        addView(scroll, LinearLayout.LayoutParams(-1, -2))
    }
    val root = FrameLayout(context).apply {
        setBackgroundColor(SCRIM)
        isClickable = true
        setOnClickListener { dismiss() }
        val width = minOf(context.resources.displayMetrics.widthPixels, context.dp(560))
        addView(panel, FrameLayout.LayoutParams(width, -2, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL))
        elevation = context.dp(16).toFloat()
    }
    var onDismiss: (() -> Unit)? = null

    fun header(text: String) {
        body.addView(TextView(context).apply {
            this.text = text
            setTextColor(0xB3FFFFFF.toInt())
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            typeface = Typeface.DEFAULT_BOLD
            setPadding(context.dp(20), context.dp(12), context.dp(20), context.dp(4))
        })
    }

    /** A row; `selected` ticks it, a null `onClick` greys it. */
    fun row(
        text: String,
        detail: String? = null,
        selected: Boolean = false,
        onClick: (() -> Unit)?,
    ): TextView {
        val view = TextView(context).apply {
            val check = if (selected) "✓  " else ""
            this.text = if (detail.isNullOrBlank()) "$check$text" else "$check$text\n$detail"
            setTextColor(
                when {
                    onClick == null -> 0x80FFFFFF.toInt()
                    selected -> ACCENT
                    else -> Color.WHITE
                },
            )
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(context.dp(20), context.dp(12), context.dp(20), context.dp(12))
            minHeight = context.dp(48)
            gravity = Gravity.CENTER_VERTICAL
            if (onClick != null) {
                val ripple = TypedValue()
                context.theme.resolveAttribute(android.R.attr.selectableItemBackground, ripple, true)
                setBackgroundResource(ripple.resourceId)
                isClickable = true
                isFocusable = true
                setOnClickListener { onClick() }
            }
        }
        body.addView(view, LinearLayout.LayoutParams(-1, -2))
        return view
    }

    /** A line of pills; `selected` marks one. */
    fun chips(labels: List<String>, selected: Int = -1, onPick: (Int) -> Unit) {
        val line = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(context.dp(12), context.dp(6), context.dp(20), context.dp(6))
        }
        labels.forEachIndexed { index, label ->
            line.addView(context.pill(label, filled = index == selected) { onPick(index) })
        }
        body.addView(HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(line)
        }, LinearLayout.LayoutParams(-1, -2))
    }

    /** A row with a name, its value, and pills that change it. */
    fun stepper(name: String, value: String, steps: List<Pair<String, () -> Unit>>) {
        val line = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(20), context.dp(6), context.dp(16), context.dp(6))
            addView(TextView(context).apply {
                text = "$name\n$value"
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            }, LinearLayout.LayoutParams(0, -2, 1f))
            steps.forEach { (label, action) -> addView(context.pill(label, onClick = action)) }
        }
        body.addView(line, LinearLayout.LayoutParams(-1, -2))
    }

    fun custom(view: View, height: Int = ViewGroup.LayoutParams.WRAP_CONTENT) {
        body.addView(view, LinearLayout.LayoutParams(-1, height))
    }

    fun show() = host.showSheet(this)

    fun dismiss() = host.closeSheet(this)

    internal fun dismissed() {
        onDismiss?.invoke()
    }
}

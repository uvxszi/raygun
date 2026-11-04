package app.raygun.audio

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.media3.common.PlaybackException
import androidx.media3.datasource.HttpDataSource
import java.util.Locale
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.roundToInt

/** One turn of the waiting arc: it grows from its head, then shrinks from its tail. */
private const val ARC_PERIOD_MS = 1_400L
/** The whole ring turns once in this long. */
private const val TURN_MS = 2_200L
private const val MIN_SWEEP = 18f
private const val MAX_SWEEP = 285f

/**
 * The player's waiting ring, drawn here in the accent: an arc that grows and shrinks as it turns
 * while nothing is known, and a filling ring with the percentage once [progress] is (0 to 1).
 * It redraws itself only while shown.
 */
internal class LoadingRing(context: Context) : View(context) {
    private val stroke = context.dp(4).toFloat()
    private val track = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = stroke
        color = 0x33FFFFFF
    }
    private val arc = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = stroke
        strokeCap = Paint.Cap.ROUND
        color = playerAccent
    }
    private val label = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.WHITE
        textAlign = Paint.Align.CENTER
        typeface = Typeface.DEFAULT_BOLD
    }
    private val bounds = RectF()
    private val started = System.nanoTime()
    private var shown = 0f
    private var lastFrame = 0L

    /** How far along it is; null while that isn't known. */
    var progress: Float? = null
        set(value) {
            val clamped = value?.coerceIn(0f, 1f)
            if (field == null && clamped != null) shown = 0f
            field = clamped
            invalidate()
        }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val inset = stroke / 2 + 1
        bounds.set(inset, inset, width - inset, height - inset)
        canvas.drawOval(bounds, track)
        val now = (System.nanoTime() - started) / 1_000_000
        val target = progress
        if (!DeviceState.animatorsEnabled()) {
            // Animations are off (Remove animations, a scale of 0): a still arc, and the fill
            // at its value at once.
            if (target == null) {
                canvas.drawArc(bounds, -90f, MAX_SWEEP * 0.75f, false, arc)
            } else {
                shown = target
                canvas.drawArc(bounds, -90f, 360f * shown.coerceAtLeast(0.02f), false, arc)
                drawPercent(canvas)
            }
        } else if (target == null) {
            val cycle = now / ARC_PERIOD_MS
            val phase = (now % ARC_PERIOD_MS).toFloat() / ARC_PERIOD_MS
            val head = if (phase < 0.5f) ease(phase * 2) else 1f
            val tail = if (phase < 0.5f) 0f else ease(phase * 2 - 1)
            val turn = (now % TURN_MS) * 360f / TURN_MS
            val start = turn + (cycle + tail) * (MAX_SWEEP - MIN_SWEEP)
            canvas.drawArc(bounds, start % 360f, MIN_SWEEP + (head - tail) * (MAX_SWEEP - MIN_SWEEP), false, arc)
            postInvalidateOnAnimation()
        } else {
            // Eases towards the target, so jumps in what's buffered fill smoothly.
            val step = if (lastFrame == 0L) 1f else min(1f, (now - lastFrame) / 160f)
            shown += (target - shown) * step
            canvas.drawArc(bounds, -90f, 360f * shown.coerceAtLeast(0.02f), false, arc)
            drawPercent(canvas)
            if (kotlin.math.abs(target - shown) > 0.002f) postInvalidateOnAnimation()
        }
        lastFrame = now
    }

    override fun onVisibilityChanged(changedView: View, visibility: Int) {
        super.onVisibilityChanged(changedView, visibility)
        // A ring shown again starts its frames afresh.
        lastFrame = 0L
        if (visibility == VISIBLE) invalidate()
    }

    private fun drawPercent(canvas: Canvas) {
        label.textSize = height * 0.26f
        val baseline = height / 2f - (label.descent() + label.ascent()) / 2
        canvas.drawText(String.format(Locale.ROOT, "%d%%", (shown * 100).roundToInt()), width / 2f, baseline, label)
    }

    private fun ease(value: Float): Float = ((1 - cos(PI * value)) / 2).toFloat()
}

/**
 * What the player shows while it waits (it replaces the platform's spinner): the ring where the
 * play button is, and below it what is happening ("Finding a source…") and a detail ("Trying
 * source 2 of 5 · Vidstream"). Over a dimmed picture while a video is found, over the picture
 * while one buffers. In picture-in-picture only a small ring shows.
 */
internal class PlayerLoading(context: Context) {
    private val ringSize = context.dp(56)
    val ring = LoadingRing(context)
    private val title = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
        gravity = Gravity.CENTER
        maxLines = 4
        ellipsize = TextUtils.TruncateAt.END
        setShadowLayer(6f, 0f, 1f, 0xCC000000.toInt())
    }
    private val detail = TextView(context).apply {
        setTextColor(0xCCFFFFFF.toInt())
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
        gravity = Gravity.CENTER
        maxLines = 2
        ellipsize = TextUtils.TruncateAt.END
        setShadowLayer(6f, 0f, 1f, 0xCC000000.toInt())
    }
    private val texts = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        addView(title, LinearLayout.LayoutParams(-2, -2))
        addView(detail, LinearLayout.LayoutParams(-2, -2).apply { topMargin = context.dp(4) })
    }

    /** The ring at the centre (where the play button is), the words just under it. */
    val root: FrameLayout = object : FrameLayout(context) {
        override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
            val centreX = (right - left) / 2
            val centreY = (bottom - top) / 2
            val half = ring.measuredWidth / 2
            ring.layout(centreX - half, centreY - half, centreX + half, centreY + half)
            val textTop = centreY + half + context.dp(16)
            val textHalf = texts.measuredWidth / 2
            texts.layout(centreX - textHalf, textTop, centreX + textHalf, textTop + texts.measuredHeight)
        }
    }.apply {
        addView(ring, FrameLayout.LayoutParams(ringSize, ringSize))
        addView(texts, FrameLayout.LayoutParams(-2, -2))
        visibility = View.GONE
    }

    /** Waiting for a video to be found, or for one to start: the picture is dimmed. */
    var finding = false
        private set

    val shown: Boolean get() = root.visibility == View.VISIBLE

    /** Shows the ring with `text` (none while buffering) and `note` under it. */
    fun show(text: String?, note: String?, dim: Boolean) {
        finding = dim
        setText(title, text)
        setText(detail, note)
        root.setBackgroundColor(if (dim) 0x99000000.toInt() else Color.TRANSPARENT)
        root.visibility = View.VISIBLE
    }

    fun setNote(note: String?) = setText(detail, note)

    fun hide() {
        finding = false
        ring.progress = null
        root.visibility = View.GONE
    }

    /** Picture-in-picture: a small ring, no words. */
    fun compact(on: Boolean) {
        val size = if (on) root.context.dp(28) else ringSize
        ring.layoutParams = FrameLayout.LayoutParams(size, size)
        texts.visibility = if (on) View.GONE else View.VISIBLE
    }

    /** Keeps the words to a readable width. */
    fun fitTo(width: Int) {
        val max = min(width - root.context.dp(48), root.context.dp(480))
        title.maxWidth = max
        detail.maxWidth = max
    }

    private fun setText(view: TextView, text: String?) {
        if (view.text?.toString() != (text ?: "")) view.text = text ?: ""
        view.visibility = if (text.isNullOrBlank()) View.GONE else View.VISIBLE
    }
}

/**
 * When a video can't play: what happened in plain words, what can be done (try again, another
 * source, another app), and the player's own words under More info.
 */
internal class PlayerProblem(private val host: VideoPlayerActivity) {
    private val context: Context = host
    private val message = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        gravity = Gravity.CENTER
    }
    private val detail = TextView(context).apply {
        setTextColor(0xB3FFFFFF.toInt())
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
        typeface = Typeface.MONOSPACE
        gravity = Gravity.CENTER
        setTextIsSelectable(true)
        visibility = View.GONE
    }
    private val moreInfo = context.pill(host.label("moreInfo", "More info")) {
        detail.visibility = if (detail.visibility == View.VISIBLE) View.GONE else View.VISIBLE
    }
    private val retry = context.pill(host.label("retry", "Try again"), filled = true) { onRetry?.invoke() }
    private val sources = context.pill(host.label("sources", "Sources")) { host.sheets.sources() }
    private val elsewhere = context.pill(host.label("openElsewhere", "Open in another app")) { host.openElsewhere() }
    private val buttons = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER
        addView(retry)
        addView(sources)
        addView(elsewhere)
    }
    private var onRetry: (() -> Unit)? = null

    val root = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        background = rounded(0xE6202124.toInt(), context.dp(20).toFloat())
        setPadding(context.dp(20), context.dp(18), context.dp(20), context.dp(14))
        isClickable = true
        addView(message, LinearLayout.LayoutParams(-2, -2))
        addView(buttons, LinearLayout.LayoutParams(-2, -2).apply { topMargin = context.dp(14) })
        addView(moreInfo, LinearLayout.LayoutParams(-2, -2).apply { topMargin = context.dp(8) })
        addView(detail, LinearLayout.LayoutParams(-2, -2).apply { topMargin = context.dp(8) })
        visibility = View.GONE
    }

    val shown: Boolean get() = root.visibility == View.VISIBLE

    /**
     * Shows `text`; `info` is the raw detail. `retry` tries again (no button without it), and
     * `sources` and `elsewhere` offer another source or another app.
     */
    fun show(text: String, info: String?, retry: (() -> Unit)?, sources: Boolean, elsewhere: Boolean) {
        message.text = text
        detail.text = info ?: ""
        detail.visibility = View.GONE
        moreInfo.visibility = if (info.isNullOrBlank()) View.GONE else View.VISIBLE
        onRetry = retry
        this.retry.visibility = if (retry == null) View.GONE else View.VISIBLE
        this.sources.visibility = if (sources) View.VISIBLE else View.GONE
        this.elsewhere.visibility = if (elsewhere) View.VISIBLE else View.GONE
        // The first button sits at the start of its row.
        val first = listOf(this.retry, this.sources, this.elsewhere).firstOrNull { it.visibility == View.VISIBLE }
        listOf(this.retry, this.sources, this.elsewhere).forEach { pill ->
            (pill.layoutParams as LinearLayout.LayoutParams).marginStart = if (pill === first) 0 else context.dp(8)
        }
        root.visibility = View.VISIBLE
        host.controls.setProblem(true)
    }

    fun hide() {
        if (root.visibility == View.GONE) return
        root.visibility = View.GONE
        host.controls.setProblem(false)
    }

    /** Keeps the panel to a readable width. */
    fun fitTo(width: Int) {
        val max = min(width - context.dp(48), context.dp(440))
        message.maxWidth = max
        detail.maxWidth = max
    }
}

/**
 * What went wrong with a video, for the viewer: the network, the server (the link may have
 * expired), the format, the phone's decoders or copy protection. The second part is Media3's own
 * words, for More info.
 */
internal fun describeProblem(error: PlaybackException, label: (String, String) -> String): Pair<String, String> {
    val http = generateSequence(error.cause) { it.cause }
        .filterIsInstance<HttpDataSource.InvalidResponseCodeException>()
        .firstOrNull()
    val message = when (error.errorCode) {
        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
        PlaybackException.ERROR_CODE_TIMEOUT,
        -> label("problemNetwork", "Couldn't reach the video's server. Check the connection and try again.")
        PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS -> when (http?.responseCode ?: 0) {
            404, 410 -> label("problemGone", "The video isn't on its server any more.")
            in 500..599 -> label("problemServer", "The video's server had a problem. Try again in a while.")
            else -> label("problemRefused", "The video's server refused to send it. The link may have expired.")
        }
        PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> label("problemGone", "The video isn't on its server any more.")
        PlaybackException.ERROR_CODE_IO_NO_PERMISSION -> label("problemNoAccess", "Raygun isn't allowed to open this file.")
        PlaybackException.ERROR_CODE_IO_CLEARTEXT_NOT_PERMITTED ->
            label("problemInsecure", "The video's address isn't secure (http), and Android blocks it.")
        in 3000..3999 -> label("problemFormat", "This video's format can't be read here. Another app may play it.")
        in 4000..5999 -> label("problemDecode", "This phone can't decode this video. Another app may play it.")
        in 6000..6999 -> label("problemProtected", "This video is copy-protected and can't play here.")
        else -> label("problemOther", "This video couldn't be played.")
    }
    val detail = buildString {
        append(error.errorCodeName)
        http?.let { append(" (HTTP ").append(it.responseCode).append(')') }
        error.message?.takeIf { it.isNotBlank() }?.let { append(": ").append(it) }
    }
    return message to detail
}

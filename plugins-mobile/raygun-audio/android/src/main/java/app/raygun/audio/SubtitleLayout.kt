package app.raygun.audio

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.os.Build
import android.text.Layout
import android.text.TextPaint
import android.text.style.LineBackgroundSpan
import androidx.media3.common.text.Cue
import androidx.media3.ui.CaptionStyleCompat

// Where subtitle lines go and how their backgrounds are drawn: the widest a line gets, rounded
// boxes behind the lines, and how far the lines slide to stay clear of the player's bars.

/** How a line lines up in its box, as `Layout.Alignment` does for left-to-right text. */
internal const val LINE_START = -1
internal const val LINE_MIDDLE = 0
internal const val LINE_END = 1

/** A cue's box across the picture: where it is (`Cue.position`), what holds it there, its width. */
internal data class CueBox(val position: Float, val anchor: Int, val size: Float)

/** How a cue's text lines up: centred unless the file says otherwise. */
internal fun lineAlignment(alignment: Layout.Alignment?): Int = when (alignment) {
    Layout.Alignment.ALIGN_NORMAL -> LINE_START
    Layout.Alignment.ALIGN_OPPOSITE -> LINE_END
    else -> LINE_MIDDLE
}

/**
 * The box a cue keeps to when its lines may be at most `max` of the picture's width (a fraction),
 * so longer lines wrap; null leaves the cue as it is. Media3 sizes a box from `size` (a share of
 * the width; unset, as wide as the text) and puts it at `position` by `anchor` (unset `position`:
 * centred). A narrower box keeps the lines where they were:
 * - a centred cue stays centred, at most `max` wide;
 * - a box the file sized (WebVTT, TTML) narrows around the side its text lines up on;
 * - a cue placed at a point as wide as its text (ASS, SubRip's `{\an}`) gets a box from that point
 *   as far as it may reach, when its text lines up with that point; any other is left alone.
 */
internal fun narrowedBox(position: Float, anchor: Int, size: Float, align: Int, max: Float): CueBox? {
    val unset = Cue.DIMEN_UNSET
    if (position == unset) {
        if (align != LINE_MIDDLE) return null
        val width = if (size == unset) max else minOf(size, max)
        return CueBox(unset, anchor, width)
    }
    val share = when (anchor) {
        Cue.ANCHOR_TYPE_MIDDLE -> 0.5f
        Cue.ANCHOR_TYPE_END -> 1f
        else -> 0f
    }
    if (size == unset) {
        val reach = when {
            share == 0f && align == LINE_START -> 1f - position
            share == 0.5f && align == LINE_MIDDLE -> 2f * minOf(position, 1f - position)
            share == 1f && align == LINE_END -> position
            else -> return null
        }
        // A cue at the very edge has no room to give: Media3 would draw nothing.
        if (reach < 0.1f) return null
        return CueBox(position, anchor, minOf(max, reach))
    }
    if (size <= max) return null
    val left = position - size * share
    val right = left + size
    val from = when (align) {
        LINE_START -> left
        LINE_END -> right - max
        else -> (left + right - max) / 2f
    }
    return CueBox(from.coerceIn(0f, 1f - max), Cue.ANCHOR_TYPE_START, max)
}

/** Where a line `width` wide sits between `left` and `right`, lined up as `align` says. */
internal fun lineExtent(left: Float, right: Float, width: Float, align: Int): Pair<Float, Float> = when (align) {
    LINE_START -> left to left + width
    LINE_END -> right - width to right
    else -> {
        val from = left + (right - left - width) / 2f
        from to from + width
    }
}

/** How far lines `chosen` px above the picture's foot rise to stay `gap` above a bar at `barTop`. */
internal fun liftAbove(pictureBottom: Int, chosen: Int, barTop: Int, gap: Int): Int =
    maxOf(0, pictureBottom - chosen - (barTop - gap))

/** How far lines starting at `lineTop` drop to stay `gap` below a bar ending at `barBottom`. */
internal fun dropBelow(lineTop: Int, barBottom: Int, gap: Int): Int = maxOf(0, barBottom + gap - lineTop)

/**
 * The subtitles' background behind each line, a box with rounded corners hugging the line's
 * words (Media3's own background is square). Media3's painter draws a cue's layout twice for some
 * edges (the outline, then the text over it; raised and depressed likewise), and a background
 * belongs under the edge, so it is drawn in the first of those only: for an outline while the
 * paint strokes, for raised and depressed while the shadow falls up and to the left (Android 10
 * on; before that the box is drawn in both).
 */
internal class RoundedLineBackground(
    private val color: Int,
    private val edge: Int,
    private val align: Int,
) : LineBackgroundSpan {
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val box = RectF()
    /** Measured line widths by their start, end and the text size they were measured at. */
    private val widths = HashMap<Triple<Int, Int, Float>, Float>()

    override fun drawBackground(
        canvas: Canvas,
        paint: Paint,
        left: Int,
        right: Int,
        top: Int,
        baseline: Int,
        bottom: Int,
        text: CharSequence,
        start: Int,
        end: Int,
        lineNumber: Int,
    ) {
        if (!drawsOn(paint)) return
        var last = end
        while (last > start && text[last - 1].isWhitespace()) last -= 1
        if (last <= start) return
        val size = paint.textSize
        val width = widths.getOrPut(Triple(start, last, size)) {
            val measure = paint as? TextPaint ?: TextPaint(paint)
            Layout.getDesiredWidth(text, start, last, measure)
        }
        // A right-to-left paragraph lines up "normal" on the right.
        val side = if (align != LINE_MIDDLE && rightToLeft(text, start, last)) -align else align
        val (from, to) = lineExtent(left.toFloat(), right.toFloat(), width, side)
        val padding = size * 0.3f
        box.set(from - padding, top.toFloat(), to + padding, bottom.toFloat())
        val radius = minOf(size * 0.24f, box.height() / 2f)
        fill.color = color
        canvas.drawRoundRect(box, radius, radius, fill)
    }

    /** Whether a line's first letter with a direction reads right to left. */
    private fun rightToLeft(text: CharSequence, start: Int, end: Int): Boolean {
        for (index in start until end) {
            when (Character.getDirectionality(text[index])) {
                Character.DIRECTIONALITY_RIGHT_TO_LEFT,
                Character.DIRECTIONALITY_RIGHT_TO_LEFT_ARABIC,
                -> return true
                Character.DIRECTIONALITY_LEFT_TO_RIGHT -> return false
            }
        }
        return false
    }

    private fun drawsOn(paint: Paint): Boolean = when (edge) {
        CaptionStyleCompat.EDGE_TYPE_OUTLINE -> paint.style == Paint.Style.FILL_AND_STROKE
        CaptionStyleCompat.EDGE_TYPE_RAISED, CaptionStyleCompat.EDGE_TYPE_DEPRESSED ->
            Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || paint.shadowLayerDx <= 0f
        else -> true
    }
}

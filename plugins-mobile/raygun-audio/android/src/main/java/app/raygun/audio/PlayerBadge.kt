package app.raygun.audio

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.text.style.ReplacementSpan

/**
 * A small tag drawn inline after a row's name ("Preferred"): smaller bold words on a faint
 * rounded pill, centred on the line, in the row's own colour.
 */
internal class BadgeSpan(private val density: Float) : ReplacementSpan() {
    private val padX get() = 6f * density
    private val gap get() = 6f * density
    private val scale = 0.72f

    private fun small(paint: Paint): Paint = Paint(paint).apply {
        textSize = paint.textSize * scale
        typeface = Typeface.create(paint.typeface, Typeface.BOLD)
    }

    override fun getSize(paint: Paint, text: CharSequence, start: Int, end: Int, fm: Paint.FontMetricsInt?): Int {
        val words = small(paint)
        return (gap + padX * 2 + words.measureText(text, start, end)).toInt() + 1
    }

    override fun draw(
        canvas: Canvas,
        text: CharSequence,
        start: Int,
        end: Int,
        x: Float,
        top: Int,
        y: Int,
        bottom: Int,
        paint: Paint,
    ) {
        val words = small(paint)
        val width = words.measureText(text, start, end)
        val metrics = words.fontMetrics
        val textHeight = metrics.descent - metrics.ascent
        val pillHeight = textHeight + 2f * density
        // Centred on the line's own text (its baseline and the full-size letters' middle).
        val middle = y + (paint.fontMetrics.ascent + paint.fontMetrics.descent) / 2f
        val left = x + gap
        val pill = RectF(left, middle - pillHeight / 2f, left + width + padX * 2, middle + pillHeight / 2f)
        val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = (paint.color and 0x00FFFFFF) or 0x2E000000
            style = Paint.Style.FILL
        }
        canvas.drawRoundRect(pill, pillHeight / 2f, pillHeight / 2f, fill)
        val baseline = middle - (metrics.ascent + metrics.descent) / 2f
        canvas.drawText(text, start, end, left + padX, baseline, words)
    }
}

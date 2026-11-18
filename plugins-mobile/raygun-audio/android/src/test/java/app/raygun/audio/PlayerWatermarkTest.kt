package app.raygun.audio

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

/** The same cases and numbers as the desktop's `watermarkMath.test.ts`. */
class PlayerWatermarkTest {
    private class Random(seed: Long) {
        private var state = seed

        fun next(): Double {
            state = (state * 1_103_515_245L + 12_345L) % 2_147_483_648L
            return state / 2_147_483_648.0
        }
    }

    private fun byte(value: Double): Byte = WatermarkMath.roundHalfUp(value * 255).toByte()

    @Test
    fun laysTheBoxAndARingOnOneGridInsideThePicture() {
        val middle = WatermarkMath.sampleLayout(WatermarkRemoval(x = 4_000, y = 4_000, width = 1_000, height = 500), 1920, 1080)
        assertEquals(48, middle.innerCols)
        assertEquals(14, middle.innerRows)
        assertEquals(24, middle.innerLeft)
        assertEquals(7, middle.innerTop)
        assertEquals(96, middle.cols)
        assertEquals(28, middle.rows)
        assertEquals(768.0 - 96, middle.rect.left, 1e-5)

        val corner = WatermarkMath.sampleLayout(WatermarkRemoval(width = 1_000, height = 1_000), 1920, 1080)
        assertEquals(0, corner.innerLeft)
        assertEquals(0, corner.innerTop)
        assertEquals(corner.innerCols + (corner.innerCols + 1) / 2, corner.cols)
        assertEquals(0.0, corner.rect.left, 0.0)
        assertEquals(0.0, corner.rect.top, 0.0)
    }

    @Test
    fun findsTheOverlaysOpacityAndColourFromFramesThatChangeUnderIt() {
        val layout = WatermarkMath.sampleLayout(WatermarkRemoval(x = 4_000, y = 4_000, width = 1_000, height = 1_000), 640, 360)
        val estimator = WatermarkEstimator(layout)
        val random = Random(7)
        fun inLogo(col: Int, row: Int) =
            col >= layout.innerLeft + layout.innerCols / 4.0 && col < layout.innerLeft + layout.innerCols * 3 / 4.0 &&
                row >= layout.innerTop + layout.innerRows / 4.0 && row < layout.innerTop + layout.innerRows * 3 / 4.0
        repeat(200) { frame ->
            val pixels = ByteArray(layout.cols * layout.rows * 4)
            for (cell in 0 until layout.cols * layout.rows) {
                val video = random.next()
                val shown = if (inLogo(cell % layout.cols, cell / layout.cols)) 0.5 + 0.5 * video else video
                val value = byte(shown)
                pixels[cell * 4] = value
                pixels[cell * 4 + 1] = value
                pixels[cell * 4 + 2] = value
                pixels[cell * 4 + 3] = -1
            }
            estimator.add(pixels)
            if (frame == WatermarkMath.MIN_SAMPLES - 2) assertNull(estimator.estimate())
        }
        val mask = estimator.estimate()
        assertNotNull(mask)
        fun at(col: Int, row: Int) = (row * layout.innerCols + col) * 4
        val centre = at(layout.innerCols / 2, layout.innerRows / 2)
        val alpha = (mask!!.data[centre + 3].toInt() and 0xFF) / 255.0
        assertTrue("opacity $alpha", alpha > 0.4 && alpha < 0.6)
        // The colour is stored times the opacity.
        assertTrue((mask.data[centre].toInt() and 0xFF) / 255.0 / alpha > 0.85)
        assertEquals(0, mask.data[at(0, 0) + 3].toInt())
    }

    @Test
    fun waitsForAPictureThatChanges() {
        val layout = WatermarkMath.sampleLayout(WatermarkRemoval(x = 4_000, y = 4_000, width = 1_000, height = 1_000), 640, 360)
        val estimator = WatermarkEstimator(layout)
        val still = ByteArray(layout.cols * layout.rows * 4) { 128.toByte() }
        repeat(60) { estimator.add(still) }
        assertNull(estimator.estimate())
    }

    @Test
    fun findsAStillLogoInACornerOfAMovingPicture() {
        val finder = OverlayFinder()
        val random = Random(11)
        fun logo(col: Int, row: Int) = col in 52..58 && row in 3..6 && (col + row) % 2 == 0
        repeat(48) {
            val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
            for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                val col = cell % OverlayFinder.COLS
                val row = cell / OverlayFinder.COLS
                val value = if (logo(col, row)) 240.toByte() else WatermarkMath.roundHalfUp(random.next() * 200).toByte()
                pixels[cell * 4] = value
                pixels[cell * 4 + 1] = value
                pixels[cell * 4 + 2] = value
            }
            finder.add(pixels)
        }
        val box = finder.find()
        assertNotNull(box)
        assertEquals(WatermarkMath.roundHalfUp(51.0 / 64 * 10_000), box!!.x)
        assertEquals(WatermarkMath.roundHalfUp(2.0 / 36 * 10_000), box.y)
        assertEquals(WatermarkMath.roundHalfUp(60.0 / 64 * 10_000), box.x + box.width)
        assertEquals(WatermarkMath.roundHalfUp(8.0 / 36 * 10_000), box.y + box.height)
    }

    @Test
    fun takesALetterboxForNoLogo() {
        val finder = OverlayFinder()
        val random = Random(5)
        repeat(48) {
            val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
            for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                val value = if (cell < OverlayFinder.COLS * 4) 0 else WatermarkMath.roundHalfUp(random.next() * 200)
                pixels[cell * 4] = value.toByte()
                pixels[cell * 4 + 1] = value.toByte()
                pixels[cell * 4 + 2] = value.toByte()
            }
            finder.add(pixels)
        }
        assertNull(finder.find())
    }

    @Test
    fun findsNothingInAPictureThatHardlyChanges() {
        val finder = OverlayFinder()
        val flat = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4) { 90 }
        repeat(48) { finder.add(flat) }
        assertNull(finder.find())
    }

    @Test
    fun mapsTheBoxAndControlsOntoTheShadersNumbers() {
        assertArrayEquals(
            floatArrayOf(0.1f, 0.7f, 0.4f, 0.8f),
            WatermarkMath.shaderBox(WatermarkRemoval(x = 1_000, y = 2_000, width = 3_000, height = 1_000)),
            1e-6f,
        )
        assertEquals(2.0, WatermarkMath.contrast(100), 0.0)
        assertEquals(0.5, WatermarkMath.contrast(-100), 0.0)
        assertEquals(0.25, WatermarkMath.feather(50), 0.0)
    }

    @Test
    fun aBoxThatsOffChangesNothing() {
        val box = WatermarkRemoval(x = 100, y = 100, width = 1_000, height = 500)
        assertTrue(PictureEffects(watermark = box).neutral)
        assertFalse(PictureEffects(watermark = box.copy(enabled = true)).neutral)
        // Presets compare the adjustments alone.
        assertEquals(PictureEffects.NONE, PictureEffects(watermark = box.copy(enabled = true)).adjustments)
        assertEquals(WatermarkRemoval(x = 9_000, width = 1_000), WatermarkRemoval(x = 9_000, width = 5_000).clamped())
    }

    @Test
    fun pitchMovesBySemitonesApartFromTheTempo() {
        assertEquals(1f, pitchRatio(0), 0f)
        assertEquals(2f, pitchRatio(120), 1e-6f)
        assertEquals(0.5f, pitchRatio(-120), 1e-6f)
        assertEquals(2f, pitchRatio(500), 1e-6f)
        assertTrue(abs(pitchRatio(10) - 1.0594631f) < 1e-6f)
        assertEquals("+1.5", VideoSoundMath.pitchText(15))
        assertEquals("−3", VideoSoundMath.pitchText(-30))
        assertEquals("0", VideoSoundMath.pitchText(0))
        assertTrue(VideoSound().neutral)
        assertFalse(VideoSound(pitch = 5).neutral)
    }
    private fun pixel(values: DoubleArray, pixels: ByteArray, cell: Int) {
        for (channel in 0 until 3) pixels[cell * 4 + channel] = WatermarkMath.roundHalfUp(values[channel] * 255).toByte()
        pixels[cell * 4 + 3] = -1
    }

    /** Three held shots of different scenery with strong edges (a little noise), a logo or not. */
    private fun shots(withLogo: Boolean, count: Int = 3): OverlayFinder {
        val finder = OverlayFinder()
        val random = Random(17)
        fun logo(col: Int, row: Int) = col in 50..58 && row in 3..6 && (col + row) % 2 == 0
        val blocks = DoubleArray(22 * 12) { random.next() }
        repeat(count) { shot ->
            // Scenery: blocks of 3 × 3 cells at their own levels, held for the whole shot; each
            // shot's level of a block is another third of the way round, so the shots differ.
            val levels = DoubleArray(blocks.size) { 0.1 + 0.8 * ((blocks[it] + shot / 3.0) % 1.0) }
            repeat(30) {
                val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
                for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                    val col = cell % OverlayFinder.COLS
                    val row = cell / OverlayFinder.COLS
                    val scenery = levels[(row / 3) * 22 + col / 3] + (random.next() - 0.5) * 0.02
                    val value = if (withLogo && logo(col, row)) 0.95 else scenery
                    pixel(DoubleArray(3) { value.coerceIn(0.0, 1.0) }, pixels, cell)
                }
                finder.add(pixels)
            }
        }
        return finder
    }

    @Test
    fun takesAUniformPatchThatLooksAlikeInEveryShotForNoOverlay() {
        val finder = OverlayFinder()
        val random = Random(23)
        fun grass(col: Int, row: Int) = col in 16..23 && row >= 28
        repeat(90) {
            val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
            for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                val value = if (grass(cell % OverlayFinder.COLS, cell / OverlayFinder.COLS)) 0.35 + (random.next() - 0.5) * 0.01 else random.next()
                pixel(DoubleArray(3) { value }, pixels, cell)
            }
            finder.add(pixels)
        }
        assertTrue(finder.cuts >= 2)
        val verdict = finder.verdict()
        assertNull(verdict.found)
        assertFalse(verdict.unsure)
    }

    @Test
    fun takesSceneryHeldStillWithinShotsForNoOverlay() {
        val finder = shots(withLogo = false)
        assertEquals(2, finder.cuts)
        assertTrue(finder.ready)
        val verdict = finder.verdict()
        assertNull(verdict.found)
        assertFalse(verdict.unsure)
    }

    @Test
    fun findsALogoThatStaysTheSameAcrossSceneChanges() {
        val box = shots(withLogo = true).find()
        assertNotNull(box)
        assertTrue(box!!.x / 10_000.0 <= 50.0 / 64)
        assertTrue((box.x + box.width) / 10_000.0 >= 59.0 / 64)
    }

    @Test
    fun waitsForSceneChangesInOneHeldShotAndCantTellWithoutThem() {
        val finder = shots(withLogo = true, count = 1)
        assertFalse(finder.ready)
        assertTrue(finder.verdict().unsure)
    }

    @Test
    fun findsNothingInMovingVideoWithoutAnOverlayAndCantTellInAStillOne() {
        val finder = OverlayFinder()
        val random = Random(3)
        repeat(48) {
            val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
            for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                pixel(doubleArrayOf(random.next(), random.next(), random.next()), pixels, cell)
            }
            finder.add(pixels)
        }
        val moving = finder.verdict()
        assertNull(moving.found)
        assertFalse(moving.unsure)

        val still = OverlayFinder()
        val flat = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4) { 90 }
        repeat(48) { still.add(flat) }
        assertTrue(still.verdict().unsure)
    }

    @Test
    fun findsAStillTextBlockAndTakesHalfClearWhiteTextOutWithoutTinting() {
        val opacity = 0.4
        fun inBlock(col: Int, row: Int) = col in 4..14 && row in 29..32
        val finder = OverlayFinder()
        val random = Random(13)
        repeat(48) {
            val pixels = ByteArray(OverlayFinder.COLS * OverlayFinder.ROWS * 4)
            for (cell in 0 until OverlayFinder.COLS * OverlayFinder.ROWS) {
                val col = cell % OverlayFinder.COLS
                val block = inBlock(col, cell / OverlayFinder.COLS)
                // A caption: a dark box that stays, its letters white at 80 % (every other column).
                val video = if (block) doubleArrayOf(0.1, 0.08, 0.06) else doubleArrayOf(random.next(), random.next(), random.next())
                val letter = if (block && col % 2 == 0) 0.8 else 0.0
                pixel(DoubleArray(3) { letter + (1 - letter) * video[it] }, pixels, cell)
            }
            finder.add(pixels)
        }
        val box = finder.verdict().found
        assertNotNull(box)
        assertTrue(box!!.x / 10_000.0 <= 4.0 / 64)
        assertTrue((box.x + box.width) / 10_000.0 >= 15.0 / 64)
        assertTrue(box.y / 10_000.0 <= 29.0 / 36)

        val layout = WatermarkMath.sampleLayout(WatermarkRemoval(x = 4_000, y = 4_000, width = 1_000, height = 1_000), 640, 360)
        val estimator = WatermarkEstimator(layout)
        fun covered(col: Int, row: Int) = col >= layout.innerLeft && col < layout.innerLeft + layout.innerCols &&
            row >= layout.innerTop && row < layout.innerTop + layout.innerRows
        repeat(300) {
            val pixels = ByteArray(layout.cols * layout.rows * 4)
            for (cell in 0 until layout.cols * layout.rows) {
                val video = doubleArrayOf(random.next(), random.next(), random.next())
                val over = covered(cell % layout.cols, cell / layout.cols)
                pixel(DoubleArray(3) { if (over) opacity + (1 - opacity) * video[it] else video[it] }, pixels, cell)
            }
            estimator.add(pixels)
        }
        val mask = estimator.estimate()
        assertNotNull(mask)
        val cell = ((layout.innerRows / 2) * layout.innerCols + layout.innerCols / 2) * 4
        val texel = IntArray(4) { mask!!.data[cell + it].toInt() and 0xFF }
        assertTrue(texel[3] / 255.0 <= WatermarkMath.ALPHA_MAX + 0.01)
        for (video in listOf(doubleArrayOf(0.6, 0.3, 0.2), doubleArrayOf(0.1, 0.5, 0.8), doubleArrayOf(0.9, 0.9, 0.2))) {
            val shown = DoubleArray(3) { opacity + (1 - opacity) * video[it] }
            val restored = WatermarkMath.restorePixel(shown, texel)
            for (channel in 0 until 3) assertTrue("channel $channel", abs(restored[channel] - video[channel]) < 0.1)
            if (video.toSet().size == 3) {
                assertEquals((0..2).sortedByDescending { video[it] }, (0..2).sortedByDescending { restored[it] })
            }
        }
    }

    @Test
    fun onlyTheMarkItselfIsMaskedNotStillPictureWithoutEdges() {
        // As `watermarkMath.test.ts`: a box half over a still, flat part of the picture (a dark
        // band) and half over moving picture with a logo's strokes in it.
        val layout = WatermarkMath.sampleLayout(WatermarkRemoval(x = 4_000, y = 4_000, width = 1_000, height = 1_000), 640, 360)
        val estimator = WatermarkEstimator(layout)
        val next = Random(5)
        val stroke = { col: Int, row: Int ->
            col >= layout.innerLeft + layout.innerCols / 2 + 2 && col < layout.innerLeft + layout.innerCols - 2 &&
                row >= layout.innerTop + 2 && row < layout.innerTop + layout.innerRows - 2 && col % 3 == 0
        }
        val band = { col: Int -> col < layout.innerLeft + layout.innerCols / 2 }
        repeat(200) {
            val pixels = ByteArray(layout.cols * layout.rows * 4)
            for (cell in 0 until layout.cols * layout.rows) {
                val col = cell % layout.cols
                val row = cell / layout.cols
                val video = next.next()
                val inside = col >= layout.innerLeft && row >= layout.innerTop &&
                    col < layout.innerLeft + layout.innerCols && row < layout.innerTop + layout.innerRows
                val shown = when {
                    inside && band(col) -> 0.1
                    stroke(col, row) -> 0.6 + 0.4 * video
                    else -> video
                }
                val byte = (shown * 255).toInt().toByte()
                pixels[cell * 4] = byte
                pixels[cell * 4 + 1] = byte
                pixels[cell * 4 + 2] = byte
                pixels[cell * 4 + 3] = 0xFF.toByte()
            }
            estimator.add(pixels)
        }
        val mask = estimator.estimate()
        assertNotNull(mask)
        fun alphaAt(col: Int, row: Int) = mask!!.data[(row * layout.innerCols + col) * 4 + 3].toInt() and 0xFF
        // The still band has no edge of its own inside it: untouched.
        assertEquals(0, alphaAt(2, layout.innerRows / 2))
        // A stroke is masked.
        val strokeCol = (layout.innerCols / 2 + 2 until layout.innerCols - 2).first { (it + layout.innerLeft) % 3 == 0 }
        assertTrue(alphaAt(strokeCol, layout.innerRows / 2) > 0)
    }
}

package app.raygun.audio

import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.Timeline
import androidx.media3.common.TrackGroup
import androidx.media3.common.util.Clock
import androidx.media3.common.util.HandlerWrapper
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.trackselection.ExoTrackSelection
import androidx.media3.exoplayer.upstream.BandwidthMeter
import org.junit.Assert.assertEquals
import org.junit.Test

@OptIn(UnstableApi::class)
class QualityHoldTest {
    private var bitrate = 500_000L

    private val meter = object : BandwidthMeter {
        override fun getBitrateEstimate(): Long = bitrate
        override fun getTransferListener(): TransferListener? = null
        override fun addEventListener(eventHandler: Handler, eventListener: BandwidthMeter.EventListener) = Unit
        override fun removeEventListener(eventListener: BandwidthMeter.EventListener) = Unit
    }

    private val clock = object : Clock {
        override fun currentTimeMillis(): Long = 0
        override fun elapsedRealtime(): Long = 0
        override fun uptimeMillis(): Long = 0
        override fun nanoTime(): Long = 0
        override fun createHandler(looper: Looper, callback: Handler.Callback?): HandlerWrapper =
            throw UnsupportedOperationException()
        override fun onThreadBlocked() = Unit
    }

    private fun variant(id: String, height: Int, bitrate: Int): Format = Format.Builder()
        .setId(id)
        .setSampleMimeType(MimeTypes.VIDEO_H264)
        .setHeight(height)
        .setWidth(height * 16 / 9)
        .setAverageBitrate(bitrate)
        .setPeakBitrate(bitrate)
        .build()

    private fun selection(hold: QualityHold): ExoTrackSelection {
        val group = TrackGroup(
            "video",
            variant("1080", 1080, 5_000_000),
            variant("480", 480, 1_000_000),
            variant("240", 240, 300_000),
        )
        return hold.Factory(clock).createTrackSelections(
            arrayOf(ExoTrackSelection.Definition(group, 0, 1, 2)),
            meter,
            MediaSource.MediaPeriodId(Any()),
            Timeline.EMPTY,
        )[0]!!
    }

    private fun update(selection: ExoTrackSelection) =
        selection.updateSelectedTrack(0, 20_000_000, 600_000_000, emptyList(), emptyArray())

    private fun heightOf(selection: ExoTrackSelection): Int = selection.selectedFormat.height

    @Test
    fun aHeldSelectionKeepsItsVariantWhenTheConnectionImproves() {
        val hold = QualityHold()
        val chosen = selection(hold)
        update(chosen)
        assertEquals(240, heightOf(chosen))
        hold.held = true
        bitrate = 50_000_000
        update(chosen)
        assertEquals(240, heightOf(chosen))
        hold.held = false
        update(chosen)
        assertEquals(1080, heightOf(chosen))
    }

    @Test
    fun aSelectionMadeWhileHeldStillMakesItsFirstPick() {
        val hold = QualityHold()
        hold.held = true
        bitrate = 50_000_000
        val chosen = selection(hold)
        update(chosen)
        assertEquals(1080, heightOf(chosen))
    }
}

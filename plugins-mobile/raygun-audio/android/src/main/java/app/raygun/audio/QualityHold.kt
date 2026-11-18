package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.TrackGroup
import androidx.media3.common.util.Clock
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.source.chunk.MediaChunk
import androidx.media3.exoplayer.source.chunk.MediaChunkIterator
import androidx.media3.exoplayer.trackselection.AdaptiveTrackSelection
import androidx.media3.exoplayer.trackselection.DefaultTrackSelector
import androidx.media3.exoplayer.upstream.BandwidthMeter
import com.google.common.collect.ImmutableList

/**
 * Holding the variant playing while the quality is automatic (Quality's Playing row): the
 * adaptive selection stops switching and keeps what it has buffered. A track override can't do
 * this: any new selection makes an HLS stream drop what it buffered and load again.
 *
 * `held` is set on the main thread and read on the playback thread.
 */
@OptIn(UnstableApi::class)
internal class QualityHold {
    @Volatile
    var held = false

    /** The player's track selector, its adaptive selections asking [held] first. */
    fun trackSelector(context: android.content.Context): DefaultTrackSelector =
        DefaultTrackSelector(context, Factory())

    internal inner class Factory(private val clock: Clock = Clock.DEFAULT) : AdaptiveTrackSelection.Factory() {
        override fun createAdaptiveTrackSelection(
            group: TrackGroup,
            tracks: IntArray,
            type: Int,
            bandwidthMeter: BandwidthMeter,
            adaptationCheckpoints: ImmutableList<AdaptiveTrackSelection.AdaptationCheckpoint>,
        ): AdaptiveTrackSelection = Holding(group, tracks, type, bandwidthMeter, adaptationCheckpoints, clock)
    }

    private inner class Holding(
        group: TrackGroup,
        tracks: IntArray,
        type: Int,
        bandwidthMeter: BandwidthMeter,
        adaptationCheckpoints: List<AdaptiveTrackSelection.AdaptationCheckpoint>,
        clock: Clock,
    ) : AdaptiveTrackSelection(
        group,
        tracks,
        type,
        bandwidthMeter,
        AdaptiveTrackSelection.DEFAULT_MIN_DURATION_FOR_QUALITY_INCREASE_MS.toLong(),
        AdaptiveTrackSelection.DEFAULT_MAX_DURATION_FOR_QUALITY_DECREASE_MS.toLong(),
        AdaptiveTrackSelection.DEFAULT_MIN_DURATION_TO_RETAIN_AFTER_DISCARD_MS.toLong(),
        AdaptiveTrackSelection.DEFAULT_MAX_WIDTH_TO_DISCARD,
        AdaptiveTrackSelection.DEFAULT_MAX_HEIGHT_TO_DISCARD,
        AdaptiveTrackSelection.DEFAULT_BANDWIDTH_FRACTION,
        AdaptiveTrackSelection.DEFAULT_BUFFERED_FRACTION_TO_LIVE_EDGE_FOR_QUALITY_INCREASE,
        adaptationCheckpoints,
        clock,
    ) {
        private var started = false

        override fun updateSelectedTrack(
            playbackPositionUs: Long,
            bufferedDurationUs: Long,
            availableDurationUs: Long,
            queue: List<MediaChunk>,
            mediaChunkIterators: Array<MediaChunkIterator>,
        ) {
            // The first pick is always made, so a selection made while held still has one.
            if (held && started) return
            started = true
            super.updateSelectedTrack(playbackPositionUs, bufferedDurationUs, availableDurationUs, queue, mediaChunkIterators)
        }

        override fun evaluateQueueSize(playbackPositionUs: Long, queue: List<MediaChunk>): Int =
            if (held) queue.size else super.evaluateQueueSize(playbackPositionUs, queue)
    }
}

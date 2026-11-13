/*
 * Adapted from Media3's TimeOffsetMediaPeriod (androidx/media, Apache License 2.0):
 *
 * Copyright (C) 2016 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Changed for Raygun: only the sound's samples are moved, by a delay that can change while the
 * video plays (it applies from the next seek), and a later sound is read from that much before
 * the position asked so it has its start.
 */
package app.raygun.audio

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.StreamKey
import androidx.media3.common.util.UnstableApi
import androidx.media3.decoder.DecoderInputBuffer
import androidx.media3.exoplayer.FormatHolder
import androidx.media3.exoplayer.LoadingInfo
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.exoplayer.source.MediaPeriod
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.SampleStream
import androidx.media3.exoplayer.source.TrackGroupArray
import androidx.media3.exoplayer.source.WrappingMediaSource
import androidx.media3.exoplayer.trackselection.ExoTrackSelection
import androidx.media3.exoplayer.upstream.Allocator

/** The largest audio delay either way (ten minutes), as the subtitle delay's. */
internal const val MAX_AUDIO_DELAY_MS = 600_000L

/**
 * How much later the sound plays than the picture, in microseconds; negative plays it earlier.
 * Set from the player's thread; periods take it at their next seek ([AudioDelayPeriod]).
 */
internal class AudioDelay {
    @Volatile
    var us: Long = 0L
}

/**
 * Where a period reads from for `positionUs` when its sound is `delayUs` later: that much before
 * it (never before the start), so the sound's start is there; an earlier sound reads from the
 * position itself and its first samples go before the position, unplayed.
 */
internal fun leadPosition(positionUs: Long, delayUs: Long): Long =
    (positionUs - delayUs.coerceAtLeast(0)).coerceAtLeast(0)

/**
 * The position a seek or selection landed on, as the player sees it: the one asked when the
 * period went where [leadPosition] sent it, else where it went, moved by the lead.
 */
internal fun landedPosition(askedUs: Long, sentUs: Long, gotUs: Long, delayUs: Long): Long =
    if (gotUs == sentUs) askedUs else gotUs + delayUs.coerceAtLeast(0)

/**
 * A media source whose sound plays [AudioDelay] later than its picture (the video's own sound,
 * or another source's merged in): its periods are [AudioDelayPeriod]s.
 */
@OptIn(UnstableApi::class)
internal class AudioDelaySource(source: MediaSource, private val delay: AudioDelay) : WrappingMediaSource(source) {
    override fun createPeriod(id: MediaSource.MediaPeriodId, allocator: Allocator, startPositionUs: Long): MediaPeriod =
        AudioDelayPeriod(super.createPeriod(id, allocator, startPositionUs), delay)

    override fun releasePeriod(mediaPeriod: MediaPeriod) {
        super.releasePeriod((mediaPeriod as AudioDelayPeriod).inner)
    }
}

/**
 * A period whose sound samples are moved by the delay: the picture's keep their times, and a
 * later sound is read from that much before the position (the picture's samples before it are
 * decoded, not shown, as after any seek). The delay in use ([appliedUs]) changes only at a seek
 * or a new preparation, both on the playback thread, so samples already read never jump.
 */
@OptIn(UnstableApi::class)
internal class AudioDelayPeriod(
    val inner: MediaPeriod,
    private val delay: AudioDelay,
) : MediaPeriod, MediaPeriod.Callback {
    private var callback: MediaPeriod.Callback? = null

    /** The delay the sound's samples are moved by now. */
    @Volatile
    var appliedUs: Long = delay.us
        private set

    private val leadUs: Long get() = appliedUs.coerceAtLeast(0)

    override fun prepare(callback: MediaPeriod.Callback, positionUs: Long) {
        this.callback = callback
        appliedUs = delay.us
        inner.prepare(this, leadPosition(positionUs, appliedUs))
    }

    override fun maybeThrowPrepareError() = inner.maybeThrowPrepareError()

    override fun getTrackGroups(): TrackGroupArray = inner.trackGroups

    override fun getStreamKeys(trackSelections: List<ExoTrackSelection>): List<StreamKey> =
        inner.getStreamKeys(trackSelections)

    override fun selectTracks(
        selections: Array<out ExoTrackSelection?>,
        mayRetainStreamFlags: BooleanArray,
        streams: Array<SampleStream?>,
        streamResetFlags: BooleanArray,
        positionUs: Long,
    ): Long {
        val childStreams = arrayOfNulls<SampleStream>(streams.size)
        for (index in streams.indices) {
            val stream = streams[index]
            childStreams[index] = if (stream is DelayedSampleStream) stream.inner else stream
        }
        val sent = leadPosition(positionUs, appliedUs)
        val got = inner.selectTracks(selections, mayRetainStreamFlags, childStreams, streamResetFlags, sent)
        for (index in streams.indices) {
            val child = childStreams[index]
            val sound = selections.getOrNull(index)?.trackGroup?.type == C.TRACK_TYPE_AUDIO
            val before = streams[index]
            streams[index] = when {
                child == null -> null
                !sound -> child
                before is DelayedSampleStream && before.inner === child -> before
                else -> DelayedSampleStream(child, this)
            }
        }
        return landedPosition(positionUs, sent, got, appliedUs)
    }

    override fun discardBuffer(positionUs: Long, toKeyframe: Boolean) =
        inner.discardBuffer(leadPosition(positionUs, appliedUs), toKeyframe)

    override fun setUsesStreamPrerollFlags() = inner.setUsesStreamPrerollFlags()

    override fun readDiscontinuity(): Long {
        val position = inner.readDiscontinuity()
        return if (position == C.TIME_UNSET) C.TIME_UNSET else position + leadUs
    }

    override fun seekToUs(positionUs: Long): Long {
        // A new delay applies from here: everything is read again.
        appliedUs = delay.us
        val sent = leadPosition(positionUs, appliedUs)
        return landedPosition(positionUs, sent, inner.seekToUs(sent), appliedUs)
    }

    override fun getAdjustedSeekPositionUs(positionUs: Long, seekParameters: SeekParameters): Long =
        inner.getAdjustedSeekPositionUs(positionUs, seekParameters)

    override fun getBufferedPositionUs(): Long = inner.bufferedPositionUs

    override fun getNextLoadPositionUs(): Long = inner.nextLoadPositionUs

    override fun continueLoading(loadingInfo: LoadingInfo): Boolean = inner.continueLoading(loadingInfo)

    override fun isLoading(): Boolean = inner.isLoading

    override fun reevaluateBuffer(positionUs: Long) = inner.reevaluateBuffer(leadPosition(positionUs, appliedUs))

    override fun setEndPositionUs(endPositionUs: Long): Long = inner.setEndPositionUs(endPositionUs)

    override fun onPrepared(mediaPeriod: MediaPeriod) {
        callback?.onPrepared(this)
    }

    override fun onContinueLoadingRequested(source: MediaPeriod) {
        callback?.onContinueLoadingRequested(this)
    }
}

/** A sound's samples, moved by its period's delay. */
@OptIn(UnstableApi::class)
internal class DelayedSampleStream(
    val inner: SampleStream,
    private val period: AudioDelayPeriod,
) : SampleStream {
    override fun isReady(): Boolean = inner.isReady

    override fun maybeThrowError() = inner.maybeThrowError()

    override fun readData(formatHolder: FormatHolder, buffer: DecoderInputBuffer, readFlags: Int): Int {
        val result = inner.readData(formatHolder, buffer, readFlags)
        if (result == C.RESULT_BUFFER_READ && !buffer.isEndOfStream) {
            buffer.timeUs = movedTime(buffer.timeUs, period.appliedUs)
        }
        return result
    }

    override fun skipData(positionUs: Long): Int = inner.skipData(positionUs - period.appliedUs)

    override fun getFlags(): Int = inner.flags
}

/** A sample's time moved by `delayUs`; the ends of time stay where they are. */
internal fun movedTime(timeUs: Long, delayUs: Long): Long =
    if (timeUs == C.TIME_UNSET || timeUs == C.TIME_END_OF_SOURCE) timeUs else timeUs + delayUs

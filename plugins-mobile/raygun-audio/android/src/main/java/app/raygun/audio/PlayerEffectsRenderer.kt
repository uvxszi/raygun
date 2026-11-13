package app.raygun.audio

import android.content.Context
import android.os.Handler
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.mediacodec.MediaCodecAdapter
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.video.MediaCodecVideoRenderer
import androidx.media3.exoplayer.video.PlaybackVideoGraphWrapper
import androidx.media3.exoplayer.video.VideoFrameReleaseControl
import androidx.media3.exoplayer.video.VideoRendererEventListener

/**
 * How the video renderer's effect graph is built, chosen when effects first start in a player.
 * With the replay cache, Media3 keeps the last frames before the effects, so a paused picture can
 * be drawn again with new values (`setVideoEffects(VideoFrameProcessor.REDRAW)`); the cache lets
 * only two frames past it until one is shown, which leaves smooth motion little time to make the
 * frames after each one, so a graph built while smooth motion is on goes without it (paused
 * changes then come through a seek). The graph is built once per player.
 */
internal class VideoGraphChoice {
    /** Asked for by [PlayerEffects] right before effects start. */
    @Volatile
    var wantReplay = true

    /** The renderer's graph was built with the replay cache: redraws are possible. */
    @Volatile
    var replayable = false
}

/** Media3's video renderer, its effect graph built as [choice] says. */
@UnstableApi
private class EffectsVideoRenderer(
    builder: Builder,
    private val choice: VideoGraphChoice,
) : MediaCodecVideoRenderer(builder) {
    override fun createPlaybackVideoGraphWrapper(
        context: Context,
        releaseControl: VideoFrameReleaseControl,
    ): PlaybackVideoGraphWrapper {
        val replay = choice.wantReplay
        choice.replayable = replay
        return PlaybackVideoGraphWrapper.Builder(context, releaseControl)
            .setEnablePlaylistMode(true)
            .setEnableReplayableCache(replay)
            .setClock(clock)
            .build()
    }
}

/** As `DefaultRenderersFactory`'s. */
private const val MAX_DROPPED_TO_NOTIFY = 50

/** Puts [EffectsVideoRenderer] in place of the default factory's MediaCodec video renderer. */
@UnstableApi
internal fun replaceVideoRenderer(
    out: ArrayList<Renderer>,
    context: Context,
    adapterFactory: MediaCodecAdapter.Factory,
    selector: MediaCodecSelector,
    joiningMs: Long,
    fallback: Boolean,
    handler: Handler,
    listener: VideoRendererEventListener,
    choice: VideoGraphChoice,
) {
    val index = out.indexOfFirst { it is MediaCodecVideoRenderer }
    if (index < 0) return
    val builder = MediaCodecVideoRenderer.Builder(context)
        .setCodecAdapterFactory(adapterFactory)
        .setMediaCodecSelector(selector)
        .setAllowedJoiningTimeMs(joiningMs)
        .setEnableDecoderFallback(fallback)
        .setEventHandler(handler)
        .setEventListener(listener)
        .setMaxDroppedFramesToNotify(MAX_DROPPED_TO_NOTIFY)
    out[index] = EffectsVideoRenderer(builder, choice)
}

package app.raygun.audio

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.media3.common.VideoFrameProcessor
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.mediacodec.MediaCodecAdapter
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.video.MediaCodecVideoRenderer
import androidx.media3.exoplayer.video.PlaybackVideoGraphWrapper
import androidx.media3.exoplayer.video.VideoFrameReleaseControl
import androidx.media3.exoplayer.video.VideoRendererEventListener
import androidx.media3.exoplayer.video.VideoSink

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

/**
 * When the player's loop runs again after a paused redraw was asked for: paused, Media3 looks at
 * its renderers only once a second, so the redrawn frame is pulled out soon after the effects
 * passes make it.
 */
internal val REDRAW_WAKEUPS_MS = longArrayOf(8, 24, 48, 96, 200, 400)

/** A paused redraw that brought no frame by then gives up waiting for one. */
internal const val REDRAW_GIVE_UP_MS = 1_000L

/**
 * What a redraw asked for while paused does to the frame release: Media3's release control holds
 * every frame after the first while the renderer isn't started, so a redrawn frame would wait for
 * playback to resume. The redraw makes its first frame a "first frame" again, released at once
 * (joining keeps the renderer ready meanwhile, so the player doesn't read as buffering).
 */
internal fun redrawReleasesAtOnce(effects: Any?, started: Boolean): Boolean =
    !started && effects == VideoFrameProcessor.REDRAW

/** Media3's video renderer, its effect graph built as [choice] says. */
@UnstableApi
private class EffectsVideoRenderer(
    builder: Builder,
    private val choice: VideoGraphChoice,
) : MediaCodecVideoRenderer(builder) {
    private var releaseControl: VideoFrameReleaseControl? = null
    private var wakeup: Renderer.WakeupListener? = null
    private var playbackHandler: Handler? = null

    override fun createPlaybackVideoGraphWrapper(
        context: Context,
        releaseControl: VideoFrameReleaseControl,
    ): PlaybackVideoGraphWrapper {
        this.releaseControl = releaseControl
        val replay = choice.wantReplay
        choice.replayable = replay
        return PlaybackVideoGraphWrapper.Builder(context, releaseControl)
            .setEnablePlaylistMode(true)
            .setEnableReplayableCache(replay)
            .setClock(clock)
            .build()
    }

    override fun handleMessage(messageType: Int, message: Any?) {
        if (messageType == Renderer.MSG_SET_WAKEUP_LISTENER) wakeup = message as? Renderer.WakeupListener
        super.handleMessage(messageType, message)
        // On the playback thread, after the sink took the redraw (its flush may set the release
        // instruction of the stream again).
        if (messageType == Renderer.MSG_SET_VIDEO_EFFECTS && redrawReleasesAtOnce(message, state == Renderer.STATE_STARTED)) {
            releaseRedrawn()
        }
    }

    // A seek, playing or the renderer going: a redraw's wakeups and give-up no longer apply (the
    // give-up would hold back the first frame after a seek).
    override fun onPositionReset(positionUs: Long, joining: Boolean, sampleStreamIsResetToKeyFrame: Boolean) {
        playbackHandler?.removeCallbacksAndMessages(null)
        super.onPositionReset(positionUs, joining, sampleStreamIsResetToKeyFrame)
    }

    override fun onStarted() {
        playbackHandler?.removeCallbacksAndMessages(null)
        super.onStarted()
    }

    override fun onDisabled() {
        playbackHandler?.removeCallbacksAndMessages(null)
        super.onDisabled()
    }

    private fun releaseRedrawn() {
        val control = releaseControl ?: return
        control.onStreamChanged(VideoSink.RELEASE_FIRST_FRAME_IMMEDIATELY)
        control.join(true)
        val looper = Looper.myLooper() ?: return
        val handler = playbackHandler ?: Handler(looper).also { playbackHandler = it }
        handler.removeCallbacksAndMessages(null)
        REDRAW_WAKEUPS_MS.forEach { delay -> handler.postDelayed({ wakeup?.onWakeup() }, delay) }
        // No frame came of it (nothing kept to replay): the frame shown stays the first one, so
        // the renderer never waits for one that won't come.
        handler.postDelayed({ if (state != Renderer.STATE_STARTED) control.onFrameReleasedIsFirstFrame() }, REDRAW_GIVE_UP_MS)
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

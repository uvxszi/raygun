package app.raygun.audio

import android.content.Context
import android.media.AudioDeviceInfo
import android.media.audiofx.PresetReverb
import android.os.Handler
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.AuxEffectInfo
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/** Puts a [SoundProcessor] in the player's audio sink. */
@UnstableApi
private class SoundRenderersFactory(context: Context, private val processor: SoundProcessor) :
    DefaultRenderersFactory(context) {
    override fun buildAudioSink(
        context: Context,
        enableFloatOutput: Boolean,
        enableAudioTrackPlaybackParams: Boolean,
    ): AudioSink =
        DefaultAudioSink.Builder(context)
            .setAudioProcessors(arrayOf(processor))
            .setEnableFloatOutput(enableFloatOutput)
            .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
            .build()
}

/** A player and the processor its sound goes through. */
@UnstableApi
class Deck(val player: ExoPlayer, val processor: SoundProcessor) {
    /** Stops the deck once its fade-out is over, unless it's reused meanwhile. */
    var stopAt: Runnable? = null
}

/**
 * Owns the native players. Media3 promotes this to a foreground service while something plays,
 * which keeps the process alive in the background and provides the media notification and
 * lock-screen controls.
 *
 * Two players take turns (M46.12): a crossfade starts the next song on the other one, hands it
 * to the media session, and fades the old one out. Only the session's player holds audio focus,
 * or the new one's request would pause the old one.
 */
@UnstableApi
class RaygunPlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private val decks = mutableListOf<Deck>()
    private var current = 0
    private val main = Handler(Looper.getMainLooper())
    private var reverb: PresetReverb? = null
    private val audioAttributes = AudioAttributes.Builder()
        .setUsage(C.USAGE_MEDIA)
        .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
        .build()

    override fun onCreate() {
        super.onCreate()
        repeat(2) { index ->
            val processor = SoundProcessor().apply { setConfig(RaygunSound.config) }
            val player = ExoPlayer.Builder(this, SoundRenderersFactory(this, processor))
                .setAudioAttributes(audioAttributes, /* handleAudioFocus = */ index == 0)
                .setHandleAudioBecomingNoisy(true)
                .setWakeMode(C.WAKE_MODE_NETWORK)
                .build()
            // The output picked in Devices (M46.43), for players made after it.
            RaygunSound.output?.let { player.setPreferredAudioDevice(it) }
            decks.add(Deck(player, processor))
        }
        session = MediaSession.Builder(this, RaygunSessionPlayer(decks[0].player)).build()
        instance = this
        applyReverb()
    }

    /** The deck in the session: what plays now. */
    val deck: Deck get() = decks[current]

    /** Plays through `device` (M46.43: Bluetooth headphones, the speaker…), or Android's choice. */
    fun applyOutput(device: AudioDeviceInfo?) {
        for (each in decks) each.player.setPreferredAudioDevice(device)
    }

    /** New sound settings: every player's processor follows them, and the reverb. */
    fun applySound(config: SoundConfig, reverbPreset: Short?) {
        for (each in decks) each.processor.setConfig(config)
        RaygunSound.reverb = reverbPreset
        applyReverb()
    }

    /** Android's preset reverbs live on the output mix; each player sends to it. */
    private fun applyReverb() {
        val preset = RaygunSound.reverb
        if (preset == null) {
            for (each in decks) each.player.setAuxEffectInfo(AuxEffectInfo(AuxEffectInfo.NO_AUX_EFFECT_ID, 0f))
            reverb?.release()
            reverb = null
            return
        }
        val effect = reverb ?: runCatching { PresetReverb(0, 0) }.getOrNull()?.also { reverb = it } ?: return
        runCatching {
            effect.preset = preset
            effect.enabled = true
        }
        for (each in decks) each.player.setAuxEffectInfo(AuxEffectInfo(effect.id, 1f))
    }

    /**
     * Plays `item` on the other player, fading in over `fadeMs` while the current song fades out
     * (a crossfade). The session moves to the new player, so controls and reports follow it.
     */
    fun crossfade(item: MediaItem, startMs: Long, fadeMs: Long, gainDb: Double, measure: Boolean) {
        val old = deck
        val next = decks[1 - current]
        next.stopAt?.let(main::removeCallbacks)
        next.stopAt = null
        next.player.volume = old.player.volume
        next.player.playbackParameters = old.player.playbackParameters
        old.player.setAudioAttributes(audioAttributes, false)
        next.player.setAudioAttributes(audioAttributes, true)
        next.processor.startTrack(gainDb, fadeMs, measure)
        next.player.setMediaItem(item, startMs)
        next.player.prepare()
        next.player.playWhenReady = true
        current = 1 - current
        session?.player = RaygunSessionPlayer(next.player)
        retire(old, fadeMs)
    }

    /** Fades `deck` out over `fadeMs`, then stops it (another engine took over). */
    fun retire(deck: Deck, fadeMs: Long) {
        deck.processor.stopMeasuring()
        deck.processor.fade(0.0, fadeMs)
        val stop = Runnable {
            deck.stopAt = null
            deck.player.stop()
            deck.player.clearMediaItems()
        }
        deck.stopAt = stop
        main.postDelayed(stop, fadeMs + 100)
    }

    /** A song loaded the usual way (cut, not crossfaded); a fading-out player stops now. */
    fun startTrack(gainDb: Double, measure: Boolean) {
        // This player is reused now: a fade-out it was in the middle of no longer stops it.
        deck.stopAt?.let(main::removeCallbacks)
        deck.stopAt = null
        val other = decks[1 - current]
        other.stopAt?.let {
            main.removeCallbacks(it)
            it.run()
        }
        deck.processor.startTrack(gainDb, 0, measure)
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: android.content.Intent?) {
        // Swiping the app away while paused ends the service; while playing, music continues.
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        instance = null
        main.removeCallbacksAndMessages(null)
        session?.release()
        session = null
        for (each in decks) each.player.release()
        decks.clear()
        reverb?.release()
        reverb = null
        super.onDestroy()
    }

    companion object {
        /** The running service, for the plugin in the same process (crossfades, sound). */
        @Volatile
        var instance: RaygunPlaybackService? = null
    }
}

/** The sound settings, kept for players made later (the service may start after they're set). */
object RaygunSound {
    @Volatile
    var config = SoundConfig()

    /** A `PresetReverb` preset, or null for none. */
    @Volatile
    var reverb: Short? = null

    /** The output picked in Devices (M46.43), or null for Android's own choice. */
    @Volatile
    var output: AudioDeviceInfo? = null
}

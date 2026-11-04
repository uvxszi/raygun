package app.raygun.audio

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.media.audiofx.PresetReverb
import android.os.Handler
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.AuxEffectInfo
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * The locks a player holds while it plays: a song from a file (or the proxy's `/local/` route)
 * needs only the CPU; a stream also keeps Wi-Fi awake. Nothing is held while paused or stopped.
 */
@UnstableApi
private class WakeFollower(private val player: ExoPlayer) : Player.Listener {
    private var mode = C.WAKE_MODE_NONE

    override fun onEvents(player: Player, events: Player.Events) {
        if (events.containsAny(
                Player.EVENT_PLAY_WHEN_READY_CHANGED,
                Player.EVENT_MEDIA_ITEM_TRANSITION,
                Player.EVENT_PLAYBACK_STATE_CHANGED,
            )
        ) {
            apply()
        }
    }

    fun apply() {
        val active = player.playWhenReady &&
            player.playbackState != Player.STATE_IDLE &&
            player.playbackState != Player.STATE_ENDED
        val uri = player.currentMediaItem?.localConfiguration?.uri
        val wanted = when {
            !active || uri == null -> C.WAKE_MODE_NONE
            isLocal(uri) -> C.WAKE_MODE_LOCAL
            else -> C.WAKE_MODE_NETWORK
        }
        if (wanted != mode) {
            mode = wanted
            player.setWakeMode(wanted)
        }
    }

    private fun isLocal(uri: android.net.Uri): Boolean = when (uri.scheme) {
        "file", "content", "android.resource" -> true
        "http" -> (uri.host == "127.0.0.1" || uri.host == "localhost") &&
            uri.path.orEmpty().startsWith("/local/")
        else -> false
    }
}

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

/** The connection hint the plugin's own controller carries: its commands drive the players. */
const val ENGINE_CONTROLLER_HINT = "app.raygun.audio.engine"

/** The extra on intents that open Raygun at a screen (`player`, `/downloads?tab=music`). */
const val EXTRA_OPEN_ROUTE = "app.raygun.audio.ROUTE"

/** Opens Raygun at `route`. */
fun openRaygunIntent(context: Context, route: String): PendingIntent? {
    val launch = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
    launch.putExtra(EXTRA_OPEN_ROUTE, route)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
    return PendingIntent.getActivity(
        context,
        route.hashCode(),
        launch,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
}

/**
 * Owns the native players. Media3 promotes this to a foreground service while something plays,
 * which keeps the process alive in the background and provides the media notification and
 * lock-screen controls.
 *
 * Two players take turns (M46.12): a crossfade starts the next song on the other one, hands it
 * to the media session, and fades the old one out. Only the session's player holds audio focus,
 * or the new one's request would pause the old one.
 *
 * While the native engine isn't the one playing (a song in the webview's YouTube player, or the
 * next song while its stream is found), the session shows [mirror] instead: Rust's player as it
 * reports itself, so the notification stays and Raygun stays in the foreground.
 */
@UnstableApi
class RaygunPlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private val decks = mutableListOf<Deck>()
    /** Each deck's player as the session shows it, made once (each listens to its player). */
    private val shown = mutableListOf<RaygunSessionPlayer>()
    private var current = 0
    private val main = Handler(Looper.getMainLooper())
    private var reverb: PresetReverb? = null
    private val audioAttributes = AudioAttributes.Builder()
        .setUsage(C.USAGE_MEDIA)
        .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
        .build()
    lateinit var mirror: MirrorPlayer
        private set

    /** Forgets the output picked in Devices once it's gone (headphones switched off). */
    private val outputs = object : AudioDeviceCallback() {
        override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) {
            val chosen = RaygunSound.output ?: return
            if (removed.any { it.id == chosen.id }) {
                RaygunSound.output = null
                applyOutput(null)
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        repeat(2) { index ->
            val processor = SoundProcessor().apply { setConfig(RaygunSound.config) }
            val player = ExoPlayer.Builder(this, SoundRenderersFactory(this, processor))
                .setAudioAttributes(audioAttributes, /* handleAudioFocus = */ index == 0)
                .setHandleAudioBecomingNoisy(true)
                .build()
            // Wake and Wi-Fi locks follow what plays (WakeFollower), not a fixed mode.
            player.addListener(WakeFollower(player))
            // The output picked in Devices (M46.43), for players made after it.
            RaygunSound.output?.let { player.setPreferredAudioDevice(it) }
            decks.add(Deck(player, processor))
            shown.add(RaygunSessionPlayer(player, ::fromRaygun))
        }
        mirror = MirrorPlayer(this)
        val builder = MediaSession.Builder(this, shown[0])
        // Tapping the notification opens the full player.
        openRaygunIntent(this, "player")?.let(builder::setSessionActivity)
        session = builder.build()
        instance = this
        applyReverb()
        getSystemService(AudioManager::class.java)?.registerAudioDeviceCallback(outputs, main)
    }

    /**
     * Whether the player call being handled comes from Raygun: the plugin's controller, or the
     * service and Media3 themselves (no controller). Anything else is the system or a device.
     */
    private fun fromRaygun(): Boolean {
        val caller = session?.controllerForCurrentRequest ?: return true
        return caller.connectionHints.getBoolean(ENGINE_CONTROLLER_HINT, false)
    }

    /** The deck in the session: what plays now. */
    val deck: Deck get() = decks[current]

    /** The session shows the native player again (the native engine loaded a song). */
    fun useDeck() {
        val player = shown[current]
        if (session?.player !== player) session?.player = player
        mirror.standDown()
    }

    /** The session shows Rust's player as it reports itself (the native engine holds nothing). */
    fun useMirror() {
        if (session?.player !== mirror) session?.player = mirror
    }

    /** Plays through `device` (M46.43: Bluetooth headphones, the speakerâ€¦), or Android's choice. */
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
        session?.player = shown[current]
        mirror.standDown()
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

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Swiping the app away while paused ends the service; while playing, music continues.
        val player = session?.player
        if (player == null || !player.playWhenReady || player.playbackState == Player.STATE_IDLE) {
            // Also ends the foreground state Media3 keeps for a while after a pause.
            pauseAllPlayersAndStopSelf()
        }
    }

    override fun onDestroy() {
        instance = null
        main.removeCallbacksAndMessages(null)
        getSystemService(AudioManager::class.java)?.unregisterAudioDeviceCallback(outputs)
        session?.release()
        session = null
        mirror.shutDown()
        for (each in decks) each.player.release()
        decks.clear()
        shown.clear()
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

package app.raygun.audio

import android.media.AudioDeviceInfo
import android.media.AudioRouting
import android.media.AudioTrack
import android.os.Build
import android.os.Handler
import android.os.SystemClock
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioOutput
import androidx.media3.exoplayer.audio.AudioOutputProvider
import androidx.media3.exoplayer.audio.AudioTrackAudioOutput
import androidx.media3.exoplayer.audio.ForwardingAudioOutput
import androidx.media3.exoplayer.audio.ForwardingAudioOutputProvider

/*
 * Where the sound picked in Devices goes (M46.43).
 *
 * ExoPlayer gives each AudioTrack the picked output (`setPreferredAudioDevice`), and Android
 * routes a track that starts with one there. A track that is already playing when the pick
 * changes is re-created with it, but Android's audio policy routes an output to a preferred
 * device only while every track playing on it asks for that device, and the replaced track
 * (without the preference) still plays for a moment when the new one starts: the sound stays
 * where it was, wired headphones keeping it from the speaker. A track that pauses until Android
 * lets the old one go and then plays again is routed afresh. [RoutedOutput.hold] does that below
 * ExoPlayer, which keeps seeing it play; [OutputKeeper] checks where each track ends up, holds it
 * again when it isn't where it was asked to go, and says when Android keeps it elsewhere.
 */

/**
 * What an output is, for Devices (M46.43): `speaker`, `wired`, `bluetooth`, `usb` or `hdmi`;
 * null for outputs nobody picks (the earpiece, calls).
 */
fun outputKindOf(type: Int): String? = when (type) {
    AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "speaker"
    AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET -> "wired"
    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "bluetooth"
    AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_USB_HEADSET -> "usb"
    AudioDeviceInfo.TYPE_HDMI, AudioDeviceInfo.TYPE_HDMI_ARC -> "hdmi"
    else -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
        (type == AudioDeviceInfo.TYPE_BLE_HEADSET || type == AudioDeviceInfo.TYPE_BLE_SPEAKER)
    ) "bluetooth" else null
}

/** Wired, Bluetooth and USB audio: newly connected, they take the sound from a pick. */
fun takesOverSound(type: Int): Boolean = when (outputKindOf(type)) {
    "wired", "bluetooth", "usb" -> true
    else -> false
}

/** Whether `a` and `b` are the same output (ids, or the same kind and address). */
fun sameOutput(a: AudioDeviceInfo?, b: AudioDeviceInfo?): Boolean {
    if (a == null || b == null) return false
    if (a.id == b.id) return true
    if (a.type != b.type) return false
    return Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && a.address == b.address
}

/** A deck's AudioTracks as ExoPlayer makes them, each followed by a [RoutedOutput]. */
@UnstableApi
class RoutedOutputProvider(base: AudioOutputProvider, private val watch: RouteWatch) :
    ForwardingAudioOutputProvider(base) {
    override fun getAudioOutput(config: AudioOutputProvider.OutputConfig): AudioOutput {
        val output = super.getAudioOutput(config)
        val track = (output as? AudioTrackAudioOutput)?.audioTrack ?: return output
        return RoutedOutput(output, track, watch)
    }
}

/** The track a deck plays through now, and what it reports about its routing. */
@UnstableApi
class RouteWatch(val handler: Handler, private val onChange: () -> Unit) {
    @Volatile
    var current: RoutedOutput? = null
        private set

    fun attach(output: RoutedOutput) {
        current = output
        handler.post { onChange() }
    }

    fun detach(output: RoutedOutput) {
        if (current === output) current = null
    }

    /** Android routed `output` somewhere (it started, or the device it plays on changed). */
    fun changed(output: RoutedOutput) {
        if (current === output) onChange()
    }
}

/**
 * One AudioTrack of a deck: where Android routes it, and [hold], which pauses it for a moment
 * and plays it again without ExoPlayer seeing a pause, so Android routes it afresh.
 */
@UnstableApi
class RoutedOutput(
    base: AudioOutput,
    private val track: AudioTrack,
    private val watch: RouteWatch,
) : ForwardingAudioOutput(base) {
    private val lock = Any()
    /** What ExoPlayer last asked for: playing or not. */
    private var playing = false
    private var released = false
    /** Paused by [hold], to be played again. */
    private var held = false
    /** How often this track was held, and when last. */
    var holds = 0
        private set
    var heldAt = 0L
        private set
    private val listener = AudioRouting.OnRoutingChangedListener { watch.changed(this) }

    init {
        track.addOnRoutingChangedListener(listener, watch.handler)
        watch.attach(this)
    }

    override fun play() = synchronized(lock) {
        playing = true
        held = false
        super.play()
    }

    override fun pause() = synchronized(lock) {
        playing = false
        held = false
        super.pause()
    }

    override fun stop() = synchronized(lock) {
        // The end of the stream plays out: a held track plays again first.
        if (held) runCatching { track.play() }
        held = false
        super.stop()
    }

    override fun flush() = synchronized(lock) {
        held = false
        super.flush()
    }

    override fun release() {
        synchronized(lock) {
            released = true
            playing = false
            held = false
        }
        runCatching { track.removeOnRoutingChangedListener(listener) }
        watch.detach(this)
        super.release()
    }

    /** ExoPlayer plays it (a hold under way counts as playing). */
    val isPlaying: Boolean get() = synchronized(lock) { playing && !released }

    /** Where Android sends this track now, when it says. */
    fun routedDevice(): AudioDeviceInfo? = synchronized(lock) {
        if (released) null else runCatching { track.routedDevice }.getOrNull()
    }

    /**
     * Pauses the track for `ms` and plays it again, unless ExoPlayer paused, stopped or let it go
     * meanwhile; then `after` runs. Nothing happens to a track that doesn't play. A hold that
     * isn't `counted` (the one that takes a new pick) leaves [holds] as it was.
     */
    fun hold(ms: Long, counted: Boolean = true, after: () -> Unit = {}): Boolean {
        synchronized(lock) {
            if (!playing || released || held) return false
            held = true
            if (counted) holds += 1
            heldAt = SystemClock.elapsedRealtime()
            runCatching { track.pause() }
        }
        watch.handler.postDelayed({
            synchronized(lock) {
                if (held && playing && !released) runCatching { track.play() }
                held = false
            }
            after()
        }, ms)
        return true
    }
}

/**
 * Keeps the native players' sound on the output picked in Devices: a playing track is held while
 * it takes a new pick, and each track Android routes elsewhere is held again, a few times at most.
 * [refused] says Android kept the sound elsewhere all the same.
 */
@UnstableApi
class OutputKeeper(private val main: Handler, private val playing: () -> RoutedOutput?) {
    /** The picked output was asked for and Android kept the playing song elsewhere. */
    @Volatile
    var refused = false
        private set

    private val check = Runnable { check() }

    /** The output picked changed: the playing track is held while it takes it (see the file). */
    fun picked(apply: () -> Unit) {
        refused = false
        main.removeCallbacks(check)
        // Held first, the playing track takes the pick while paused: Android lets the track it
        // replaces go before the new one starts.
        val held = RaygunSound.output != null &&
            playing()?.hold(HOLD_MS, counted = false) { schedule(SETTLE_MS) } == true
        apply()
        if (!held) schedule(SETTLE_MS)
    }

    /** Where a track goes changed, or a new one started: looked at once it has settled. */
    fun routingChanged() = schedule(SETTLE_MS)

    private fun schedule(delayMs: Long) {
        main.removeCallbacks(check)
        main.postDelayed(check, delayMs)
    }

    private fun check() {
        val wanted = RaygunSound.output
        val output = playing()
        if (wanted == null || output == null || !output.isPlaying) {
            if (wanted == null) refused = false
            return
        }
        val routed = output.routedDevice() ?: return
        val verdict = routeVerdict(
            sameOutput(routed, wanted),
            output.holds,
            SystemClock.elapsedRealtime() - output.heldAt,
        )
        if (verdict != RouteVerdict.Kept) {
            android.util.Log.i(
                "RaygunAudio",
                "output ${wanted.type}/${wanted.id} picked, track on ${routed.type}/${routed.id}: $verdict",
            )
        }
        when (verdict) {
            RouteVerdict.Kept -> refused = false
            RouteVerdict.Hold -> output.hold(HOLD_MS, counted = true) { schedule(SETTLE_MS) }
            RouteVerdict.Later -> {
                refused = true
                schedule(RETRY_MS)
            }
            RouteVerdict.Refused -> refused = true
        }
    }

    fun stop() = main.removeCallbacks(check)

    companion object {
        /** How long a track pauses to be routed afresh. */
        const val HOLD_MS = 250L
        /** How long after a track starts, or its routing changes, where it went is looked at. */
        const val SETTLE_MS = 400L
        /** A track routed elsewhere is held again after this long, [MAX_HOLDS] times at most. */
        const val RETRY_MS = 4_000L
        const val MAX_HOLDS = 3
    }
}

/** What [OutputKeeper] does about a playing track. */
enum class RouteVerdict { Kept, Hold, Later, Refused }

/**
 * A playing track on the picked output is kept; one elsewhere is held to be routed afresh, again
 * a while after the last hold, until [OutputKeeper.MAX_HOLDS] holds were tried.
 */
fun routeVerdict(onPicked: Boolean, holds: Int, sinceHoldMs: Long): RouteVerdict = when {
    onPicked -> RouteVerdict.Kept
    holds == 0 -> RouteVerdict.Hold
    holds >= OutputKeeper.MAX_HOLDS -> RouteVerdict.Refused
    sinceHoldMs >= OutputKeeper.RETRY_MS -> RouteVerdict.Hold
    else -> RouteVerdict.Later
}

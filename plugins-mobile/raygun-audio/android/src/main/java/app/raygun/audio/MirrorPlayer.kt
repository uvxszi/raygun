package app.raygun.audio

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioManager
import android.net.Uri
import android.os.Looper
import android.os.SystemClock
import androidx.core.content.ContextCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import org.json.JSONObject

/** Rust's player as the plugin last heard of it (`MediaSnapshot` in `raygun-app/src/background.rs`). */
data class MirrorState(
    val itemId: String,
    val title: String,
    val artist: String?,
    val album: String?,
    val artworkUrl: String?,
    /** `loading`, `buffering`, `playing`, `paused`, `ended` or `error`. */
    val status: String,
    val positionMs: Long,
    val durationMs: Long?,
    val rate: Float,
    /** The webview's engine holds the item (YouTube). */
    val web: Boolean,
    /** When [positionMs] was true, on the elapsed-realtime clock. */
    val at: Long,
) {
    /** Something plays, or is about to. */
    val active: Boolean get() = status == "playing" || status == "loading" || status == "buffering"

    companion object {
        fun parse(json: JSONObject?): MirrorState? {
            if (json == null) return null
            return MirrorState(
                itemId = json.optString("itemId"),
                title = json.optString("title"),
                artist = json.optString("artist").takeIf { json.has("artist") && !json.isNull("artist") },
                album = json.optString("album").takeIf { json.has("album") && !json.isNull("album") },
                artworkUrl = json.optString("artworkUrl")
                    .takeIf { json.has("artworkUrl") && !json.isNull("artworkUrl") },
                status = json.optString("status", "paused"),
                positionMs = json.optDouble("positionMs", 0.0).toLong(),
                durationMs = if (json.isNull("durationMs")) null else json.optDouble("durationMs").toLong(),
                rate = json.optDouble("rate", 1.0).toFloat(),
                web = json.optBoolean("web", false),
                at = SystemClock.elapsedRealtime(),
            )
        }
    }
}

/**
 * Whether unplugged headphones pause Rust's player through the mirror: only while the session
 * shows it for a webview song that plays or is about to (the deck's player pauses itself).
 */
internal fun noisyPauses(shown: Boolean, state: MirrorState?): Boolean =
    shown && state != null && state.web && state.active

private val MIRROR_COMMANDS = Player.Commands.Builder()
    .addAll(
        Player.COMMAND_PLAY_PAUSE,
        Player.COMMAND_STOP,
        Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
        Player.COMMAND_SEEK_TO_NEXT,
        Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
        Player.COMMAND_SEEK_TO_PREVIOUS,
        Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        Player.COMMAND_SEEK_BACK,
        Player.COMMAND_SEEK_FORWARD,
        Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
        Player.COMMAND_GET_TIMELINE,
        Player.COMMAND_GET_METADATA,
    )
    .build()

/**
 * Raygun's player as Rust sees it, for the media session while the native engine isn't the one
 * playing: a song in the webview's YouTube player, or the next song while its stream is found.
 * The session (notification, lock screen, headsets, Bluetooth) shows it and keeps Raygun in the
 * foreground; its buttons go to Rust like the native engine's.
 *
 * It never asks for Android's audio focus. The webview takes the focus itself once its media is
 * audible (Chromium's media session, which Android's WebView keeps) and pauses that media when the
 * focus goes elsewhere, so a call or another app pauses it and its engine tells Rust. A request
 * from here took the focus from the webview or lost it to the webview: the webview's request as a
 * YouTube song started reached Rust from here as a pause, and asking again when Play was pressed
 * paused the webview's media. Unplugged headphones are the one thing the webview doesn't act on,
 * so this passes them on ([noisyPauses]).
 */
@UnstableApi
class MirrorPlayer(private val context: Context) : SimpleBasePlayer(Looper.getMainLooper()) {
    private var state: MirrorState? = null
    private var noisyRegistered = false
    /**
     * The session shows this player. Only then does it pass on headphone changes: while the deck
     * plays, a pause from here would stop the deck's song (its player handles noise itself).
     */
    private var shown = false

    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY && noisyPauses(shown, state)) {
                NativeBridge.control("pause")
            }
        }
    }

    /** Whether the webview's engine holds the item and something plays or is about to. */
    val webActive: Boolean get() = state?.let { it.web && it.status != "ended" && it.status != "error" } == true

    val current: MirrorState? get() = state

    fun update(next: MirrorState?) {
        state = next
        follow()
        invalidateState()
    }

    /** The session shows this player now. */
    fun show() {
        shown = true
        follow()
    }

    /** The headphone watch follows what the webview plays, while this is shown. */
    private fun follow() {
        listenForNoisy(noisyPauses(shown, state))
    }

    /** The native engine plays now: its player handles focus and noise itself. */
    fun standDown() {
        shown = false
        listenForNoisy(false)
    }

    private fun listenForNoisy(on: Boolean) {
        if (on == noisyRegistered) return
        noisyRegistered = on
        if (on) {
            ContextCompat.registerReceiver(
                context,
                noisy,
                IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        } else {
            runCatching { context.unregisterReceiver(noisy) }
        }
    }

    fun shutDown() {
        standDown()
    }

    override fun getState(): State {
        val shown = state ?: return State.Builder()
            .setAvailableCommands(MIRROR_COMMANDS)
            .setPlaybackState(Player.STATE_IDLE)
            .build()
        val metadata = MediaMetadata.Builder()
            .setTitle(shown.title)
            .setArtist(shown.artist)
            .setAlbumTitle(shown.album)
            .setArtworkUri(shown.artworkUrl?.let(Uri::parse))
            .build()
        val item = MediaItemData.Builder(shown.itemId)
            .setMediaItem(MediaItem.Builder().setMediaId(shown.itemId).setMediaMetadata(metadata).build())
            .setMediaMetadata(metadata)
            .setDurationUs(shown.durationMs?.let { it * 1000 } ?: C.TIME_UNSET)
            .setIsSeekable(shown.durationMs != null)
            .build()
        val playing = shown.status == "playing"
        val position = if (playing) {
            PositionSupplier.getExtrapolating(
                shown.positionMs + ((SystemClock.elapsedRealtime() - shown.at) * shown.rate).toLong(),
                shown.rate,
            )
        } else {
            PositionSupplier.getConstant(shown.positionMs)
        }
        return State.Builder()
            .setAvailableCommands(MIRROR_COMMANDS)
            .setPlaylist(listOf(item))
            .setCurrentMediaItemIndex(0)
            .setPlayWhenReady(shown.active, Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE)
            .setPlaybackState(
                when (shown.status) {
                    "playing", "paused" -> Player.STATE_READY
                    "loading", "buffering" -> Player.STATE_BUFFERING
                    "ended" -> Player.STATE_ENDED
                    else -> Player.STATE_IDLE
                },
            )
            .setContentPositionMs(position)
            .setPlaybackParameters(PlaybackParameters(shown.rate))
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        NativeBridge.control(if (playWhenReady) "play" else "pause")
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        NativeBridge.control("pause")
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> NativeBridge.control("next")
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM ->
                NativeBridge.control("previous")
            else -> if (positionMs != C.TIME_UNSET) NativeBridge.control("seek:${positionMs.coerceAtLeast(0)}")
        }
        return Futures.immediateVoidFuture()
    }
}

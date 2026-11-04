package app.raygun.audio

import androidx.media3.common.C
import androidx.media3.common.ForwardingSimpleBasePlayer
import androidx.media3.common.Player
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

private val QUEUE_COMMANDS = intArrayOf(
    Player.COMMAND_SEEK_TO_NEXT,
    Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
    Player.COMMAND_SEEK_TO_PREVIOUS,
    Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
)

/**
 * The player the media session exposes (M09). The session only ever holds the current item, but
 * Raygun's queue lives in Rust, so this player always offers next/previous and hands those buttons
 * (notification, lock screen, headset, Bluetooth) to Rust instead of seeking itself.
 *
 * Play, pause, stop and seeks from those go to Rust too: Rust's player decides (a pause it didn't
 * hear of would leave it saying "playing", and a play past the user's own pause would be paused
 * again as a late start), then tells the engine. Only Raygun's own controller ([fromRaygun]: the
 * plugin's, or the service itself) drives the player directly.
 *
 * Built on [ForwardingSimpleBasePlayer] rather than `ForwardingPlayer`: it derives events from
 * state, so changing the advertised commands can't drop or garble listener callbacks.
 */
class RaygunSessionPlayer(player: Player, private val fromRaygun: () -> Boolean) :
    ForwardingSimpleBasePlayer(player) {
    override fun getState(): State {
        val state = super.getState()
        return state.buildUpon()
            .setAvailableCommands(state.availableCommands.buildUpon().addAll(*QUEUE_COMMANDS).build())
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (fromRaygun()) return super.handleSetPlayWhenReady(playWhenReady)
        NativeBridge.control(if (playWhenReady) "play" else "pause")
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        if (fromRaygun()) return super.handleStop()
        NativeBridge.control("pause")
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> =
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> {
                NativeBridge.control("next")
                Futures.immediateVoidFuture()
            }
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> {
                NativeBridge.control("previous")
                Futures.immediateVoidFuture()
            }
            else -> if (fromRaygun() || positionMs == C.TIME_UNSET) {
                super.handleSeek(mediaItemIndex, positionMs, seekCommand)
            } else {
                NativeBridge.control("seek:${positionMs.coerceAtLeast(0)}")
                Futures.immediateVoidFuture()
            }
        }
}

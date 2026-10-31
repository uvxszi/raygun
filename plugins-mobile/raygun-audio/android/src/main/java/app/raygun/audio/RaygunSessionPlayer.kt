package app.raygun.audio

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
 * Built on [ForwardingSimpleBasePlayer] rather than `ForwardingPlayer`: it derives events from
 * state, so changing the advertised commands can't drop or garble listener callbacks.
 */
class RaygunSessionPlayer(player: Player) : ForwardingSimpleBasePlayer(player) {
    override fun getState(): State {
        val state = super.getState()
        return state.buildUpon()
            .setAvailableCommands(state.availableCommands.buildUpon().addAll(*QUEUE_COMMANDS).build())
            .build()
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
            else -> super.handleSeek(mediaItemIndex, positionMs, seekCommand)
        }
}

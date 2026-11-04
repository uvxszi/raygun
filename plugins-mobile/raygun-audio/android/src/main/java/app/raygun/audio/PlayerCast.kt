package app.raygun.audio

import android.Manifest
import android.content.pm.PackageManager
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Typeface
import android.os.Build
import android.text.InputType
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject

/** The volume bar's steps: 5 % each. */
private const val VOLUME_STEPS = 20

/**
 * Play on TV: the devices sheet (⋮ More → Play on TV), adding a TV by its address, and the remote
 * that shows while a TV plays the video. Finding devices and playing on them is Rust's (`cast` in
 * raygun-app): this asks with `cast*` requests and follows the `castDevices`, `cast` and
 * `castError` updates. While a TV plays, the player's own copy is stopped and isn't reported.
 */
internal class PlayerCast(private val host: VideoPlayerActivity) {
    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    /** What plays on the TV, as Rust last said; null when nothing does. */
    private var status: JSONObject? = null
    val active: Boolean get() = status != null
    private var devices = JSONArray()
    private var searching = false
    private var devicesSheet: PlayerSheet? = null
    private var seeking = false
    private var volumeDragging = false

    private val title = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        typeface = Typeface.DEFAULT_BOLD
        maxLines = 2
    }
    private val device = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
        typeface = Typeface.DEFAULT_BOLD
        gravity = Gravity.CENTER
    }
    private val state = TextView(host).apply {
        setTextColor(0xB3FFFFFF.toInt())
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
        gravity = Gravity.CENTER
    }
    private val note = TextView(host).apply {
        setTextColor(0xE6FFFFFF.toInt())
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
        gravity = Gravity.CENTER
        visibility = View.GONE
    }
    private val time = TextView(host).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
    }
    private val seekBar = SeekBar(host).apply {
        max = 1000
        progressTintList = ColorStateList.valueOf(playerAccent)
        thumbTintList = ColorStateList.valueOf(playerAccent)
        setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                if (fromUser) time.text = timeText(progress * durationMs() / 1000)
            }

            override fun onStartTrackingTouch(bar: SeekBar) {
                seeking = true
            }

            override fun onStopTrackingTouch(bar: SeekBar) {
                seeking = false
                val duration = durationMs()
                if (duration > 0) seekTo(bar.progress * duration / 1000)
            }
        })
    }
    private val playButton: ImageButton = host.iconButton(R.drawable.raygun_ic_play, label("play", "Play"), 56) { toggle() }
    private val muteButton: ImageButton = host.iconButton(R.drawable.raygun_ic_volume, label("castMute", "Mute the TV")) {
        val muted = status?.optBoolean("muted") == true
        control(JSONObject().put("type", "mute").put("muted", !muted))
    }
    private val volumeBar = SeekBar(host).apply {
        max = VOLUME_STEPS
        progressTintList = ColorStateList.valueOf(playerAccent)
        thumbTintList = ColorStateList.valueOf(playerAccent)
        contentDescription = label("castTvVolume", "TV volume")
        setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {}

            override fun onStartTrackingTouch(bar: SeekBar) {
                volumeDragging = true
            }

            override fun onStopTrackingTouch(bar: SeekBar) {
                volumeDragging = false
                setVolume(bar.progress.toDouble() / VOLUME_STEPS)
            }
        })
    }
    private val volumeRow = LinearLayout(host).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        addView(muteButton)
        addView(volumeBar, LinearLayout.LayoutParams(0, -2, 1f))
        visibility = View.GONE
    }

    /** The remote, over the whole player while a TV plays. */
    val root = FrameLayout(host).apply {
        setBackgroundColor(Color.BLACK)
        isClickable = true
        visibility = View.GONE
        addView(LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(host.dp(8), host.dp(12), host.dp(16), 0)
            addView(host.iconButton(R.drawable.raygun_ic_back, label("back", "Back")) { host.leave() })
            addView(title, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = host.dp(8) })
        }, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
        addView(LinearLayout(host).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(host.dp(24), 0, host.dp(24), 0)
            addView(device)
            addView(state, LinearLayout.LayoutParams(-2, -2).apply { topMargin = host.dp(6) })
            addView(note, LinearLayout.LayoutParams(-2, -2).apply { topMargin = host.dp(6) })
        }, FrameLayout.LayoutParams(-1, -2, Gravity.CENTER))
        addView(LinearLayout(host).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(host.dp(16), 0, host.dp(16), host.dp(16))
            addView(seekBar, LinearLayout.LayoutParams(-1, -2))
            addView(time, LinearLayout.LayoutParams(-2, -2).apply { marginStart = host.dp(16) })
            addView(LinearLayout(host).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER
                addView(host.iconButton(R.drawable.raygun_ic_rewind, label("seekBack", "Back {n} s", (host.seekStepMs / 1000).toString()), 52) {
                    seekTo(positionMs() - host.seekStepMs)
                })
                addView(playButton)
                addView(host.iconButton(R.drawable.raygun_ic_forward, label("seekForward", "Forward {n} s", (host.seekStepMs / 1000).toString()), 52) {
                    seekTo(positionMs() + host.seekStepMs)
                })
            }, LinearLayout.LayoutParams(-1, -2))
            addView(volumeRow, LinearLayout.LayoutParams(-1, -2))
            addView(LinearLayout(host).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER
                setPadding(0, host.dp(8), 0, 0)
                addView(host.pill(label("castStop", "Stop")) { control(JSONObject().put("type", "stop")) })
                addView(host.pill(label("castDisconnect", "Disconnect"), filled = true) {
                    host.request("castDisconnect", JSONObject().put("resume", true))
                })
            }, LinearLayout.LayoutParams(-1, -2))
        }, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
    }

    // ---- Devices ----

    /** ⋮ More → Play on TV: asks for the local network first where Android wants it (17+). */
    fun open() {
        if (needsLocalNetwork()) {
            host.askLocalNetwork()
            return
        }
        searching = true
        host.request("castDevices")
        showDevices()
    }

    fun permissionAnswered(granted: Boolean) {
        if (granted) {
            open()
        } else {
            host.showHint(label("castDenied", "Allow local network access to play on a TV"), 3_500)
        }
    }

    private fun needsLocalNetwork(): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.CINNAMON_BUN &&
            host.applicationInfo.targetSdkVersion >= Build.VERSION_CODES.CINNAMON_BUN &&
            ContextCompat.checkSelfPermission(host, Manifest.permission.ACCESS_LOCAL_NETWORK) !=
            PackageManager.PERMISSION_GRANTED

    private fun kindName(kind: String): String = when (kind) {
        "googleCast" -> label("castGoogleCast", "Google Cast")
        "dlna" -> label("castDlna", "DLNA")
        else -> label("castAirplay", "AirPlay")
    }

    private fun showDevices() {
        val sheet = PlayerSheet(host, label("playOnTv", "Play on TV"))
        for (index in 0 until devices.length()) {
            val entry = devices.optJSONObject(index) ?: continue
            val detail = listOfNotNull(
                kindName(entry.optString("kind")),
                entry.text("model"),
                label("castAdded", "Added by address").takeIf { entry.optBoolean("added") },
            ).joinToString(" · ")
            sheet.row(entry.optString("name"), detail) {
                sheet.dismiss()
                start(entry.optString("id"))
            }
        }
        when {
            searching -> sheet.row(label("castSearching", "Looking for TVs…"), onClick = null)
            devices.length() == 0 -> sheet.row(label("castNothingFound", "No TVs found on this network"), onClick = null)
        }
        sheet.row(label("castAddByAddress", "Add by address")) { addByAddress() }
        devicesSheet = sheet
        sheet.show()
    }

    private fun addByAddress() {
        val sheet = PlayerSheet(host, label("castAddByAddress", "Add by address"))
        val field = EditText(host).apply {
            hint = "192.168.1.20"
            contentDescription = label("castAddress", "TV's address")
            setTextColor(Color.WHITE)
            setHintTextColor(0x80FFFFFF.toInt())
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_DONE
            isSingleLine = true
            backgroundTintList = ColorStateList.valueOf(playerAccent)
        }
        val add = {
            val address = field.text.toString().trim()
            if (address.isNotEmpty()) {
                sheet.dismiss()
                host.showHint(label("castConnecting", "Connecting…"))
                host.request("castAdd", JSONObject().put("address", address))
            }
        }
        field.setOnEditorActionListener { _, action, _ ->
            if (action == EditorInfo.IME_ACTION_DONE) add()
            action == EditorInfo.IME_ACTION_DONE
        }
        sheet.custom(LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(host.dp(20), host.dp(4), host.dp(16), host.dp(8))
            addView(field, LinearLayout.LayoutParams(0, -2, 1f))
            addView(host.pill(label("castAdd", "Add"), filled = true) { add() })
        })
        sheet.show()
        field.requestFocus()
    }

    private fun start(id: String) {
        val extra = JSONObject()
            .put("device", id)
            .put("positionMs", host.player.currentPosition.coerceAtLeast(0))
            .put("subtitle", host.chosenSubtitleIndex() ?: JSONObject.NULL)
        host.showHint(label("castConnecting", "Connecting…"))
        host.request("castStart", extra)
    }

    // ---- Updates from Rust ----

    fun onUpdate(update: JSONObject) {
        when (update.optString("type")) {
            "castDevices" -> {
                devices = update.optJSONArray("devices") ?: JSONArray()
                searching = update.optBoolean("searching")
                if (host.sheetShown(devicesSheet)) showDevices()
            }
            "cast" -> show(update.optJSONObject("status"))
            "castError" -> {
                host.hideHint()
                if (active) {
                    note.text = update.optString("text")
                    note.visibility = View.VISIBLE
                } else {
                    host.showHint(update.optString("text"), 4_000)
                }
            }
        }
    }

    /** A TV's status: the remote shows while there is one. */
    fun show(next: JSONObject?) {
        val was = active
        status = next
        if (next == null) {
            root.visibility = View.GONE
            return
        }
        if (!was) {
            host.hideHint()
            host.castStarted()
        }
        root.visibility = View.VISIBLE
        render()
        if (next.optString("state") == "finished") host.castFinished()
    }

    private fun render() {
        val status = status ?: return
        val name = status.optJSONObject("device")?.optString("name").orEmpty()
        val heading = status.text("subtitle")?.let { "${status.optString("title")}\n$it" } ?: status.optString("title")
        title.show(heading)
        device.show(label("castPlayingOn", "Playing on {n}", name))
        val current = status.optString("state")
        state.show(
            when (current) {
                "connecting" -> label("castConnecting", "Connecting…")
                "loading" -> label("castLoading", "Loading on the TV…")
                "playing" -> label("castPlaying", "Playing")
                "paused" -> label("castPaused", "Paused")
                "buffering" -> label("castBuffering", "Buffering…")
                "stopped" -> label("castStopped", "Stopped")
                "finished" -> label("castFinished", "Finished")
                else -> label("castFailed", "Couldn't play on the TV")
            },
        )
        val error = status.text("error")
        note.text = error.orEmpty()
        note.visibility = if (error == null) View.GONE else View.VISIBLE
        val playing = current == "playing" || current == "buffering"
        playButton.setImageResource(if (playing) R.drawable.raygun_ic_pause else R.drawable.raygun_ic_play)
        playButton.contentDescription = if (playing) label("pause", "Pause") else label("play", "Play")
        val hasVolume = !status.isNull("volume") && status.has("volume")
        volumeRow.visibility = if (hasVolume) View.VISIBLE else View.GONE
        if (hasVolume && !volumeDragging) {
            volumeBar.progress = (status.optDouble("volume", 0.0) * VOLUME_STEPS).toInt()
        }
        val muted = status.optBoolean("muted")
        muteButton.alpha = if (muted) 0.45f else 1f
        muteButton.contentDescription = if (muted) label("castUnmute", "Unmute the TV") else label("castMute", "Mute the TV")
        tick()
    }

    /** The moving position, each tick of the player. */
    fun tick() {
        if (!active || seeking) return
        val position = positionMs()
        val duration = durationMs()
        time.show(timeText(position))
        seekBar.progress = if (duration > 0) (position * 1000 / duration).toInt() else 0
    }

    private fun timeText(position: Long): String = "${formatTime(position)} / ${formatTime(durationMs())}"

    private fun durationMs(): Long = status?.let { if (it.isNull("durationMs")) 0L else it.optDouble("durationMs", 0.0).toLong() } ?: 0L

    /** Where the TV is: it moves on from its last report while playing. */
    private fun positionMs(): Long {
        val status = status ?: return 0
        val reported = status.optDouble("positionMs", 0.0)
        val moved = if (status.optString("state") == "playing") {
            (System.currentTimeMillis() - status.optDouble("atMs", System.currentTimeMillis().toDouble())).coerceAtLeast(0.0)
        } else {
            0.0
        }
        val position = (reported + moved).toLong()
        val duration = durationMs()
        return if (duration > 0) position.coerceAtMost(duration) else position
    }

    // ---- The remote's buttons ----

    private fun control(control: JSONObject) {
        note.visibility = View.GONE
        host.request("castControl", JSONObject().put("control", control))
    }

    fun toggle() {
        val playing = status?.optString("state").let { it == "playing" || it == "buffering" }
        control(JSONObject().put("type", if (playing) "pause" else "play"))
    }

    private fun seekTo(position: Long) {
        val duration = durationMs()
        val target = if (duration > 0) position.coerceIn(0, duration) else position.coerceAtLeast(0)
        control(JSONObject().put("type", "seek").put("positionMs", target))
    }

    private fun setVolume(level: Double) {
        control(JSONObject().put("type", "volume").put("level", level.coerceIn(0.0, 1.0)))
    }

    /** The phone's keys work the TV while it plays: volume and play/pause. */
    fun onKey(keyCode: Int): Boolean {
        if (!active) return false
        when (keyCode) {
            KeyEvent.KEYCODE_VOLUME_UP, KeyEvent.KEYCODE_VOLUME_DOWN -> {
                val status = status ?: return false
                if (status.isNull("volume") || !status.has("volume")) return false
                val step = if (keyCode == KeyEvent.KEYCODE_VOLUME_UP) 1 else -1
                val level = ((status.optDouble("volume", 0.0) * VOLUME_STEPS).toInt() + step).coerceIn(0, VOLUME_STEPS)
                volumeBar.progress = level
                setVolume(level.toDouble() / VOLUME_STEPS)
            }
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK, KeyEvent.KEYCODE_SPACE -> toggle()
            KeyEvent.KEYCODE_MEDIA_PLAY -> control(JSONObject().put("type", "play"))
            KeyEvent.KEYCODE_MEDIA_PAUSE -> control(JSONObject().put("type", "pause"))
            else -> return false
        }
        return true
    }
}

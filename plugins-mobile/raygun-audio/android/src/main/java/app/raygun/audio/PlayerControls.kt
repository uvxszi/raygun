package app.raygun.audio

import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.ui.DefaultTimeBar
import androidx.media3.ui.TimeBar

/**
 * The player's own controls (M45, after Aniyomi's): a top bar (back, title, episodes, sources,
 * audio, subtitles, more), previous · play · next in the middle, and at the bottom the seek bar
 * with chapter markers, then lock, rotate, speed and chapter on the left and aspect and
 * picture-in-picture on the right. The skip button floats above them. Taps that miss a button fall
 * through to the gestures underneath.
 */
internal class PlayerControls(private val host: VideoPlayerActivity) {
    private val context: Context = host

    private val title = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        maxLines = 2
    }
    /** The sleep timer's time left. */
    private val note = TextView(context).apply {
        setTextColor(0xCCFFFFFF.toInt())
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
        visibility = View.GONE
    }
    val episodesButton: ImageButton =
        context.iconButton(R.drawable.raygun_ic_episodes, host.label("episodes", "Episodes")) { host.sheets.episodes() }
    val sourcesButton: ImageButton =
        context.iconButton(R.drawable.raygun_ic_sources, host.label("sources", "Sources")) { host.sheets.sources() }
    private val audioButton =
        context.iconButton(R.drawable.raygun_ic_audio, host.label("audio", "Audio")) { host.sheets.audio() }
    private val subtitlesButton =
        context.iconButton(R.drawable.raygun_ic_subtitles, host.label("subtitles", "Subtitles")) { host.sheets.subtitles() }
    private val moreButton =
        context.iconButton(R.drawable.raygun_ic_more, host.label("more", "More")) { host.sheets.more() }
    private val topBar = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        background = GradientDrawable(
            GradientDrawable.Orientation.TOP_BOTTOM,
            intArrayOf(0xB3000000.toInt(), 0x00000000),
        )
        setPadding(context.dp(8), context.dp(8), context.dp(8), context.dp(24))
        addView(context.iconButton(R.drawable.raygun_ic_back, host.label("back", "Back")) { host.leave() })
        addView(LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(context.dp(8), 0, context.dp(8), 0)
            addView(title)
            addView(note)
        }, LinearLayout.LayoutParams(0, -2, 1f))
        addView(episodesButton)
        addView(sourcesButton)
        addView(audioButton)
        addView(subtitlesButton)
        addView(moreButton)
    }

    private val previousButton =
        context.iconButton(R.drawable.raygun_ic_previous, host.label("previous", "Previous episode"), 56) { host.request("previous") }
    private val playButton =
        context.iconButton(R.drawable.raygun_ic_pause, host.label("pause", "Pause"), 72) { togglePlay() }
    private val nextButton =
        context.iconButton(R.drawable.raygun_ic_next, host.label("next", "Next episode"), 56) { host.requestNext() }
    private val buffering = ProgressBar(context).apply { visibility = View.GONE }
    private val centre = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER
        addView(previousButton)
        addView(FrameLayout(context).apply {
            addView(playButton, FrameLayout.LayoutParams(context.dp(72), context.dp(72), Gravity.CENTER))
            addView(buffering, FrameLayout.LayoutParams(context.dp(72), context.dp(72), Gravity.CENTER))
        }, LinearLayout.LayoutParams(context.dp(96), context.dp(96)).apply {
            marginStart = context.dp(32)
            marginEnd = context.dp(32)
        })
        addView(nextButton)
    }

    private val position = timeText()
    private val duration = timeText().apply {
        isClickable = true
        setOnClickListener { host.toggleRemaining() }
    }
    val timeBar = DefaultTimeBar(context).apply {
        setPlayedColor(ACCENT)
        setScrubberColor(ACCENT)
        setBufferedColor(0x66FFFFFF)
        setUnplayedColor(0x33FFFFFF)
        setAdMarkerColor(0xFFFFD54F.toInt())
        setPlayedAdMarkerColor(0x99FFD54F.toInt())
    }
    private val lockButton =
        context.iconButton(R.drawable.raygun_ic_lock, host.label("lock", "Lock")) { lock() }
    private val rotateButton =
        context.iconButton(R.drawable.raygun_ic_rotate, host.label("rotate", "Rotate")) { host.rotate() }
    private val speedChip = context.pill("1×") { host.cycleSpeed() }.apply {
        setOnLongClickListener {
            host.sheets.speed()
            true
        }
    }
    private val chapterChip = context.pill("") { host.sheets.chapters() }.apply {
        maxWidth = context.dp(200)
        maxLines = 1
        visibility = View.GONE
    }
    private val aspectButton =
        context.iconButton(R.drawable.raygun_ic_aspect, host.label("aspect", "Aspect")) { host.cycleAspect() }
    private val pipButton =
        context.iconButton(R.drawable.raygun_ic_pip, host.label("pip", "Picture-in-picture")) { host.enterPip() }
    private val bottomBar = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        background = GradientDrawable(
            GradientDrawable.Orientation.BOTTOM_TOP,
            intArrayOf(0xB3000000.toInt(), 0x00000000),
        )
        setPadding(context.dp(12), context.dp(24), context.dp(12), context.dp(8))
        addView(LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(position)
            addView(timeBar, LinearLayout.LayoutParams(0, context.dp(32), 1f))
            addView(duration)
        })
        addView(LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(lockButton)
            addView(rotateButton)
            addView(speedChip)
            addView(chapterChip)
            addView(View(context), LinearLayout.LayoutParams(0, 1, 1f))
            addView(aspectButton)
            addView(pipButton)
        })
    }

    private val unlockButton = context.iconButton(R.drawable.raygun_ic_lock_open, host.label("unlock", "Unlock"), 52) { unlock() }.apply {
        background = rounded(0x99000000.toInt(), context.dp(26).toFloat())
        visibility = View.GONE
    }

    /** The skip button: "Skip opening", a countdown, or "+85 s". */
    val skipButton: TextView = context.pill("") { host.onSkipTapped() }.apply {
        setOnLongClickListener {
            host.sheets.introLength()
            true
        }
        visibility = View.GONE
    }
    /** "Don't skip", beside the countdown. */
    val dontSkipButton: TextView = context.pill(host.label("dontSkip", "Don't skip")) { host.cancelSkipCountdown() }.apply {
        visibility = View.GONE
    }
    private val skipRow = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        addView(dontSkipButton)
        addView(skipButton)
    }

    val root = FrameLayout(context).apply {
        addView(topBar, FrameLayout.LayoutParams(-1, -2, Gravity.TOP))
        addView(centre, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
        addView(bottomBar, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
        val skipPlace = FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM or Gravity.END)
        skipPlace.marginEnd = context.dp(24)
        skipPlace.bottomMargin = context.dp(32)
        addView(skipRow, skipPlace)
        val unlockPlace = FrameLayout.LayoutParams(context.dp(52), context.dp(52), Gravity.TOP or Gravity.START)
        unlockPlace.marginStart = context.dp(24)
        unlockPlace.topMargin = context.dp(24)
        addView(unlockButton, unlockPlace)
    }

    var visible = false
        private set
    var locked = false
        private set
    private var scrubbing = false
    private var scrubPosition = 0L
    private val hideLater = Runnable { if (!scrubbing) hide() }

    init {
        timeBar.addListener(object : TimeBar.OnScrubListener {
            override fun onScrubStart(timeBar: TimeBar, position: Long) {
                scrubbing = true
                scrubPosition = position
                host.main.removeCallbacks(hideLater)
                update()
            }

            override fun onScrubMove(timeBar: TimeBar, position: Long) {
                scrubPosition = position
                update()
            }

            override fun onScrubStop(timeBar: TimeBar, position: Long, canceled: Boolean) {
                scrubbing = false
                if (!canceled) host.player.seekTo(position)
                bump()
            }
        })
        // Room for a camera cutout or rounded corners.
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
            view.setPadding(cutout.left, cutout.top, cutout.right, cutout.bottom)
            insets
        }
        hide()
    }

    private fun timeText() = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
        setPadding(context.dp(8), 0, context.dp(8), 0)
        text = formatTime(0)
    }

    fun setTitle(text: String) {
        title.text = text
    }

    /** Episodes and previous/next for a session with more than one episode; Sources for any. */
    fun setSession(session: Boolean, several: Boolean, hasPrevious: Boolean, hasNext: Boolean) {
        episodesButton.visibility = if (session && several) View.VISIBLE else View.GONE
        sourcesButton.visibility = if (session) View.VISIBLE else View.GONE
        previousButton.visibility = if (session && several) View.VISIBLE else View.INVISIBLE
        nextButton.visibility = if (session && several) View.VISIBLE else View.INVISIBLE
        previousButton.isEnabled = hasPrevious
        previousButton.alpha = if (hasPrevious) 1f else 0.35f
        nextButton.isEnabled = hasNext
        nextButton.alpha = if (hasNext) 1f else 0.35f
    }

    /** Marks chapter starts (and the ends of skippable ones) on the seek bar. */
    fun setChapters(chapters: List<Chapter>) {
        val marks = chapters
            .flatMap { chapter -> listOfNotNull(chapter.startMs, chapter.endMs.takeIf { chapter.skippable }) }
            .filter { it > 0 }
            .distinct()
            .sorted()
        timeBar.setAdGroupTimesMs(marks.toLongArray(), BooleanArray(marks.size), marks.size)
        chapterChip.visibility = if (chapters.isEmpty()) View.GONE else View.VISIBLE
    }

    fun setNote(text: String?) {
        note.text = text ?: ""
        note.visibility = if (text.isNullOrBlank()) View.GONE else View.VISIBLE
    }

    /** Refreshes times, the play button, the speed and the chapter; runs a few times a second. */
    fun update() {
        val player = host.player
        val length = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0L
        val now = if (scrubbing) scrubPosition else player.currentPosition
        if (!scrubbing) {
            timeBar.setPosition(now)
            timeBar.setBufferedPosition(player.bufferedPosition)
        }
        timeBar.setDuration(length)
        position.text = formatTime(now)
        duration.text = when {
            length <= 0 -> "--:--"
            host.prefs.showRemaining -> "-" + formatTime(length - now)
            else -> formatTime(length)
        }
        val playing = player.playWhenReady && player.playbackState != Player.STATE_ENDED
        playButton.setImageResource(if (playing) R.drawable.raygun_ic_pause else R.drawable.raygun_ic_play)
        playButton.contentDescription = host.label(if (playing) "pause" else "play", if (playing) "Pause" else "Play")
        buffering.visibility = if (player.playbackState == Player.STATE_BUFFERING) View.VISIBLE else View.GONE
        speedChip.text = formatSpeed(player.playbackParameters.speed)
        chapterChip.text = host.chapterAt(now)
            ?.let { (index, chapter) -> host.chapterName(chapter, index) }
            ?: host.label("chapters", "Chapters")
    }

    private fun togglePlay() {
        val player = host.player
        when {
            player.playbackState == Player.STATE_ENDED -> {
                player.seekTo(0)
                player.play()
            }
            player.playWhenReady -> player.pause()
            else -> player.play()
        }
        update()
        bump()
    }

    fun show() {
        visible = true
        if (locked) {
            unlockButton.visibility = View.VISIBLE
        } else {
            topBar.visibility = View.VISIBLE
            centre.visibility = View.VISIBLE
            bottomBar.visibility = View.VISIBLE
        }
        placeSkip()
        host.liftSubtitles(!locked)
        update()
        bump()
    }

    fun hide() {
        visible = false
        host.main.removeCallbacks(hideLater)
        topBar.visibility = View.GONE
        centre.visibility = View.GONE
        bottomBar.visibility = View.GONE
        unlockButton.visibility = View.GONE
        placeSkip()
        host.liftSubtitles(false)
    }

    fun toggle() = if (visible) hide() else show()

    /** Restarts the timeout; controls stay up while paused or loading. */
    fun bump() {
        host.main.removeCallbacks(hideLater)
        if (host.player.isPlaying && !host.isLoading()) host.main.postDelayed(hideLater, host.prefs.controlsMs)
    }

    /** Shows the top bar alone (while a video is found). */
    fun showTopOnly() {
        topBar.visibility = View.VISIBLE
    }

    private fun lock() {
        locked = true
        hide()
        unlockButton.visibility = View.VISIBLE
        visible = true
        bump()
        host.showHint(host.label("locked", "Controls locked"))
    }

    private fun unlock() {
        locked = false
        unlockButton.visibility = View.GONE
        show()
    }

    /** The skip button sits above the bottom bar while the controls show. */
    private fun placeSkip() {
        val params = skipRow.layoutParams as FrameLayout.LayoutParams
        params.bottomMargin = context.dp(if (visible && !locked) 112 else 32)
        skipRow.layoutParams = params
    }
}

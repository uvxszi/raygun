package app.raygun.audio

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.ui.DefaultTimeBar
import androidx.media3.ui.TimeBar

/**
 * The player's own controls (M45, after Aniyomi's): a top bar (back, title, episodes, sources,
 * audio, more), previous · back · play · forward · next in the middle, and at the bottom the seek
 * bar with chapter markers (a live stream's DVR window, with its LIVE badge), then lock, rotate,
 * speed and chapter on the left and quality, subtitles, aspect and picture-in-picture on the
 * right. The skip button floats above them. Taps that miss a button fall through to the gestures
 * underneath. While the video waits, the loading ring takes the play button's place.
 */
internal class PlayerControls(private val host: VideoPlayerActivity) {
    private val context: Context = host

    private val title = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        maxLines = 2
        ellipsize = TextUtils.TruncateAt.END
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
        context.iconButton(R.drawable.raygun_ic_audio, host.label("audio", "Audio")) { host.sheets.audio() }.apply {
            // Shown once the video turns out to have more than one audio track.
            visibility = View.GONE
        }
    private val subtitlesButton =
        context.iconButton(R.drawable.raygun_ic_subtitles, host.label("subtitles", "Subtitles")) { host.sheets.subtitles() }
    private val moreButton =
        context.iconButton(R.drawable.raygun_ic_more, host.label("more", "More")) { host.sheets.more() }
    // The bars draw no shade of their own: the player's vignette covers the window
    // ([PlayerVignette]).
    private val topBar = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
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
        addView(moreButton)
    }

    private val previousButton =
        context.iconButton(R.drawable.raygun_ic_previous, host.label("previous", "Previous episode"), 48) { host.request("previous") }
    private val playButton =
        context.iconButton(R.drawable.raygun_ic_pause, host.label("pause", "Pause"), 72) { togglePlay() }
    private val nextButton =
        context.iconButton(R.drawable.raygun_ic_next, host.label("next", "Next episode"), 48) { host.requestNext() }
    private val backButton = seekButton(back = true)
    private val forwardButton = seekButton(back = false)
    // The same space on both sides of each button keeps the play button at the screen's centre,
    // where the loading ring shows, with or without previous and next.
    private val centre = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER
        addView(previousButton, spaced(48))
        addView(backButton, spaced(52))
        addView(playButton, spaced(72))
        addView(forwardButton, spaced(52))
        addView(nextButton, spaced(48))
    }

    private val position = timeText()
    private val duration = timeText().apply {
        isClickable = true
        setOnClickListener { host.toggleRemaining() }
    }
    val timeBar = DefaultTimeBar(context).apply {
        setPlayedColor(playerAccent)
        setScrubberColor(playerAccent)
        setBufferedColor(0x66FFFFFF)
        setUnplayedColor(0x33FFFFFF)
        setAdMarkerColor(0xFFFFD54F.toInt())
        setPlayedAdMarkerColor(0x99FFD54F.toInt())
    }
    private val lockButton =
        context.iconButton(R.drawable.raygun_ic_lock, host.label("lock", "Lock")) { lock() }
    /** A tap turns portrait ⇄ landscape; holding keeps the screen exactly as it is now. */
    private val rotateButton =
        context.iconButton(R.drawable.raygun_ic_rotate, host.label("rotateHint", "Rotate. Hold to keep this way")) {
            host.rotate()
        }.apply {
            setOnLongClickListener {
                host.lockRotation()
                true
            }
        }
    private val speedChip = context.pill("1×") { host.cycleSpeed() }.apply {
        setOnLongClickListener {
            host.sheets.speed()
            true
        }
    }
    private val chapterChip = context.pill("") { host.sheets.chapters() }.apply {
        maxWidth = context.dp(200)
        maxLines = 1
        ellipsize = TextUtils.TruncateAt.END
        visibility = View.GONE
    }
    private val aspectButton =
        context.iconButton(R.drawable.raygun_ic_aspect, host.label("aspect", "Aspect")) { host.cycleAspect() }
    private val pipButton =
        context.iconButton(R.drawable.raygun_ic_pip, host.label("pip", "Picture-in-picture")) { host.enterPip() }
    /** Resolutions from this stream or the session's other sources ([PlayerQuality]). */
    private val qualityButton =
        context.iconButton(R.drawable.raygun_ic_quality, host.label("quality", "Quality")) { host.quality.open() }.apply {
            visibility = View.GONE
        }
    private val bottomBar = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(context.dp(12), context.dp(24), context.dp(12), context.dp(8))
        addView(LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(position)
            addView(timeBar, LinearLayout.LayoutParams(0, context.dp(32), 1f))
            addView(duration)
            addView(host.liveStream.badge, LinearLayout.LayoutParams(-2, -2).apply { marginEnd = context.dp(4) })
        })
        addView(LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(lockButton)
            addView(rotateButton)
            addView(speedChip)
            addView(chapterChip)
            addView(View(context), LinearLayout.LayoutParams(0, 1, 1f))
            // Quality, then subtitles, beside the picture's aspect.
            addView(qualityButton)
            addView(subtitlesButton)
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
    val skipRow = LinearLayout(context).apply {
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
    private var shownPlaying: Boolean? = null
    private var busy = false
    private var problem = false
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
        setSeekStep(host.seekStepMs)
        hide()
    }

    private fun timeText() = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
        setPadding(context.dp(8), 0, context.dp(8), 0)
        text = formatTime(0)
    }

    private fun spaced(size: Int) = LinearLayout.LayoutParams(context.dp(size), context.dp(size)).apply {
        marginStart = context.dp(7)
        marginEnd = context.dp(7)
    }

    /** A circular arrow with the seek step inside it ("10"). */
    private fun seekButton(back: Boolean): FrameLayout = FrameLayout(context).apply {
        val ripple = TypedValue()
        context.theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, ripple, true)
        setBackgroundResource(ripple.resourceId)
        isClickable = true
        isFocusable = true
        addView(ImageView(context).apply {
            setImageResource(if (back) R.drawable.raygun_ic_replay else R.drawable.raygun_ic_replay_forward)
            setColorFilter(Color.WHITE)
        }, FrameLayout.LayoutParams(context.dp(40), context.dp(40), Gravity.CENTER))
        addView(TextView(context).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            // The arrow's circle sits a little below the middle of the icon.
            translationY = context.dp(2).toFloat()
        }, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
        setOnClickListener {
            host.seekBy(if (back) -host.seekStepMs else host.seekStepMs)
            bump()
        }
    }

    /** The step the middle buttons jump, shown inside them. */
    fun setSeekStep(stepMs: Long) {
        val seconds = (stepMs / 1000).toString()
        listOf(backButton to "seekBack", forwardButton to "seekForward").forEach { (button, key) ->
            (button.getChildAt(1) as TextView).show(seconds)
            button.contentDescription = host.label(
                key,
                if (key == "seekBack") "Back {n} s" else "Forward {n} s",
                seconds,
            )
        }
    }

    fun setTitle(text: String) {
        title.show(text)
    }

    /** Episodes and previous/next for a session with more than one episode; Sources for any. */
    fun setSession(session: Boolean, several: Boolean, hasPrevious: Boolean, hasNext: Boolean) {
        episodesButton.visibility = if (session && several) View.VISIBLE else View.GONE
        sourcesButton.visibility = if (session) View.VISIBLE else View.GONE
        previousButton.visibility = if (session && several) View.VISIBLE else View.GONE
        nextButton.visibility = if (session && several) View.VISIBLE else View.GONE
        previousButton.isEnabled = hasPrevious
        previousButton.alpha = if (hasPrevious) 1f else 0.35f
        nextButton.isEnabled = hasNext
        nextButton.alpha = if (hasNext) 1f else 0.35f
    }

    /** The audio button shows when there's a choice of audio tracks. */
    fun setAudioChoice(several: Boolean) {
        audioButton.visibility = if (several) View.VISIBLE else View.GONE
    }

    /** The quality button shows in a session (other sources' sizes) or with variants to pick. */
    fun setQualityChoice(offered: Boolean) {
        qualityButton.visibility = if (offered) View.VISIBLE else View.GONE
    }

    /** The rotate button shows that the screen is kept as it is. */
    fun setRotationLocked(locked: Boolean) {
        rotateButton.setImageResource(if (locked) R.drawable.raygun_ic_rotate_lock else R.drawable.raygun_ic_rotate)
        rotateButton.setColorFilter(if (locked) playerAccent else Color.WHITE)
        rotateButton.contentDescription = if (locked) {
            host.label("rotationLockedHint", "Rotation locked. Tap to unlock")
        } else {
            host.label("rotateHint", "Rotate. Hold to keep this way")
        }
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
        note.show(text ?: "")
        note.visibility = if (text.isNullOrBlank()) View.GONE else View.VISIBLE
    }

    /** While the video waits, the loading ring shows where the play button is. */
    fun setBusy(waiting: Boolean) {
        if (busy == waiting) return
        busy = waiting
        playButton.visibility = if (waiting) View.INVISIBLE else View.VISIBLE
    }

    /** While a problem shows in the middle, the middle buttons make way for it. */
    fun setProblem(shown: Boolean) {
        problem = shown
        centre.visibility = if (visible && !locked && !shown) View.VISIBLE else View.GONE
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
        host.liveStream.update()
        if (host.liveStream.isLive) {
            // The DVR window to seek in (hidden when there's too little), how far behind live.
            timeBar.visibility = if (host.liveStream.seekable()) View.VISIBLE else View.INVISIBLE
            position.show(host.liveStream.positionText())
            duration.visibility = View.GONE
        } else {
            timeBar.visibility = View.VISIBLE
            duration.visibility = View.VISIBLE
            position.show(formatTime(now))
            duration.show(
                when {
                    length <= 0 -> "--:--"
                    host.prefs.showRemaining -> "-" + formatTime(length - now)
                    else -> formatTime(length)
                },
            )
        }
        val playing = player.playWhenReady && player.playbackState != Player.STATE_ENDED
        if (shownPlaying != playing) {
            shownPlaying = playing
            playButton.setImageResource(if (playing) R.drawable.raygun_ic_pause else R.drawable.raygun_ic_play)
            playButton.contentDescription = host.label(if (playing) "pause" else "play", if (playing) "Pause" else "Play")
        }
        speedChip.show(formatSpeed(player.playbackParameters.speed))
        if (chapterChip.visibility == View.VISIBLE) {
            chapterChip.show(
                host.chapterAt(now)
                    ?.let { (index, chapter) -> host.chapterName(chapter, index) }
                    ?: host.label("chapters", "Chapters"),
            )
        }
    }

    private fun togglePlay() {
        host.togglePlay()
        bump()
    }

    fun show() {
        visible = true
        if (locked) {
            unlockButton.visibility = View.VISIBLE
        } else {
            topBar.visibility = View.VISIBLE
            centre.visibility = if (problem) View.GONE else View.VISIBLE
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

    /** How high the skip button and the next-episode prompt sit: above the bottom bar when shown. */
    fun bottomOffset(): Int = context.dp(if (visible && !locked) 112 else 32)

    /** The bottom bar's height with what's below it (a cutout), for lifting subtitles above it. */
    fun barHeight(): Int = (bottomBar.height.takeIf { it > 0 } ?: context.dp(108)) + root.paddingBottom

    /** The skip button sits above the bottom bar while the controls show. */
    private fun placeSkip() {
        val params = skipRow.layoutParams as FrameLayout.LayoutParams
        params.bottomMargin = bottomOffset()
        skipRow.layoutParams = params
        host.placeNextPrompt()
    }
}

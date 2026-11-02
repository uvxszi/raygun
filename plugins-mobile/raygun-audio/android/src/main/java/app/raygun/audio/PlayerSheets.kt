package app.raygun.audio

import android.content.res.ColorStateList
import android.graphics.Color
import android.util.TypedValue
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import androidx.media3.common.C
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

/** Colours offered for subtitles in the player, as in Settings (0xAARRGGBB and a label key). */
private val TEXT_COLORS = listOf(
    0xFFFFFFFF.toInt() to "colorWhite",
    0xFFFFEB3B.toInt() to "colorYellow",
    0xFF80DEEA.toInt() to "colorCyan",
    0xFFA5D6A7.toInt() to "colorGreen",
    0xFFBDBDBD.toInt() to "colorGrey",
    0xFF000000.toInt() to "colorBlack",
)
private val BACKGROUNDS = listOf(
    0x00000000 to "colorNone",
    0x80000000.toInt() to "colorBlackHalf",
    0xBF000000.toInt() to "colorBlackMost",
    0xFF000000.toInt() to "colorBlack",
)
private val EDGES = listOf("none", "outline", "shadow", "raised", "depressed")
private val SLEEP_MINUTES = listOf(15, 30, 45, 60, 90)
private val INTRO_SECONDS = listOf(0, 30, 60, 75, 85, 90, 100, 110, 120, 150, 180)

/** The player's sheets (M45): what the top bar, the chips and ⋮ More open. */
internal class PlayerSheets(private val host: VideoPlayerActivity) {
    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    fun episodes() {
        val episodes = host.episodes
        if (episodes.length() == 0) {
            // Not here yet: ask again.
            host.showHint(label("findingSource", "Finding a source…"))
            host.request("ready")
            return
        }
        val sheet = PlayerSheet(host, label("episodes", "Episodes"))
        val autoplay = host.autoplay
        sheet.row(
            label(if (autoplay) "autoplayOn" else "autoplayOff", if (autoplay) "Autoplay: on" else "Autoplay: off"),
        ) {
            host.autoplay = !host.autoplay
            host.request("autoplay", JSONObject().put("on", host.autoplay))
            episodes()
        }
        for (index in 0 until episodes.length()) {
            val episode = episodes.optJSONObject(index) ?: continue
            val marks = buildString {
                if (episode.optBoolean("watched")) append("✓ ")
                if (episode.optBoolean("downloaded")) append("⤓ ")
            }.trim()
            val current = episode.optBoolean("current")
            sheet.row(episode.optString("label"), marks.ifBlank { null }, current) {
                sheet.dismiss()
                if (!current) host.request("episode", JSONObject().put("index", index))
            }
        }
        sheet.show()
    }

    fun sources() {
        val groups = host.sources
        val sheet = PlayerSheet(host, label("sources", "Sources"))
        var shown = 0
        for (group in 0 until groups.length()) {
            val server = groups.optJSONObject(group) ?: continue
            val links = server.optJSONArray("links") ?: JSONArray()
            val state = server.optString("state")
            if (links.length() == 0) {
                val hidden = (state == "failed" && !host.prefs.showFailedSources) ||
                    (state == "empty" && !host.prefs.showEmptySources)
                if (hidden) continue
                val detail = when (state) {
                    "resolving" -> label("resolving", "finding links…")
                    "failed" -> label("failed", "failed") +
                        server.optString("error").let { if (it.isBlank()) "" else ": $it" }
                    else -> label("empty", "no videos")
                }
                sheet.row(server.optString("name"), detail, onClick = null)
                shown += 1
                continue
            }
            // Failed lately and tried last (M46.17).
            val unreliable = if (server.optBoolean("unreliable")) {
                " · " + label("unreliable", "didn't work lately")
            } else {
                ""
            }
            sheet.header(server.optString("name") + unreliable)
            for (link in 0 until links.length()) {
                val entry = links.optJSONObject(link) ?: continue
                val current = entry.optBoolean("current")
                val failed = entry.optBoolean("failed")
                sheet.row(entry.optString("label"), if (failed) label("failed", "failed") else null, current) {
                    sheet.dismiss()
                    if (!current) {
                        host.showLoading(label("findingSource", "Finding a source…"))
                        host.request("source", JSONObject().put("group", group).put("link", link))
                    }
                }
                shown += 1
            }
        }
        if (shown == 0) {
            host.showHint(label("findingSource", "Finding a source…"))
            host.request("ready")
            return
        }
        sheet.show()
    }

    fun audio() {
        val tracks = host.tracks.list(C.TRACK_TYPE_AUDIO)
        val sheet = PlayerSheet(host, label("audio", "Audio"))
        if (tracks.isEmpty()) sheet.row(label("off", "Off"), onClick = null)
        tracks.forEachIndexed { index, track ->
            sheet.row(host.tracks.name(track, index + 1), host.tracks.audioDetail(track), track.selected) {
                host.tracks.select(C.TRACK_TYPE_AUDIO, track)
                host.state.audio = track.choice()
                host.remember(JSONObject().put("audio", track.choice().toJson()))
                sheet.dismiss()
            }
        }
        sheet.show()
    }

    fun subtitles() {
        val tracks = host.tracks.list(C.TRACK_TYPE_TEXT)
        val sheet = PlayerSheet(host, label("subtitles", "Subtitles"))
        val off = host.tracks.textOff()
        sheet.row(label("off", "Off"), selected = off) {
            host.tracks.select(C.TRACK_TYPE_TEXT, null)
            host.state.subtitleOff = true
            host.remember(JSONObject().put("subtitle", JSONObject().put("off", true)))
            subtitles()
        }
        tracks.forEachIndexed { index, track ->
            sheet.row(host.tracks.name(track, index + 1), selected = !off && track.selected) {
                host.tracks.select(C.TRACK_TYPE_TEXT, track)
                host.state.subtitleOff = false
                host.state.subtitle = track.choice()
                host.remember(JSONObject().put("subtitle", track.choice().toJson()))
                subtitles()
            }
        }
        sheet.row(label("addSubtitleFile", "Add a subtitle file")) {
            sheet.dismiss()
            host.pickSubtitleFile()
        }

        sheet.header(label("secondSubtitles", "Second subtitles"))
        val second = host.second.current
        sheet.row(label("off", "Off"), selected = second == null) {
            host.second.clear()
            host.remember(JSONObject().put("secondarySubtitle", JSONObject.NULL))
            subtitles()
        }
        host.sidecars.forEachIndexed { index, sidecar ->
            val name = sidecar.label?.takeIf { !sameLanguage(it, sidecar.language) }
                ?: languageName(sidecar.language)
                ?: sidecar.label
                ?: label("track", "Track {n}", (index + 1).toString())
            sheet.row(name, selected = second == sidecar) {
                host.loadSecond(sidecar)
                host.remember(JSONObject().put("secondarySubtitle", sidecar.choice().toJson()))
                subtitles()
            }
        }

        val delay = host.state.subtitleDelayMs
        val shown = String.format(Locale.ROOT, "%+.1f", delay / 1000.0)
        sheet.stepper(label("delay", "Delay"), "$shown s", listOf(
            "−1" to { host.setSubtitleDelay(delay - 1000); subtitles() },
            "−0.1" to { host.setSubtitleDelay(delay - 100); subtitles() },
            "+0.1" to { host.setSubtitleDelay(delay + 100); subtitles() },
            "+1" to { host.setSubtitleDelay(delay + 1000); subtitles() },
            "0" to { host.setSubtitleDelay(0); subtitles() },
        ))
        sheet.row(label("style", "Style")) { style() }
        sheet.show()
    }

    fun style() {
        val look = host.prefs.style
        val sheet = PlayerSheet(host, label("style", "Style"))
        sheet.stepper(label("size", "Size"), "${look.size}%", listOf(
            "−" to { host.updateStyle(look.copy(size = (look.size - 10).coerceAtLeast(50))); style() },
            "+" to { host.updateStyle(look.copy(size = (look.size + 10).coerceAtMost(200))); style() },
        ))
        sheet.stepper(label("height", "Height"), "${look.position}%", listOf(
            "−" to { host.updateStyle(look.copy(position = (look.position - 2).coerceAtLeast(0))); style() },
            "+" to { host.updateStyle(look.copy(position = (look.position + 2).coerceAtMost(40))); style() },
        ))
        val color = TEXT_COLORS.indexOfFirst { it.first == look.textColor }
        sheet.header(label("textColor", "Text colour"))
        sheet.chips(TEXT_COLORS.map { colorName(it.second) }, color) { picked ->
            host.updateStyle(look.copy(textColor = TEXT_COLORS[picked].first))
            style()
        }
        sheet.header(label("edge", "Edge"))
        sheet.chips(EDGES.map { edgeName(it) }, EDGES.indexOf(look.edge)) { picked ->
            host.updateStyle(look.copy(edge = EDGES[picked]))
            style()
        }
        sheet.header(label("background", "Background"))
        val background = BACKGROUNDS.indexOfFirst { it.first == look.backgroundColor }
        sheet.chips(BACKGROUNDS.map { colorName(it.second) }, background) { picked ->
            host.updateStyle(look.copy(backgroundColor = BACKGROUNDS[picked].first))
            style()
        }
        sheet.row(label("bold", "Bold"), selected = look.bold) {
            host.updateStyle(look.copy(bold = !look.bold))
            style()
        }
        sheet.row(label("italic", "Italic"), selected = look.italic) {
            host.updateStyle(look.copy(italic = !look.italic))
            style()
        }
        sheet.row(label("reset", "Reset")) {
            host.updateStyle(SubtitleLook())
            style()
        }
        sheet.show()
    }

    private fun colorName(key: String) = label(key, key.removePrefix("color"))

    private fun edgeName(edge: String) =
        label("edge" + edge.replaceFirstChar { it.uppercase() }, edge.replaceFirstChar { it.uppercase() })

    fun speed() {
        val player = host.player
        val sheet = PlayerSheet(host, label("speed", "Speed"))
        val current = player.playbackParameters.speed
        val value = TextView(host).apply {
            text = formatSpeed(current)
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 28f)
            setPadding(host.dp(20), 0, host.dp(20), 0)
        }
        sheet.custom(value)
        val slider = SeekBar(host).apply {
            max = (400 - 25) / 5
            progress = ((current * 100 - 25) / 5).toInt().coerceIn(0, max)
            progressTintList = ColorStateList.valueOf(ACCENT)
            thumbTintList = ColorStateList.valueOf(ACCENT)
            setPadding(host.dp(28), host.dp(12), host.dp(28), host.dp(12))
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                    if (!fromUser) return
                    val speed = (25 + progress * 5) / 100f
                    value.text = formatSpeed(speed)
                    host.setSpeed(speed)
                }

                override fun onStartTrackingTouch(bar: SeekBar) {}

                override fun onStopTrackingTouch(bar: SeekBar) {}
            })
        }
        sheet.custom(slider)
        val presets = host.prefs.speedPresets
        sheet.chips(presets.map { formatSpeed(it) }, presets.indexOfFirst { it == current }) { picked ->
            host.setSpeed(presets[picked])
            speed()
        }
        sheet.custom(LinearLayout(host).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(host.dp(12), host.dp(8), host.dp(20), host.dp(4))
            addView(host.pill(label("makeDefault", "Make default")) {
                val speed = host.player.playbackParameters.speed
                host.prefs.defaultSpeed = speed
                host.savePrefs(JSONObject().put("defaultSpeed", (speed * 100).toInt()))
                host.showHint(label("madeDefault", "Default speed: {n}", formatSpeed(speed)))
            })
            addView(host.pill(label("reset", "Reset")) {
                host.setSpeed(1f)
                speed()
            })
        })
        sheet.row(label("pitchCorrection", "Pitch correction"), selected = host.prefs.pitchCorrection) {
            host.prefs.pitchCorrection = !host.prefs.pitchCorrection
            host.savePrefs(JSONObject().put("pitchCorrection", host.prefs.pitchCorrection))
            host.applySpeed(host.player.playbackParameters.speed)
            speed()
        }
        sheet.show()
    }

    fun chapters() {
        val chapters = host.chapters
        val sheet = PlayerSheet(host, label("chapters", "Chapters"))
        val current = host.chapterAt(host.player.currentPosition)?.first
        chapters.forEachIndexed { index, chapter ->
            sheet.row(host.chapterName(chapter, index), formatTime(chapter.startMs), index == current) {
                host.player.seekTo(chapter.startMs)
                sheet.dismiss()
            }
        }
        sheet.show()
    }

    fun more() {
        val sheet = PlayerSheet(host, label("more", "More"))
        sheet.row(label("sleepTimer", "Sleep timer"), host.sleepNote() ?: label("off", "Off")) { sleep() }
        sheet.row(label("screenshot", "Screenshot")) { screenshot() }
        sheet.row(label("openElsewhere", "Open in another app")) {
            sheet.dismiss()
            host.openElsewhere()
        }
        val software = host.prefs.decoder == "software"
        sheet.row(
            label("decoder", "Decoder"),
            if (software) label("decoderSoftware", "Software first") else label("decoderAuto", "Hardware, then software"),
        ) {
            host.setDecoder(if (software) "auto" else "software")
            more()
        }
        sheet.row(label("statistics", "Statistics"), selected = host.statsShown) {
            host.toggleStats()
            more()
        }
        val intro = host.introMs()
        sheet.row(
            label("introLength", "Intro length for this title"),
            if (intro <= 0) label("off", "Off") else label("seconds", "{n} s", (intro / 1000).toString()),
        ) { introLength() }
        sheet.row(label("pip", "Picture-in-picture")) {
            sheet.dismiss()
            host.enterPip()
        }
        sheet.show()
    }

    private fun sleep() {
        val sheet = PlayerSheet(host, label("sleepTimer", "Sleep timer"))
        sheet.row(label("off", "Off"), selected = !host.sleepSet()) {
            host.setSleep(null, false)
            sheet.dismiss()
        }
        SLEEP_MINUTES.forEach { minutes ->
            sheet.row(label("sleepMinutes", "{n} min", minutes.toString())) {
                host.setSleep(minutes * 60_000L, false)
                sheet.dismiss()
            }
        }
        sheet.row(label("sleepEndOfEpisode", "End of this episode"), selected = host.sleepAtEnd) {
            host.setSleep(null, true)
            sheet.dismiss()
        }
        sheet.show()
    }

    private fun screenshot() {
        val sheet = PlayerSheet(host, label("screenshot", "Screenshot"))
        sheet.row(label("withSubtitles", "With subtitles")) {
            sheet.dismiss()
            host.tools.screenshot(true)
        }
        sheet.row(label("withoutSubtitles", "Without subtitles")) {
            sheet.dismiss()
            host.tools.screenshot(false)
        }
        sheet.show()
    }

    /** After a screenshot is saved: share it. */
    fun saved(share: () -> Unit) {
        val sheet = PlayerSheet(host, label("screenshotSaved", "Saved to Pictures/Raygun"))
        sheet.row(label("share", "Share")) {
            sheet.dismiss()
            share()
        }
        sheet.show()
    }

    fun introLength() {
        val sheet = PlayerSheet(host, label("introLength", "Intro length for this title"))
        val own = host.state.introMs
        val fallback = (host.prefs.introMs / 1000).toString()
        sheet.row(label("introDefault", "Use the default ({n} s)", fallback), selected = own == null) {
            host.setIntroLength(null)
            sheet.dismiss()
        }
        INTRO_SECONDS.forEach { seconds ->
            val name = if (seconds == 0) label("off", "Off") else label("seconds", "{n} s", seconds.toString())
            sheet.row(name, selected = own == seconds * 1000L) {
                host.setIntroLength(seconds * 1000L)
                sheet.dismiss()
            }
        }
        sheet.show()
    }
}

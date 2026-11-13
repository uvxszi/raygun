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
import kotlin.concurrent.thread

/** Subtitle edges, as in Settings; the colours offered are in `PlayerCaptions.kt`. */
private val EDGES = listOf("none", "outline", "shadow", "raised", "depressed")
private val SLEEP_MINUTES = listOf(15, 30, 45, 60, 90)
/** These match `SUBTITLE_MODES`, `VideoResolution::ALL` and `SEEK_SECONDS` in Settings. */
private val SUBTITLE_MODES = listOf("foreign", "always", "never")
private val RESOLUTIONS = listOf("highest", "2160p", "1440p", "1080p", "720p", "480p", "360p")
private val SEEK_SECONDS = listOf(5, 10, 15, 30, 60)
private val INTRO_SECONDS =listOf(0, 30, 60, 75, 85, 90, 100, 110, 120, 150, 180)

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
        // Long series (hundreds of episodes) make their rows as they scroll by, from the current one.
        val current = (0 until episodes.length()).firstOrNull { episodes.optJSONObject(it)?.optBoolean("current") == true } ?: 0
        val watched = label("watched", "Watched")
        val downloaded = label("downloaded", "Downloaded")
        sheet.list(episodes.length(), current, { index ->
            val episode = episodes.optJSONObject(index) ?: JSONObject()
            val marks = listOfNotNull(
                watched.takeIf { episode.optBoolean("watched") },
                downloaded.takeIf { episode.optBoolean("downloaded") },
            ).joinToString(" · ")
            Triple(episode.optString("label"), marks.ifBlank { null }, episode.optBoolean("current"))
        }) { index ->
            sheet.dismiss()
            if (episodes.optJSONObject(index)?.optBoolean("current") != true) {
                host.request("episode", JSONObject().put("index", index))
            }
        }
        sheet.show()
    }

    /** The Quality sheet: resolutions from this stream or other sources ([PlayerQuality]). */
    fun quality() = host.quality.open()

    /** Sources: clean names with their options, or the raw list ([PlayerSourcesSheet]). */
    fun sources() = host.sourcesSheet.open()

    fun audio() {
        val tracks = host.tracks.list(C.TRACK_TYPE_AUDIO)
        val sheet = PlayerSheet(host, label("audio", "Audio"))
        if (tracks.isEmpty()) sheet.row(label("off", "Off"), onClick = null)
        // Two "English" tracks read "English" and "English 2", their channels under them.
        val shown = distinctTracks(
            tracks.mapIndexed { index, track -> host.tracks.facts(track, index + 1, host.tracks.audioDetail(track)) },
            ::numbered,
        )
        tracks.forEachIndexed { index, track ->
            sheet.row(shown[index].name, shown[index].detail, host.tracks.chosen(track)) {
                host.tracks.select(C.TRACK_TYPE_AUDIO, track)
                host.state.audio = track.choice()
                host.remember(JSONObject().put("audio", track.choice().toJson()))
                sheet.dismiss()
            }
        }
        host.effects.audioRows(sheet) { audio() }
        sheet.show()
    }

    /** The subtitle sheet's tab: the subtitles (0) or the second subtitles (1). */
    private var subtitleTab = 0

    /** The subtitle sheet last drawn, followed as the video's tracks change. */
    private var subtitleSheet: PlayerSheet? = null

    /** The subtitle sheet, on its first tab. */
    fun subtitles() {
        subtitleTab = 0
        drawSubtitles()
    }

    /**
     * Media3 settled on the tracks asked for (or loaded new ones): an open subtitle sheet shows
     * what plays now, in place.
     */
    fun tracksChanged() {
        if (host.sheetShown(subtitleSheet)) drawSubtitles()
    }

    /**
     * The subtitle sheet, drawn again after a choice or as AI subtitles move on, on the tab it
     * was on: **Subtitles** (what adds them, the delay, the style, the track to show) and
     * **Second subtitles** (a second track at the top of the picture). A choice shows ticked at
     * once ([PlayerTracks.chosen]), before Media3 reports it.
     */
    private fun drawSubtitles() {
        val sheet = PlayerSheet(host, label("subtitles", "Subtitles"))
        subtitleSheet = sheet
        sheet.chips(
            listOf(label("subtitles", "Subtitles"), label("secondSubtitles", "Second subtitles")),
            subtitleTab,
        ) { tab ->
            subtitleTab = tab
            drawSubtitles()
        }
        if (subtitleTab == 1) secondSubtitles(sheet) else mainSubtitles(sheet)
        watch(sheet, ::drawSubtitles)
        sheet.show()
    }

    /**
     * A track to list, filed under its language so the viewer's languages come first, with the
     * name that tells it apart and what goes under it ([distinctTracks]).
     */
    private class Choice(val language: String?, val shown: ShownTrack, val selected: Boolean, val pick: () -> Unit)

    /** Adds `list` as rows, the viewer's languages first, each language's tracks together. */
    private fun PlayerSheet.choices(list: List<Choice>) {
        byLanguages(list, host.languages.listed) { it.language }.forEach { choice ->
            row(choice.shown.name, choice.shown.detail, choice.selected) { choice.pick() }
        }
    }

    /** "English 2": a name numbered among others alike ([distinctTracks]). */
    private fun numbered(name: String, number: Int): String =
        host.label("numberedTrack", "{n} {of}", name, number.toString())

    /**
     * A subtitle track's name, with where it comes from under it: a file's addon or own name
     * ("Subtitle file" when neither is known), or the video itself.
     */
    private fun textFacts(track: TrackRef, ordinal: Int): TrackFacts {
        val sidecar = host.sidecarOf(track.format)
        val source = if (sidecar == null) label("inTheVideo", "In the video") else sidecar.source
        val facts = host.tracks.facts(track, ordinal, source)
        return if (facts.detail == null && sidecar != null) facts.copy(detail = label("subtitleFile", "Subtitle file")) else facts
    }

    /**
     * The video's subtitle tracks and files as the list shows them, in its order (the viewer's
     * languages first), each named apart from the others: "English", "English (SDH)",
     * "English 2". The saved AI subtitles are left out while the ones being made show.
     */
    private fun namedTextTracks(): List<Pair<TrackRef, ShownTrack>> {
        val live = host.liveTracks
        val tracks = host.tracks.list(C.TRACK_TYPE_TEXT).mapIndexedNotNull { index, track ->
            if (live.hides(track)) null else track to textFacts(track, index + 1)
        }
        val listed = byLanguages(tracks, host.languages.listed) { it.first.language }
        val shown = distinctTracks(listed.map { it.second }, ::numbered)
        return listed.mapIndexed { index, (track, _) -> track to shown[index] }
    }

    /** AI subtitles and translations once they have lines or are being made, named apart. */
    private fun namedLiveTracks(detail: (LiveTrack) -> String?): List<Pair<LiveTrack, ShownTrack>> {
        val tracks = host.liveTracks.tracks.values.filter { it.shown }
        val shown = distinctTracks(tracks.map { TrackFacts(liveName(it), detail = detail(it)) }, ::numbered)
        return tracks.mapIndexed { index, track -> track to shown[index] }
    }

    private fun mainSubtitles(sheet: PlayerSheet) {
        val live = host.liveTracks
        // What adds subtitles comes first, as buttons of its own (each opens its sheet): the list
        // below only holds subtitles to show.
        sheet.subtitleTools(host, listOf(
            SubtitleTool(R.drawable.raygun_ic_note_add, label("addSubtitleFile", "Add a subtitle file"), null) {
                sheet.dismiss()
                host.pickSubtitleFile()
            },
            SubtitleTool(R.drawable.raygun_ic_auto_awesome, label("aiTranscribe", "Transcribe with AI"), transcribeDetail()) {
                transcribe()
            },
            SubtitleTool(R.drawable.raygun_ic_translate, label("translate", "Translate subtitles"), translateDetail()) {
                translate()
            },
        ))
        // Delay and style: what is changed most while watching. The style is a card of its own,
        // with a sample, so it doesn't read as one more track.
        sheet.delayField(host, host.state.subtitleDelayMs) { delay ->
            host.setSubtitleDelay(delay)
            drawSubtitles()
        }
        sheet.styleCard(host, host.prefs.style) { style() }

        sheet.header(label("subtitles", "Subtitles"))
        val off = host.tracks.textOff() && live.primary == null
        sheet.row(label("off", "Off"), selected = off) {
            live.show(null)
            host.tracks.select(C.TRACK_TYPE_TEXT, null)
            host.state.subtitleOff = true
            host.remember(JSONObject().put("subtitle", JSONObject().put("off", true)))
            drawSubtitles()
        }
        // The video's tracks and files, then AI subtitles and translations once there are lines
        // or they're being made; the viewer's languages first, each language's tracks together.
        val own = namedTextTracks().map { (track, shown) ->
            Choice(track.language, shown, !off && live.primary == null && host.tracks.chosen(track)) {
                live.show(null)
                host.tracks.select(C.TRACK_TYPE_TEXT, track)
                host.state.subtitleOff = false
                host.state.subtitle = track.choice()
                host.remember(JSONObject().put("subtitle", track.choice().toJson()))
                drawSubtitles()
            }
        }
        val made = namedLiveTracks(::liveDetail).map { (track, shown) ->
            Choice(track.language, shown, live.primary == track.id) {
                live.show(track.id)
                drawSubtitles()
            }
        }
        sheet.choices(own + made)
    }

    /** The second subtitles: a subtitle file or a track being made, shown at the top. */
    private fun secondSubtitles(sheet: PlayerSheet) {
        val live = host.liveTracks
        val second = host.second.current
        val secondLive = host.second.liveId
        sheet.row(label("off", "Off"), selected = second == null && secondLive == null) {
            host.second.clear()
            host.remember(JSONObject().put("secondarySubtitle", JSONObject.NULL))
            drawSubtitles()
        }
        val files = host.sidecars.mapIndexedNotNull { index, sidecar ->
            if (live.isAiFile(sidecar) && live.aiListed) return@mapIndexedNotNull null
            val facts = trackFacts(
                sidecar.label,
                sidecar.language,
                languageName(sidecar.language),
                emptyList(),
                { host.label(it.key, it.fallback) },
                sidecar.source,
            ) ?: TrackFacts(label("track", "Track {n}", (index + 1).toString()))
            Triple(sidecar.language ?: sidecar.label, sidecar, facts)
        }.let { byLanguages(it, host.languages.listed) { file -> file.first } }
        val fileNames = distinctTracks(files.map { it.third }, ::numbered)
        val seconds = files.mapIndexed { index, (language, sidecar, _) ->
            Choice(language, fileNames[index], second == sidecar) {
                host.loadSecond(sidecar)
                host.remember(JSONObject().put("secondarySubtitle", sidecar.choice().toJson()))
                drawSubtitles()
            }
        } + namedLiveTracks { null }.map { (track, shown) ->
            Choice(track.language, shown, secondLive == track.id) {
                host.second.showLive(track.id)
                drawSubtitles()
            }
        }
        if (seconds.isEmpty()) {
            // Only subtitle files (the source's, one added here) and tracks being made can show
            // second: the video's own are read by the player one at a time.
            sheet.row(label("secondNone", "No subtitle files for this video"), onClick = null)
        }
        sheet.choices(seconds)
    }

    // ---- AI subtitles: transcribing and translating as the video plays ----

    /** The sheet showing AI subtitles' progress, rebuilt as Rust's updates come. */
    private var aiSheet: PlayerSheet? = null
    private var aiRebuild: (() -> Unit)? = null

    private fun watch(sheet: PlayerSheet, rebuild: () -> Unit) {
        aiSheet = sheet
        aiRebuild = rebuild
    }

    /** Rust said something new about AI subtitles: an open sheet that shows them follows. */
    fun refreshAi() {
        val sheet = aiSheet ?: return
        if (host.sheetShown(sheet)) aiRebuild?.invoke()
    }

    /** "English (translated)", or the AI subtitles' name. */
    private fun liveName(track: LiveTrack): String = if (track.kind == "ai") {
        label("aiTrack", "AI subtitles")
    } else {
        label("translatedTrack", "{n} (translated)", languageName(track.language) ?: track.language.orEmpty())
    }

    /** How far making a live track got: "Transcribing… 2:30 ahead", "Translated to the end". */
    private fun liveDetail(track: LiveTrack): String? {
        val status = track.status
        status.error?.let { return it.lineSequence().firstOrNull() }
        status.downloading?.let { return label("translateDownloading", "Downloading the models… {n}%", it.toString()) }
        // A live stream's tracks are timed by its own clock.
        val ahead = status.aheadOf(host.liveStream.clockMs())
        if (track.kind == "ai") {
            return when {
                status.preparing -> label("aiPreparing", "Getting the sound…")
                status.running -> label("aiAhead", "Transcribing… {n} ahead", formatTime(ahead ?: 0L))
                status.complete -> label("aiComplete", "Transcribed to the end")
                else -> null
            }
        }
        return when {
            status.running && ahead != null -> label("translateAhead", "Translating… {n} ahead", formatTime(ahead))
            status.running -> label("translating", "Translating…")
            status.complete -> label("translateComplete", "Translated to the end")
            else -> host.countLabel(
                "translateDone",
                "{n} of {of} line",
                "{n} of {of} lines",
                status.done.toString(),
                status.total,
            )
        }
    }

    private fun transcribeDetail(): String? {
        host.liveTracks.tracks[LiveSubtitles.AI_TRACK]?.let { track -> liveDetail(track)?.let { return it } }
        val transcription = host.liveTracks.transcription ?: return null
        transcription.number("downloading")?.let {
            return label("aiDownloading", "Downloading the speech model… {n}%", it.toString())
        }
        return if (transcription.optBoolean("ready")) null else label("aiNotSetUp", "Not set up yet")
    }

    private fun translateDetail(): String? {
        val running = host.liveTracks.tracks.values.firstOrNull { it.kind == "translation" && it.status.running }
        if (running != null) return liveDetail(running)
        return label("translateInto", "Into {n}", languageName(translateLanguage()) ?: translateLanguage())
    }

    /** Transcribe with AI: set up (the speech model downloads here), start, stop, progress. */
    fun transcribe() {
        val live = host.liveTracks
        val sheet = PlayerSheet(host, label("aiTranscribe", "Transcribe with AI"))
        val transcription = live.transcription
        val track = live.tracks[LiveSubtitles.AI_TRACK]
        when {
            transcription == null -> sheet.row(label("aiChecking", "Checking how to transcribe…"), onClick = null)
            !transcription.optBoolean("ready") -> {
                sheet.row(transcription.text("problem") ?: label("aiNotSetUp", "Not set up yet"), onClick = null)
                val downloading = transcription.number("downloading")
                val model = transcription.optJSONObject("model")
                if (downloading != null) {
                    sheet.row(label("aiDownloading", "Downloading the speech model… {n}%", downloading.toString()), onClick = null)
                    // Stops it at once; what came so far is deleted and the sheet offers it again.
                    sheet.row(label("aiDownloadCancel", "Cancel the download")) {
                        live.cancelTranscriptionSetUp()
                        transcribe()
                    }
                } else if (model != null) {
                    sheet.row(
                        label("aiDownload", "Download the speech model"),
                        formatSize(model.optLong("bytes")),
                    ) {
                        live.setUpTranscription()
                        transcribe()
                    }
                }
                transcription.text("error")?.let { sheet.row(it.lineSequence().first(), onClick = null) }
                aiSettingsRow(sheet)
            }
            else -> {
                track?.let { sheet.row(liveName(it), liveDetail(it), selected = live.primary == it.id) { live.show(it.id); transcribe() } }
                if (track?.status?.running == true) {
                    sheet.row(label("aiStop", "Stop transcribing")) { live.stop(track) }
                } else {
                    val more = (track?.cues?.isNotEmpty() == true)
                    sheet.row(label(if (more) "aiContinue" else "aiStart", if (more) "Keep transcribing from here" else "Transcribe from here")) {
                        val online = transcription.optString("engine") == "online"
                        if (online && !transcription.optBoolean("confirmed")) confirmOnline() else {
                            live.transcribe()
                            host.showHint(label("aiStarting", "Transcribing from here…"))
                            transcribe()
                        }
                    }
                }
                val engine = if (transcription.optString("engine") == "online") {
                    label("aiOnline", "Online · {n}", transcription.optString("onlineModel"))
                } else {
                    label("aiOnDevice", "On this device")
                }
                sheet.row(label("aiEngine", "Transcribed by"), engine, onClick = if (host.canPip) ({ openAiSettings(sheet) }) else null)
            }
        }
        watch(sheet, ::transcribe)
        sheet.show()
    }

    private fun aiSettingsRow(sheet: PlayerSheet) {
        if (host.canPip) {
            sheet.row(label("aiSettings", "Set up in Settings → AI")) { openAiSettings(sheet) }
        }
    }

    /** Settings → AI → Transcription in the app, while the video waits in its small window. */
    private fun openAiSettings(sheet: PlayerSheet) {
        sheet.dismiss()
        host.player.pause()
        host.request("openAiSettings")
        host.enterPip()
    }

    /** Online transcription costs money: asked once. */
    private fun confirmOnline() {
        val sheet = PlayerSheet(host, label("aiTranscribe", "Transcribe with AI"))
        sheet.row(
            label("aiOnlineCost", "This sends the video's sound to your transcription provider, which may charge for it."),
            onClick = null,
        )
        sheet.row(label("aiStart", "Transcribe from here")) {
            host.liveTracks.transcribe(confirmed = true)
            transcribe()
        }
        sheet.row(label("cancel", "Cancel")) { transcribe() }
        sheet.show()
    }

    /** The language translations go into: picked here, else the first subtitle language. */
    private fun translateLanguage(): String =
        host.liveTracks.language ?: host.languages.subtitles.firstOrNull() ?: Locale.getDefault().language

    /**
     * Translate subtitles: the main subtitles into a language, on this device or with a translator
     * from Settings → AI; translations made or being made, to show or stop.
     */
    fun translate() {
        val live = host.liveTracks
        val sheet = PlayerSheet(host, label("translate", "Translate subtitles"))
        live.tracks.values.filter { it.kind == "translation" }.forEach { track ->
            sheet.row(liveName(track), liveDetail(track), selected = live.primary == track.id) {
                live.show(track.id)
                translate()
            }
            if (track.status.running) sheet.row(label("translateStop", "Stop translating")) { live.stop(track) }
        }
        val translation = live.translation
        val source = sourceName()
        when {
            translation == null -> sheet.row(label("aiChecking", "Checking how to transcribe…"), onClick = null)
            source.second != null -> sheet.row(source.second!!, onClick = null)
            else -> {
                sheet.header(label("translateFrom", "From {n}", source.first.orEmpty()))
                val language = translateLanguage()
                // The viewer's own language first, then the rest in a steady order.
                val tags = (listOfNotNull(host.languages.subtitles.firstOrNull()) +
                    translation.optJSONArray("languages").strings() + language)
                    .distinctBy { it.lowercase(Locale.ROOT) }
                val picked = tags.indexOfFirst { it.equals(language, ignoreCase = true) }.coerceAtLeast(0)
                sheet.chips(tags.map { languageName(it) ?: it }, picked) { index ->
                    live.language = tags[index]
                    translate()
                }
                sheet.header(label("translateWith", "Translate with"))
                val device = translation.optJSONArray("device").strings().any { sameLanguage(it, language) }
                sheet.row(
                    label("translateDevice", "This device"),
                    if (device) label("translateDeviceHint", "Free, offline; models download once") else label("translateNoModel", "No model here for this language"),
                    onClick = if (device) ({ startTranslation("device") }) else null,
                )
                val providers = translation.optJSONArray("translators")
                for (index in 0 until (providers?.length() ?: 0)) {
                    val provider = providers?.optJSONObject(index) ?: continue
                    val supported = provider.optBoolean("supported")
                    sheet.row(
                        provider.optString("name"),
                        if (supported) null else label("translateNeedsKey", "Needs its key in Settings → AI"),
                        onClick = if (supported) ({ startTranslation(provider.optString("id")) }) else null,
                    )
                }
            }
        }
        watch(sheet, ::translate)
        sheet.show()
    }

    /** What the main subtitles are, to translate them; or why they can't be. */
    private fun sourceName(): Pair<String?, String?> {
        val live = host.liveTracks
        live.primary?.let { id ->
            val track = live.tracks[id] ?: return null to null
            return if (track.kind == "ai") {
                liveName(track) to null
            } else {
                null to label("translateFromOriginal", "Show the subtitles to translate first, not a translation")
            }
        }
        val selected = host.tracks.list(C.TRACK_TYPE_TEXT).firstOrNull { host.tracks.chosen(it) }
        if (host.tracks.textOff() || selected == null) {
            return null to label("translateNeedsTrack", "Turn on the subtitles to translate first")
        }
        if (mainSidecar(selected) == null) {
            return null to label(
                "translateNeedsText",
                "These subtitles are part of the video and can't be read as text: pick a subtitle file, or transcribe with AI",
            )
        }
        return shownName(selected) to null
    }

    /** A subtitle track's name as the list shows it ("English 2"). */
    private fun shownName(track: TrackRef): String =
        namedTextTracks().firstOrNull { (listed, _) -> listed.group == track.group && listed.index == track.index }
            ?.second?.name ?: host.tracks.name(track, 1)

    /** The subtitle file the selected track came from; null for the video's own tracks. */
    private fun mainSidecar(track: TrackRef): Sidecar? = host.sidecarOf(track.format)

    /** Reads the main subtitles (a file, off the main thread) and asks Rust to translate them. */
    private fun startTranslation(translator: String) {
        val live = host.liveTracks
        val language = translateLanguage()
        val primary = live.primary?.let { live.tracks[it] }
        val selected = host.tracks.list(C.TRACK_TYPE_TEXT).firstOrNull { host.tracks.chosen(it) }
        val sidecar = selected?.let { mainSidecar(it) }
        if (primary?.kind == "ai" || (primary == null && sidecar != null && live.isAiFile(sidecar))) {
            live.translate(JSONObject().put("kind", "ai"), language, translator)
            host.showHint(label("translating", "Translating…"))
            translate()
            return
        }
        if (sidecar == null) return
        host.showHint(label("translating", "Translating…"))
        val name = shownName(selected)
        thread(name = "raygun-translate") {
            val result = runCatching { SubtitleFiles.lines(SubtitleFiles.parse(SubtitleFiles.read(host, sidecar.uri), sidecar.mime)) }
            host.runOnUiThread {
                result.onSuccess { cues ->
                    val source = JSONObject()
                        .put("kind", "cues")
                        .put("label", name)
                        .put("language", sidecar.language ?: JSONObject.NULL)
                        .put("cues", cues)
                    live.translate(source, language, translator)
                }.onFailure {
                    host.showHint(label("secondFailed", "Couldn't load those subtitles") + "\n" + (it.message ?: it.javaClass.simpleName))
                }
                if (host.sheetShown(aiSheet)) translate()
            }
        }
    }

    /** This device's models for the translation asked for aren't here: download them first? */
    fun askDownload(bytes: Long) {
        val sheet = PlayerSheet(host, label("translate", "Translate subtitles"))
        sheet.row(label("translateDownloadAsk", "Translating here downloads {n} of models first, once.", formatSize(bytes)), onClick = null)
        sheet.row(label("translateDownloadStart", "Download and translate")) {
            host.liveTracks.translateWithDownload()
            translate()
        }
        sheet.row(label("cancel", "Cancel")) { sheet.dismiss() }
        sheet.show()
    }

    fun style() {
        val look = host.prefs.style
        val sheet = PlayerSheet(host, label("style", "Style"))
        // The system's caption families, each name in its own font; the lines change at once,
        // the second subtitles too.
        sheet.header(label("font", "Font"))
        sheet.fontChips(host, look.font) { font ->
            host.updateStyle(look.copy(font = font))
            style()
        }
        sheet.stepper(label("size", "Size"), "${look.size}%", listOf(
            "−" to { host.updateStyle(look.copy(size = (look.size - 10).coerceAtLeast(50))); style() },
            "+" to { host.updateStyle(look.copy(size = (look.size + 10).coerceAtMost(200))); style() },
        ))
        sheet.stepper(label("height", "Height"), "${look.position}%", listOf(
            "−" to { host.updateStyle(look.copy(position = (look.position - 2).coerceAtLeast(0))); style() },
            "+" to { host.updateStyle(look.copy(position = (look.position + 2).coerceAtMost(40))); style() },
        ))
        // The widest a line gets before it wraps, as a share of the picture's width.
        sheet.stepper(label("lineWidth", "Line width"), "${look.maxWidth}%", listOf(
            "−" to { host.updateStyle(look.copy(maxWidth = (look.maxWidth - 5).coerceAtLeast(40))); style() },
            "+" to { host.updateStyle(look.copy(maxWidth = (look.maxWidth + 5).coerceAtMost(100))); style() },
        ))
        // Each colour beside its name, and Custom for one of the viewer's own. A colour picked
        // here wins over the file's own colours, white too.
        val textColor = label("textColor", "Text colour")
        sheet.header(textColor)
        sheet.swatches(host, TEXT_SWATCHES, look.textColor, { color ->
            host.updateStyle(look.withText(color))
            style()
        }) {
            customColor(textColor, look.textColor, opacity = false) { base, color -> base.withText(color) }
        }
        sheet.header(label("edge", "Edge"))
        sheet.chips(EDGES.map { edgeName(it) }, EDGES.indexOf(look.edge)) { picked ->
            host.updateStyle(look.copy(edge = EDGES[picked]))
            style()
        }
        if (look.edge != "none") {
            val edgeColor = label("edgeColor", "Edge colour")
            sheet.header(edgeColor)
            sheet.swatches(host, EDGE_SWATCHES, look.edgeColor, { color ->
                host.updateStyle(look.copy(edgeColor = color))
                style()
            }) {
                customColor(edgeColor, look.edgeColor, opacity = false) { base, color -> base.copy(edgeColor = color) }
            }
        }
        val background = label("background", "Background")
        sheet.header(background)
        sheet.swatches(host, BACKGROUND_SWATCHES, look.backgroundColor, { color ->
            host.updateStyle(look.withBackground(color))
            style()
        }) {
            customColor(background, look.backgroundColor, opacity = true) { base, color -> base.withBackground(color) }
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

    /**
     * A colour of the viewer's own for one part of the style ([ColorPicker]): shown on the
     * subtitles as it changes and kept in Settings once picked; Done goes back to Style.
     */
    private fun customColor(title: String, current: Int, opacity: Boolean, change: (SubtitleLook, Int) -> SubtitleLook) {
        ColorPicker(host, title, current, opacity, { color, save ->
            val next = change(host.prefs.style, color)
            if (save) host.updateStyle(next) else host.previewStyle(next)
        }, ::style).show()
    }

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
            progressTintList = ColorStateList.valueOf(playerAccent)
            thumbTintList = ColorStateList.valueOf(playerAccent)
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
        // Quality has its own button now; a live stream's delay behind the edge is here.
        if (host.liveStream.isLive) {
            val held = host.liveStream.heldMs
            sheet.row(
                label("liveDelayTitle", "Live delay"),
                if (held > 0) label("seconds", "{n} s", (held / 1000).toString()) else label("liveAtEdge", "At the live edge"),
            ) { host.liveStream.delaySheet() }
        }
        sheet.row(label("playOnTv", "Play on TV")) {
            sheet.dismiss()
            host.cast.open()
        }
        sheet.row(label("loop", "Loop this video"), selected = host.looping) {
            host.toggleLoop()
            more()
        }
        sheet.row(label("effects", "Effects"), host.effects.summary()) { host.effects.open() }
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
        sheet.row(label("playerSettings", "Player settings")) { settings() }
        sheet.show()
    }

    /**
     * Settings → Video player's most used rows, changed without leaving the video; "More settings"
     * opens the rest in the app while the video waits paused in its small window.
     */
    fun settings() {
        val prefs = host.prefs
        val sheet = PlayerSheet(host, label("playerSettings", "Player settings"))

        sheet.header(label("subtitlesWhen", "Subtitles"))
        val mode = SUBTITLE_MODES.indexOf(host.currentSubtitleMode).coerceAtLeast(0)
        sheet.chips(SUBTITLE_MODES.map { modeName(it) }, mode) { picked ->
            host.setSubtitleMode(SUBTITLE_MODES[picked])
            settings()
        }
        sheet.styleCard(host, prefs.style) { style() }

        // Yours, the video's own, then the languages this video has.
        sheet.header(label("audioLanguage", "Audio language"))
        val spoken = host.tracks.list(C.TRACK_TYPE_AUDIO).mapNotNull { it.language }.distinct()
        val audio = listOf("", ORIGINAL_AUDIO) + (spoken + listOfNotNull(Languages.code(prefs.audioLanguage)))
            .filter { it.isNotBlank() && it != ORIGINAL_AUDIO }
            .distinct()
        val audioNames = audio.map { code ->
            when (code) {
                "" -> label("audioYours", "Your language")
                ORIGINAL_AUDIO -> label("audioOriginal", "Original")
                else -> languageName(code) ?: code
            }
        }
        val audioPicked = audio.indexOf(Languages.code(prefs.audioLanguage) ?: prefs.audioLanguage)
        sheet.chips(audioNames, audioPicked) { picked ->
            host.setAudioLanguage(audio[picked])
            settings()
        }

        sheet.header(label("resolution", "Resolution"))
        sheet.chips(
            RESOLUTIONS.map { if (it == "highest") label("resolutionHighest", "Highest") else it },
            RESOLUTIONS.indexOf(prefs.preferredResolution),
        ) { picked ->
            host.setResolution(RESOLUTIONS[picked])
            settings()
        }

        sheet.header(label("gestures", "Gestures"))
        sheet.row(label("brightnessSwipe", "Swipe for brightness"), selected = prefs.brightnessGesture) {
            prefs.brightnessGesture = !prefs.brightnessGesture
            host.savePrefs(JSONObject().put("brightnessGesture", prefs.brightnessGesture))
            settings()
        }
        sheet.row(label("volumeSwipe", "Swipe for volume"), selected = prefs.volumeGesture) {
            prefs.volumeGesture = !prefs.volumeGesture
            host.savePrefs(JSONObject().put("volumeGesture", prefs.volumeGesture))
            settings()
        }
        sheet.row(label("seekSwipe", "Swipe to seek"), selected = prefs.seekGesture) {
            prefs.seekGesture = !prefs.seekGesture
            host.savePrefs(JSONObject().put("seekGesture", prefs.seekGesture))
            settings()
        }

        sheet.header(label("seekStep", "Seek step"))
        val step = (host.seekStepMs / 1000).toInt()
        sheet.chips(SEEK_SECONDS.map { label("seconds", "{n} s", it.toString()) }, SEEK_SECONDS.indexOf(step)) { picked ->
            host.setSeekStep(SEEK_SECONDS[picked])
            settings()
        }

        if (host.canPip) {
            sheet.row(label("moreSettings", "More settings")) {
                sheet.dismiss()
                host.openAppSettings()
            }
        }
        sheet.show()
    }

    private fun modeName(mode: String) = when (mode) {
        "always" -> label("subtitlesAlways", "Always")
        "never" -> label("subtitlesNever", "Never")
        else -> label("subtitlesForeign", "Other languages")
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

/** "154 MB", for what a download takes. */
private fun formatSize(bytes: Long): String = when {
    bytes >= 1_000_000_000 -> String.format(Locale.ROOT, "%.1f GB", bytes / 1e9)
    bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
    else -> "${(bytes / 1_000).coerceAtLeast(1)} kB"
}

/** A JSON array of strings as a list (none when missing). */
private fun JSONArray?.strings(): List<String> =
    (0 until (this?.length() ?: 0)).mapNotNull { index -> this?.optString(index)?.takeIf { it.isNotBlank() } }

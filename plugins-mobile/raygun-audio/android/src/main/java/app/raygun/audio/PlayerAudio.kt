package app.raygun.audio

import android.net.Uri
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.FilteringMediaSource
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.MergingMediaSource
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

/** How long after a video loads the sound remembered for its title may still join it. */
private const val REMEMBERED_WINDOW_MS = 15_000L

/**
 * Another stream of the episode, offered for its sound (Rust's `audioSources` update,
 * `video_session::audio`): where it comes from, the languages its names give, and its address.
 */
internal data class AudioOffer(
    val id: String,
    val source: String,
    val languages: List<String>,
    val several: Boolean,
    val title: String,
    val sound: String?,
    val url: String,
    val headers: Map<String, String>,
    val format: String?,
) {
    companion object {
        fun list(items: JSONArray?): List<AudioOffer> {
            items ?: return emptyList()
            return (0 until items.length()).mapNotNull { index ->
                val item = items.optJSONObject(index) ?: return@mapNotNull null
                val url = item.text("url") ?: return@mapNotNull null
                val headers = mutableMapOf<String, String>()
                item.optJSONObject("headers")?.let { json -> json.keys().forEach { key -> headers[key] = json.optString(key) } }
                val languages = item.optJSONArray("languages")?.let { list ->
                    (0 until list.length()).mapNotNull { list.optString(it).takeIf { code -> code.isNotBlank() } }
                }.orEmpty()
                AudioOffer(
                    id = item.text("id") ?: url,
                    source = item.optString("source"),
                    languages = languages,
                    several = item.optBoolean("several"),
                    title = item.optString("title"),
                    sound = item.text("sound"),
                    url = url,
                    headers = headers,
                    format = item.text("format"),
                )
            }
        }
    }
}

/** The sound a title remembers from another source (`externalAudio`): its source and language. */
internal data class RememberedAudio(val source: String, val language: String?) {
    fun toJson(): JSONObject = JSONObject().put("source", source).put("language", language ?: JSONObject.NULL)

    companion object {
        fun from(json: JSONObject?): RememberedAudio? {
            val source = json?.text("source") ?: return null
            return RememberedAudio(source, json.text("language"))
        }

        fun of(offer: AudioOffer) = RememberedAudio(offer.source, offer.languages.firstOrNull())
    }
}

/**
 * The offer that brings back the sound a title remembers: the same source in the same language
 * (one that names none, when none was named).
 */
internal fun rememberedOffer(offers: List<AudioOffer>, remembered: RememberedAudio): AudioOffer? =
    offers.firstOrNull { offer ->
        offer.source == remembered.source && when (val language = remembered.language) {
            null -> offer.languages.isEmpty()
            else -> offer.languages.any { sameLanguage(it, language) }
        }
    }

/** An offer's name: the languages its names give ("Italian, English"), else `several`, else `unknown`. */
internal fun offerName(offer: AudioOffer, name: (String) -> String?, several: String, unknown: String): String {
    val languages = offer.languages.map { name(it) ?: it.uppercase(Locale.ROOT) }.distinct()
    return when {
        languages.isNotEmpty() -> languages.joinToString(", ")
        offer.several -> several
        else -> unknown
    }
}

/** What goes under an offer's name: its source, what its names say, its sound. */
internal fun offerDetail(offer: AudioOffer, extra: String? = null): String? =
    listOfNotNull(offer.source.ifBlank { null }, offer.title.ifBlank { null }, offer.sound, extra)
        .joinToString(" · ")
        .ifBlank { null }

/** The MIME type a stream's format names (HLS, DASH), else null for Media3 to tell. */
internal fun streamMimeType(format: String?): String? = when (format?.lowercase(Locale.ROOT)) {
    "hls", "m3u8" -> MimeTypes.APPLICATION_M3U8
    "dash", "mpd" -> MimeTypes.APPLICATION_MPD
    else -> null
}

/** Whether a merged track came from the sound played beside the video (the second source). */
internal fun fromSecondSource(trackGroupId: String): Boolean = trackGroupId.startsWith("1:")

/** Sound played beside the picture: another source's stream, or a file picked on the device. */
internal data class ExternalAudio(
    val uri: String,
    val headers: Map<String, String>,
    val format: String?,
    /** The offer it came from; null for a file. */
    val offer: AudioOffer?,
    /** A file's own name. */
    val name: String?,
)

/**
 * The audio sheet and the sound beside the picture. Another source's stream (one Rust offers,
 * the same episode) or a file from the device plays as the video's sound through Media3's
 * [MergingMediaSource]: the video from one source, only the sound from the other
 * ([FilteringMediaSource]), on one clock. The audio delay moves the sound against the picture
 * ([AudioDelaySource]), whichever it is; it's remembered for the title, and so is a source's
 * sound, taken again in the next episode when the same source offers the same language.
 * When the player fails with another source's sound in it, that sound goes and the video plays
 * on with its own ([dropAfterError]).
 */
@OptIn(UnstableApi::class)
internal class PlayerAudio(private val host: VideoPlayerActivity) {
    private fun label(key: String, fallback: String, value: String? = null) = host.label(key, fallback, value)

    /** The delay every source playing is read with, once it's wrapped. */
    val delay = AudioDelay()

    /** What the video's offers are for (its key), and the offers. */
    private var offersFor: String? = null
    private var offers: List<AudioOffer> = emptyList()

    /** The sound wanted beside the picture; the next source built takes it. */
    private var wanted: ExternalAudio? = null

    /** The sound in the source playing now (its tracks are the second source's). */
    var mergedIn: ExternalAudio? = null
        private set

    /** Its track is chosen once Media3 lists it. */
    private var pendingSelect = false

    /** The source playing is read with the delay. */
    private var delayed = false

    /** Offers that failed for this video. */
    private val failed = mutableSetOf<String>()

    /** The video these belong to, and when it loaded. */
    private var videoId = ""
    private var loadedAt = 0L
    private var rememberedTaken = false

    private var sheet: PlayerSheet? = null

    /** A video loads: another one starts without the last one's sound; the title's delay applies. */
    fun onLoad(id: String, state: TitleState) {
        if (id != videoId) {
            wanted = null
            pendingSelect = false
            rememberedTaken = false
            failed.clear()
            videoId = id
            loadedAt = SystemClock.elapsedRealtime()
        }
        delay.us = state.audioDelayMs.coerceIn(-MAX_AUDIO_DELAY_MS, MAX_AUDIO_DELAY_MS) * 1000
    }

    /**
     * The source to play: `main`, merged with the sound wanted, read with the delay when there's
     * one (or another sound; a delay set later then needs no rebuilding).
     */
    fun wrap(main: MediaSource): MediaSource {
        val audio = wanted
        mergedIn = audio
        val merged = if (audio != null) MergingMediaSource(main, soundOf(audio)) else main
        delayed = audio != null || delay.us != 0L
        return if (delayed) AudioDelaySource(merged, delay) else merged
    }

    /** Only the sound of `audio`'s stream: its picture and subtitles never stand in for the video's. */
    private fun soundOf(audio: ExternalAudio): MediaSource {
        val agent = audio.headers.entries.firstOrNull { it.key.equals("User-Agent", true) }?.value
            ?: host.sourceUserAgent
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent(agent)
            .setDefaultRequestProperties(audio.headers.filterKeys { !it.equals("User-Agent", true) })
            .setAllowCrossProtocolRedirects(true)
        val item = MediaItem.Builder()
            .setUri(audio.uri)
            .apply { streamMimeType(audio.format)?.let { setMimeType(it) } }
            .build()
        val source = DefaultMediaSourceFactory(DefaultDataSource.Factory(host, http)).createMediaSource(item)
        return FilteringMediaSource(source, C.TRACK_TYPE_AUDIO)
    }

    /** Whether `track` is the sound beside the picture. */
    fun isExternal(track: TrackRef): Boolean = mergedIn != null && fromSecondSource(track.group.mediaTrackGroup.id)

    /** Media3 listed the tracks: the sound just added plays. */
    fun onTracksChanged() {
        if (pendingSelect) {
            val track = host.tracks.list(C.TRACK_TYPE_AUDIO).firstOrNull { isExternal(it) }
            if (track != null) {
                pendingSelect = false
                host.tracks.select(C.TRACK_TYPE_AUDIO, track)
            }
        }
        refresh()
    }

    /**
     * The player failed with another sound in its source: that sound goes and the video plays
     * on from where it was, with its own. True when it did; false leaves the failure to the
     * player (it's the video's own).
     */
    fun dropAfterError(): Boolean {
        val audio = mergedIn ?: return false
        audio.offer?.let { failed += it.id }
        wanted = null
        pendingSelect = false
        host.showHint(label("audioFailed", "Couldn't play that sound"), 3_000)
        host.reloadKeepingPlace()
        return true
    }

    /** Rust's offers for a video (`audioSources`). */
    fun onUpdate(update: JSONObject) {
        offersFor = update.text("id")
        offers = AudioOffer.list(update.optJSONArray("items"))
        takeRemembered()
        refresh()
    }

    /** The offers for the video playing that haven't failed. */
    private fun offersHere(): List<AudioOffer> =
        if (offersFor == videoId) offers.filter { it.id !in failed } else emptyList()

    /**
     * The sound the title remembers from another source joins the video, once, while it has just
     * started: a source that answers later doesn't change the sound under the viewer.
     */
    private fun takeRemembered() {
        if (rememberedTaken || wanted != null || mergedIn != null) return
        if (SystemClock.elapsedRealtime() - loadedAt > REMEMBERED_WINDOW_MS) return
        val remembered = host.state.externalAudio ?: return
        val offer = rememberedOffer(offersHere(), remembered) ?: return
        rememberedTaken = true
        attach(offer, remember = false)
        host.showHint(label("audioFromSource", "Sound from {n}", offer.source))
    }

    /** Plays `offer`'s sound with the picture, from where the video is. */
    private fun attach(offer: AudioOffer, remember: Boolean = true) {
        wanted = ExternalAudio(offer.url, offer.headers, offer.format, offer, null)
        pendingSelect = true
        if (remember) {
            val choice = RememberedAudio.of(offer)
            host.state.externalAudio = choice
            host.remember(JSONObject().put("externalAudio", choice.toJson()))
        }
        host.reloadKeepingPlace()
    }

    /** A file picked on the device plays as the sound, for this video. */
    fun addFile(uri: Uri, name: String) {
        wanted = ExternalAudio(uri.toString(), emptyMap(), null, null, name)
        pendingSelect = true
        host.reloadKeepingPlace()
    }

    /**
     * Positive delays play the sound later. The source playing is read again from where it is
     * (a seek), or built again with the delay the first time one is set.
     */
    fun setDelay(ms: Long) {
        val value = ms.coerceIn(-MAX_AUDIO_DELAY_MS, MAX_AUDIO_DELAY_MS)
        if (value == host.state.audioDelayMs) return
        host.state.audioDelayMs = value
        delay.us = value * 1000
        host.remember(JSONObject().put("audioDelayMs", value))
        if (delayed) {
            host.player.seekTo(host.player.currentPosition)
        } else if (value != 0L) {
            host.reloadKeepingPlace()
        }
    }

    /** The video's own track: the other sound stays loaded, unheard, until the next video. */
    private fun chooseOwn(track: TrackRef) {
        host.tracks.select(C.TRACK_TYPE_AUDIO, track)
        host.state.audio = track.choice()
        val change = JSONObject().put("audio", track.choice().toJson())
        if (host.state.externalAudio != null) {
            host.state.externalAudio = null
            change.put("externalAudio", JSONObject.NULL)
        }
        wanted = null
        host.remember(change)
    }

    /** The sound beside the picture again, without loading it again. */
    private fun chooseExternal(track: TrackRef) {
        host.tracks.select(C.TRACK_TYPE_AUDIO, track)
        wanted = mergedIn
        mergedIn?.offer?.let { offer ->
            val choice = RememberedAudio.of(offer)
            host.state.externalAudio = choice
            host.remember(JSONObject().put("externalAudio", choice.toJson()))
        }
    }

    /** New tracks or offers: an open sheet shows them, where it was scrolled ([PlayerSheet.scrolled]). */
    private fun refresh() {
        if (host.sheetShown(sheet)) draw(opening = false)
    }

    fun open() = draw(opening = true)

    private fun numbered(name: String, number: Int): String =
        host.label("numberedTrack", "{n} {of}", name, number.toString())

    private fun nameOf(offer: AudioOffer): String = offerName(
        offer,
        { languageName(it) },
        label("audioSeveral", "Several languages"),
        label("audioUnnamed", "Sound"),
    )

    /**
     * The audio sheet: adding a file, the delay, the video's own tracks with the sound beside
     * it (named by where it comes from), then the other sources' sound for this episode. Opened,
     * it shows the track playing; drawn again after a change, it stays where it was scrolled.
     */
    private fun draw(opening: Boolean) {
        val sheet = PlayerSheet(host, label("audio", "Audio"))
        this.sheet = sheet
        sheet.subtitleTools(host, listOf(
            SubtitleTool(R.drawable.raygun_ic_note_add, label("addAudioFile", "Add an audio file"), null) {
                sheet.dismiss()
                host.pickAudioFile()
            },
        ))
        sheet.delayField(host, host.state.audioDelayMs, label("audioDelaySeconds", "Audio delay in seconds")) { ms ->
            setDelay(ms)
            draw(opening = false)
        }

        val tracks = host.tracks.list(C.TRACK_TYPE_AUDIO)
        val own = tracks.filter { !isExternal(it) }
        val beside = tracks.filter { isExternal(it) }
        sheet.header(label("audioTracks", "Audio tracks"))
        if (tracks.isEmpty()) sheet.row(label("off", "Off"), onClick = null)
        // Two "English" tracks read "English" and "English 2", their channels under them.
        val shown = distinctTracks(
            own.mapIndexed { index, track -> host.tracks.facts(track, index + 1, host.tracks.audioDetail(track)) },
            ::numbered,
        )
        own.forEachIndexed { index, track ->
            val row = sheet.row(shown[index].name, shown[index].detail, host.tracks.chosen(track)) {
                chooseOwn(track)
                sheet.dismiss()
            }
            if (opening && host.tracks.chosen(track)) sheet.reveal(row)
        }
        val audio = mergedIn
        beside.forEach { track ->
            val offer = audio?.offer
            val name = when {
                offer != null -> nameOf(offer).let { named ->
                    // An offer that named no language takes its track's, when the file says one.
                    if (offer.languages.isEmpty()) track.language?.let { languageName(it) } ?: named else named
                }
                else -> audio?.name ?: label("audioFile", "Audio file")
            }
            val detail = listOfNotNull(
                offer?.source?.let { label("audioFrom", "From {n}", it) } ?: label("audioFile", "Audio file"),
                host.tracks.audioDetail(track),
            ).joinToString(" · ")
            sheet.row(name, detail, host.tracks.chosen(track)) {
                chooseExternal(track)
                sheet.dismiss()
            }
        }

        val available = offersHere()
        if (host.session || available.isNotEmpty()) {
            sheet.header(label("audioOtherSources", "From other sources"))
            if (available.isEmpty()) {
                sheet.row(label("audioNoOtherSources", "No other source has this yet"), onClick = null)
            }
            available.forEach { offer ->
                val loaded = audio?.offer?.id == offer.id
                val track = if (loaded) beside.firstOrNull() else null
                val playing = track != null && host.tracks.chosen(track)
                val loading = loaded && track == null
                sheet.row(
                    nameOf(offer),
                    offerDetail(offer, label("audioLoading", "Loading…").takeIf { loading }),
                    selected = playing || loading,
                ) {
                    sheet.dismiss()
                    when {
                        track != null -> chooseExternal(track)
                        !loaded -> attach(offer)
                    }
                }
            }
        }
        sheet.show()
    }
}

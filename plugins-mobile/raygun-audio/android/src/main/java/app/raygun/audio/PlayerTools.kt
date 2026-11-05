package app.raygun.audio

import android.app.Activity
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Parcelable
import android.provider.MediaStore
import android.util.Rational
import android.view.PixelCopy
import android.view.SurfaceView
import androidx.annotation.RequiresApi
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.concurrent.thread

/** What another app said when it closed: where it stopped, and whether the video ended. */
internal data class ExternalResult(val positionMs: Long?, val durationMs: Long?, val finished: Boolean)

/**
 * The player's tools (M45): screenshots (Pixel Copy of the video, subtitles drawn over), handing a
 * video to another app (MX Player's, VLC's and mpv's extras, after Aniyomi's `ExternalIntents`),
 * and picture-in-picture's buttons.
 */
internal class PlayerTools(private val host: VideoPlayerActivity) {
    companion object {
        const val ACTION_PIP = "app.raygun.audio.PIP_CONTROL"
        const val EXTRA_CONTROL = "control"
        private const val PREVIOUS = 1
        private const val PLAY = 2
        private const val PAUSE = 3
        private const val NEXT = 4
        private const val REWIND = 5
        private const val FORWARD = 6
        private const val VLC = "org.videolan.vlc"
    }

    /** Picture-in-picture's buttons arrive here. */
    val pipReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val player = host.player
            when (intent.getIntExtra(EXTRA_CONTROL, 0)) {
                PREVIOUS -> host.request("previous")
                NEXT -> host.requestNext()
                PLAY -> player.play()
                PAUSE -> player.pause()
                REWIND -> host.seekBy(-host.seekStepMs)
                FORWARD -> host.seekBy(host.seekStepMs)
            }
            host.updatePip()
        }
    }

    @RequiresApi(Build.VERSION_CODES.O)
    fun pipParams(): PictureInPictureParams {
        val size = host.player.videoSize
        val ratio = if (size.width > 0 && size.height > 0) {
            Rational(size.width, size.height).let {
                // Android refuses ratios beyond 2.39:1.
                when {
                    it.toFloat() > 2.39f -> Rational(239, 100)
                    it.toFloat() < 0.42f -> Rational(100, 239)
                    else -> it
                }
            }
        } else {
            Rational(16, 9)
        }
        val playing = host.player.isPlaying
        val episodes = host.session && host.several
        val actions = listOf(
            if (episodes) {
                action(PREVIOUS, R.drawable.raygun_ic_previous, host.label("previous", "Previous episode"), host.hasPrevious)
            } else {
                action(REWIND, R.drawable.raygun_ic_rewind, "-${host.seekStepMs / 1000} s", true)
            },
            if (playing) {
                action(PAUSE, R.drawable.raygun_ic_pause, host.label("pause", "Pause"), true)
            } else {
                action(PLAY, R.drawable.raygun_ic_play, host.label("play", "Play"), true)
            },
            if (episodes) {
                action(NEXT, R.drawable.raygun_ic_next, host.label("next", "Next episode"), host.hasNext)
            } else {
                action(FORWARD, R.drawable.raygun_ic_forward, "+${host.seekStepMs / 1000} s", true)
            },
        )
        val builder = PictureInPictureParams.Builder()
            .setAspectRatio(ratio)
            .setActions(actions)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            builder.setAutoEnterEnabled(host.prefs.pipOnLeave && playing)
            builder.setSeamlessResizeEnabled(true)
        }
        return builder.build()
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun action(code: Int, icon: Int, title: String, enabled: Boolean): RemoteAction {
        val intent = Intent(ACTION_PIP).setPackage(host.packageName).putExtra(EXTRA_CONTROL, code)
        val pending = PendingIntent.getBroadcast(
            host,
            code,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return RemoteAction(Icon.createWithResource(host, icon), title, title, pending).apply {
            isEnabled = enabled
        }
    }

    /** Saves the picture (with the subtitles drawn on it, or not) to Pictures/Raygun. */
    fun screenshot(withSubtitles: Boolean) {
        val failed = { host.showHint(host.label("screenshotFailed", "Couldn't take a screenshot")) }
        val surface = host.playerView.videoSurfaceView as? SurfaceView
        if (surface == null || surface.width <= 0 || surface.height <= 0 || Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            failed()
            return
        }
        val bitmap = Bitmap.createBitmap(surface.width, surface.height, Bitmap.Config.ARGB_8888)
        PixelCopy.request(surface, bitmap, { result ->
            if (result != PixelCopy.SUCCESS) {
                failed()
                return@request
            }
            if (withSubtitles) {
                // The subtitles cover the screen; the picture may be smaller (fit) or larger (crop).
                val surfaceAt = IntArray(2).also { surface.getLocationOnScreen(it) }
                val subtitlesAt = IntArray(2).also { host.subtitleView.getLocationOnScreen(it) }
                val canvas = Canvas(bitmap)
                canvas.translate(
                    (subtitlesAt[0] - surfaceAt[0]).toFloat(),
                    (subtitlesAt[1] - surfaceAt[1]).toFloat(),
                )
                host.subtitleView.draw(canvas)
            }
            val name = screenshotName()
            thread(name = "raygun-screenshot") {
                val uri = runCatching { save(bitmap, name) }.getOrNull()
                host.runOnUiThread {
                    if (uri == null) failed() else host.sheets.saved { share(uri) }
                }
            }
        }, host.main)
    }

    private fun screenshotName(): String {
        val time = SimpleDateFormat("yyyy-MM-dd HH.mm.ss", Locale.ROOT).format(Date())
        val title = host.headingText().replace('\n', ' ').replace(Regex("[\\\\/:*?\"<>|]"), " ").trim().take(80)
        return "$title $time.png".trim()
    }

    @RequiresApi(Build.VERSION_CODES.Q)
    private fun save(bitmap: Bitmap, name: String): Uri? {
        val resolver = host.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Images.Media.DISPLAY_NAME, name)
            put(MediaStore.Images.Media.MIME_TYPE, "image/png")
            put(MediaStore.Images.Media.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/Raygun")
            put(MediaStore.Images.Media.IS_PENDING, 1)
        }
        val uri = resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values) ?: return null
        resolver.openOutputStream(uri)?.use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        values.clear()
        values.put(MediaStore.Images.Media.IS_PENDING, 0)
        resolver.update(uri, values, null, null)
        return uri
    }

    private fun share(uri: Uri) {
        val send = Intent(Intent.ACTION_SEND)
            .setType("image/png")
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        host.startActivity(Intent.createChooser(send, host.label("share", "Share")))
    }

    /**
     * The intent that plays `request` in another app: `packageName`'s, or any app's when blank.
     * Title, headers, subtitles and the start position go as MX Player, VLC and mpv read them.
     */
    fun externalIntent(request: JSONObject, packageName: String, startMs: Long): Intent {
        val url = request.optString("url")
        val format = request.optString("format")
        val type = when {
            packageName.isNotBlank() -> "video/*"
            format == "hls" || url.contains(".m3u8") -> "application/x-mpegURL"
            format == "dash" || url.contains(".mpd") -> "application/dash+xml"
            else -> "video/*"
        }
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(Uri.parse(url), type)
            .putExtra("title", host.headingText().replace('\n', ' '))
            .putExtra("return_result", true)
            .putExtra("secure_uri", true)
            .putExtra("from_start", false)
        // VLC reads the position as a long, the others as an int; both in milliseconds.
        if (packageName == VLC) intent.putExtra("position", startMs) else intent.putExtra("position", startMs.toInt())
        request.optJSONObject("headers")?.let { headers ->
            val pairs = mutableListOf<String>()
            val bundle = Bundle()
            headers.keys().forEach { key ->
                pairs += key
                pairs += headers.optString(key)
                bundle.putString(key, headers.optString(key))
            }
            if (pairs.isNotEmpty()) {
                intent.putExtra("headers", pairs.toTypedArray())
                intent.putExtra("android.media.intent.extra.HTTP_HEADERS", bundle)
            }
        }
        val subtitles = request.optJSONArray("subtitles")
        if (subtitles != null && subtitles.length() > 0) {
            val uris = mutableListOf<Uri>()
            val names = mutableListOf<String>()
            for (index in 0 until subtitles.length()) {
                val track = subtitles.optJSONObject(index) ?: continue
                val address = track.text("url") ?: continue
                uris += Uri.parse(address)
                names += track.text("label")
                    ?: languageName(track.text("lang"))
                    ?: track.text("lang")
                    ?: host.label("track", "Track {n}", (index + 1).toString())
            }
            if (uris.isNotEmpty()) {
                intent.putExtra("subs", uris.map { it as Parcelable }.toTypedArray())
                intent.putExtra("subs.name", names.toTypedArray())
                intent.putExtra("subs.enable", arrayOf<Parcelable>(uris.first()))
                intent.putExtra("subtitles_location", uris.first().toString())
            }
        }
        if (packageName.isNotBlank()) intent.setPackage(packageName)
        // A document another app opened with Raygun: the access it gave goes on to this player.
        if (url.startsWith("content://")) intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return intent
    }

    /** Reads where another app stopped (MX Player and mpv: `position`; VLC: `extra_position`). */
    @Suppress("DEPRECATION")
    fun externalResult(resultCode: Int, data: Intent?): ExternalResult? {
        if (resultCode != Activity.RESULT_OK) return null
        val extras = data?.extras ?: return null
        fun number(key: String): Long? = (extras.get(key) as? Number)?.toLong()?.takeIf { it > 0 }
        return ExternalResult(
            positionMs = number("extra_position") ?: number("position"),
            durationMs = number("extra_duration") ?: number("duration"),
            finished = extras.getString("end_by") == "playback_completion",
        )
    }
}

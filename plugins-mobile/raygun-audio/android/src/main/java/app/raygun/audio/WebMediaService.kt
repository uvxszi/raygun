package app.raygun.audio

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.drawable.Icon
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.Inet6Address
import java.net.InetAddress
import java.net.URL
import kotlin.concurrent.thread

/** What a website plays, as the notification and the media session show it. */
internal class WebMediaInfo(
    val title: String,
    val artist: String?,
    val album: String?,
    val artworkUrl: String?,
    val durationMs: Long?,
    val positionMs: Long?,
    val rate: Float,
    val playing: Boolean,
    /** The page's actions: `play`, `pause`, `previoustrack`, `nexttrack`, `seekto`. */
    val actions: Set<String>,
    /** When the position was read (`SystemClock.elapsedRealtime`). */
    val at: Long,
    /** The browser's, for the artwork. */
    val userAgent: String?,
)

/** The browser whose page plays: the notification's and the media session's controls go to it. */
internal interface WebMediaOwner {
    /** A `control` message for the page that plays (`play`, `pause`, `seekto` with `seekTime`…). */
    fun control(action: String, seekTime: Double?)

    /** The service stopped after a long pause: nothing outside the browser resumes the page now. */
    fun released()
}

/**
 * What the built-in browser plays, shared with [WebMediaService] in the browser's process (`:web`):
 * the browser says what plays and whether it's on screen; the service shows it, and sends the
 * notification's and the media session's controls back to the browser.
 */
internal object WebMedia {
    var now: WebMediaInfo? = null
        private set

    /** Whether the browser that plays is off screen: wake and Wi-Fi locks are held then while it plays. */
    var offScreen = false
        private set

    private var owner: WebMediaOwner? = null

    /** The running service, when there is one. */
    internal var service: WebMediaService? = null

    /**
     * `info` plays (or is paused) in `owner`'s page. The service starts when it plays and the
     * browser is on screen (Android refuses foreground services started from the background);
     * once it runs it follows every change.
     */
    fun show(context: Context, owner: WebMediaOwner, info: WebMediaInfo, offScreen: Boolean) {
        this.owner = owner
        now = info
        this.offScreen = offScreen
        val running = service
        if (running != null) {
            running.refresh()
        } else if (info.playing && !offScreen) {
            WebMediaService.start(context)
        }
    }

    /** The browser came on screen or left it. */
    fun screen(owner: WebMediaOwner, offScreen: Boolean) {
        if (this.owner !== owner) return
        this.offScreen = offScreen
        service?.refresh()
    }

    /** Nothing plays in `owner`'s pages any more (or the browser closed): the service stops. */
    fun clear(owner: WebMediaOwner) {
        if (this.owner !== owner) return
        this.owner = null
        now = null
        service?.end()
    }

    fun control(action: String, seekTime: Double? = null) {
        owner?.control(action, seekTime)
    }

    /** Paused too long: the service stops, and the browser hears it. */
    internal fun expired() {
        val was = owner
        owner = null
        now = null
        service?.end()
        was?.released()
    }
}

/**
 * Keeps a website playing with the browser off screen: a foreground service in the browser's
 * process with a media notification and a media session (lock screen, headset buttons, Android's
 * media controls), whose controls go to the page that plays ([WebMedia]). It holds a wake lock and
 * a Wi-Fi lock only while the page plays and the browser is off screen, stays up to 10 minutes
 * while it's paused, and stops when nothing plays or the browser closes.
 */
class WebMediaService : Service() {
    companion object {
        private const val CHANNEL = "raygun-web-media"

        /** Its own: Play on TV's is 7_302. */
        private const val NOTIFICATION_ID = 7_303
        private const val ACTION_PLAY = "app.raygun.audio.web.PLAY"
        private const val ACTION_PAUSE = "app.raygun.audio.web.PAUSE"
        private const val ACTION_PREVIOUS = "app.raygun.audio.web.PREVIOUS"
        private const val ACTION_NEXT = "app.raygun.audio.web.NEXT"

        /** How long it stays while the page is paused, ms. */
        private const val PAUSED_STAYS_MS = 10 * 60 * 1000L

        /** The wake lock's hold, renewed with each report while the page plays, ms. */
        private const val WAKE_MS = 10 * 60 * 1000L

        private const val LOCK_TAG = "raygun:website-playing"

        fun start(context: Context) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, WebMediaService::class.java))
            } catch (error: Exception) {
                Log.w("RaygunAudio", "a website can't play in the background", error)
            }
        }
    }

    private val main = Handler(Looper.getMainLooper())
    private lateinit var session: MediaSession
    private var foreground = false
    private var ended = false

    /** When the page was paused (`SystemClock.elapsedRealtime`); 0 while it plays. */
    private var pausedSince = 0L
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var hearsNoisy = false

    /** The artwork read, and the address it came from; the address being read. */
    private var art: Bitmap? = null
    private var artUrl: String? = null
    private var artLoading: String? = null

    /** What the session's metadata and the notification last said. */
    private var metadataShown = ""
    private var notificationShown = ""

    private val idle = Runnable { WebMedia.expired() }

    private val callback = object : MediaSession.Callback() {
        override fun onPlay() = WebMedia.control("play")

        override fun onPause() = WebMedia.control("pause")

        override fun onStop() = WebMedia.control("stop")

        override fun onSkipToNext() = WebMedia.control("nexttrack")

        override fun onSkipToPrevious() = WebMedia.control("previoustrack")

        override fun onSeekTo(pos: Long) = WebMedia.control("seekto", pos / 1000.0)
    }

    /** Headphones unplugged: the page pauses, as players do, rather than going on aloud. */
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) WebMedia.control("pause")
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        session = MediaSession(this, "RaygunWebsite").apply {
            setCallback(callback, main)
            setSessionActivity(openBrowser())
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) handleButtons(this)
            isActive = true
        }
        WebMedia.service = this
    }

    /** Before Android 8 a session handles buttons and transport controls only when it says so. */
    @Suppress("DEPRECATION")
    private fun handleButtons(session: MediaSession) {
        session.setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS or MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (ended) {
            stopSelf()
            return START_NOT_STICKY
        }
        // Started as a foreground service: in the foreground at once, whatever comes next.
        if (!foreground && !goForeground()) {
            end()
            return START_NOT_STICKY
        }
        when (intent?.action) {
            ACTION_PLAY -> WebMedia.control("play")
            ACTION_PAUSE -> WebMedia.control("pause")
            ACTION_PREVIOUS -> WebMedia.control("previoustrack")
            ACTION_NEXT -> WebMedia.control("nexttrack")
        }
        if (WebMedia.now == null) end() else refresh()
        return START_NOT_STICKY
    }

    private fun goForeground(): Boolean {
        NotificationManagerCompat.from(this).createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName("Websites playing")
                .setDescription("While a website plays in Raygun's browser")
                .setShowBadge(false)
                .build(),
        )
        return try {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification(WebMedia.now),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
            )
            foreground = true
            true
        } catch (error: Exception) {
            Log.w("RaygunAudio", "a website can't play in the background", error)
            false
        }
    }

    /**
     * Shows what plays now: the session, the notification, the locks, the wait while paused. Pages
     * report every few seconds while they play; metadata and the notification (with the artwork)
     * go to the system only when they change, the position each time.
     */
    internal fun refresh() {
        if (ended) return
        val info = WebMedia.now ?: return end()
        loadArt(info)
        val described = listOf(info.title, info.artist, info.album, info.durationMs, info.artworkUrl, shownArt(info) != null)
            .joinToString("\u0001")
        if (described != metadataShown) {
            metadataShown = described
            session.setMetadata(metadata(info))
        }
        session.setPlaybackState(playbackState(info))
        val noticed = "$described\u0001${info.playing}\u0001${info.actions.sorted()}"
        if (foreground && noticed != notificationShown) {
            notificationShown = noticed
            post(notification(info))
        }
        holdLocks(info.playing && WebMedia.offScreen)
        hearNoisy(info.playing)
        if (info.playing) {
            pausedSince = 0L
            main.removeCallbacks(idle)
        } else if (pausedSince == 0L) {
            pausedSince = SystemClock.elapsedRealtime()
            main.postDelayed(idle, PAUSED_STAYS_MS)
        }
    }

    /** Stops: the notification goes, and media buttons go back to other apps. */
    internal fun end() {
        if (ended) return
        letGo()
        if (foreground) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            foreground = false
        }
        stopSelf()
    }

    private fun letGo() {
        ended = true
        if (WebMedia.service === this) WebMedia.service = null
        main.removeCallbacksAndMessages(null)
        holdLocks(false)
        hearNoisy(false)
        session.isActive = false
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        end()
    }

    override fun onDestroy() {
        if (!ended) letGo()
        session.release()
        super.onDestroy()
    }

    // ---- The notification and the session ----

    @Suppress("DEPRECATION")
    private fun builder(): Notification.Builder =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) Notification.Builder(this, CHANNEL) else Notification.Builder(this)

    /** The media notification: the page's previous and next when it has them, play or pause. */
    private fun notification(info: WebMediaInfo?): Notification {
        val builder = builder()
        val offered = info?.actions.orEmpty()
        val actions = ArrayList<Notification.Action>()
        if ("previoustrack" in offered) actions.add(action(R.drawable.raygun_ic_previous, "Previous", ACTION_PREVIOUS))
        actions.add(
            if (info?.playing == true) {
                action(R.drawable.raygun_ic_pause, "Pause", ACTION_PAUSE)
            } else {
                action(R.drawable.raygun_ic_play, "Play", ACTION_PLAY)
            },
        )
        if ("nexttrack" in offered) actions.add(action(R.drawable.raygun_ic_next, "Next", ACTION_NEXT))
        actions.forEach { builder.addAction(it) }
        builder
            .setSmallIcon(R.drawable.ic_stat_raygun)
            // Android shows a notification without a title or text as an empty one.
            .setContentTitle(firstText(info?.title) ?: "A website")
            .setContentText(firstText(info?.artist, info?.album) ?: "Playing in Raygun's browser")
            .setContentIntent(openBrowser())
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setCategory(Notification.CATEGORY_TRANSPORT)
            .setShowWhen(false)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setStyle(
                Notification.MediaStyle()
                    .setMediaSession(session.sessionToken)
                    .setShowActionsInCompactView(*IntArray(actions.size) { it }),
            )
        shownArt(info)?.let { builder.setLargeIcon(it) }
        return builder.build()
    }

    private fun action(icon: Int, title: String, command: String): Notification.Action {
        val intent = Intent(this, WebMediaService::class.java).setAction(command)
        val pending = PendingIntent.getService(
            this,
            command.hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Action.Builder(Icon.createWithResource(this, icon), title, pending).build()
    }

    /** The browser as it was (its tabs, its page): never a new one, which would read no request. */
    private fun openBrowser(): PendingIntent = PendingIntent.getActivity(
        this,
        0,
        Intent(this, WebsiteActivity::class.java).addFlags(
            Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT or Intent.FLAG_ACTIVITY_SINGLE_TOP,
        ),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun post(notification: Notification) {
        try {
            NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, notification)
        } catch (error: SecurityException) {
            Log.w("RaygunAudio", "notifications aren't allowed", error)
        }
    }

    private fun metadata(info: WebMediaInfo): MediaMetadata {
        val builder = MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE, info.title)
        info.artist?.let { builder.putString(MediaMetadata.METADATA_KEY_ARTIST, it) }
        info.album?.let { builder.putString(MediaMetadata.METADATA_KEY_ALBUM, it) }
        info.durationMs?.let { builder.putLong(MediaMetadata.METADATA_KEY_DURATION, it) }
        shownArt(info)?.let { builder.putBitmap(MediaMetadata.METADATA_KEY_ART, it) }
        return builder.build()
    }

    private fun playbackState(info: WebMediaInfo): PlaybackState {
        var actions = PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
            PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_STOP
        if ("previoustrack" in info.actions) actions = actions or PlaybackState.ACTION_SKIP_TO_PREVIOUS
        if ("nexttrack" in info.actions) actions = actions or PlaybackState.ACTION_SKIP_TO_NEXT
        if ("seekto" in info.actions) actions = actions or PlaybackState.ACTION_SEEK_TO
        return PlaybackState.Builder()
            .setActions(actions)
            .setState(
                if (info.playing) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED,
                info.positionMs ?: PlaybackState.PLAYBACK_POSITION_UNKNOWN,
                if (info.playing) info.rate else 0f,
                info.at,
            )
            .build()
    }

    // ---- Locks and headphones ----

    @Suppress("DEPRECATION")
    private fun holdLocks(hold: Boolean) {
        if (!hold) {
            wakeLock?.takeIf { it.isHeld }?.let { runCatching { it.release() } }
            wifiLock?.takeIf { it.isHeld }?.let { runCatching { it.release() } }
            return
        }
        val wake = wakeLock ?: (getSystemService(Context.POWER_SERVICE) as? PowerManager)
            ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, LOCK_TAG)
            ?.apply { setReferenceCounted(false) }
            ?.also { wakeLock = it }
        // Renewed with each report while the page plays: one that stops saying anything lets go.
        runCatching { wake?.acquire(WAKE_MS) }
        val wifi = wifiLock ?: (applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
            ?.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, LOCK_TAG)
            ?.apply { setReferenceCounted(false) }
            ?.also { wifiLock = it }
        if (wifi != null && !wifi.isHeld) runCatching { wifi.acquire() }
    }

    private fun hearNoisy(on: Boolean) {
        if (on == hearsNoisy) return
        hearsNoisy = on
        if (on) {
            ContextCompat.registerReceiver(
                this,
                noisy,
                IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        } else {
            runCatching { unregisterReceiver(noisy) }
        }
    }

    // ---- Artwork ----

    private fun shownArt(info: WebMediaInfo?): Bitmap? {
        val address = info?.artworkUrl ?: return null
        return art?.takeIf { artUrl == address }
    }

    /** Reads the artwork off the main thread once per address; the notification follows. */
    private fun loadArt(info: WebMediaInfo) {
        val address = info.artworkUrl ?: return
        if (address == artUrl || address == artLoading) return
        artLoading = address
        val userAgent = info.userAgent
        thread(name = "raygun-web-artwork", isDaemon = true) {
            val bitmap = fetchArtwork(address, userAgent)
            main.post {
                if (ended || artLoading != address) return@post
                artLoading = null
                artUrl = address
                art = bitmap
                if (bitmap != null && WebMedia.now?.artworkUrl == address) refresh()
            }
        }
    }
}

/** Artwork read at most, bytes, and its longest side once scaled, px. */
private const val MAX_ART_BYTES = 2 * 1024 * 1024
private const val ART_PX = 512

/**
 * The picture at `address`: at most 2 MB, scaled to 512 px at most; null when it can't be had.
 * Redirects are followed by hand (a few), each address checked like the first.
 */
private fun fetchArtwork(address: String, userAgent: String?): Bitmap? {
    var next = address
    repeat(4) {
        if (!publicAddress(next)) return null
        var connection: HttpURLConnection? = null
        try {
            val opened = URL(next).openConnection() as HttpURLConnection
            connection = opened
            opened.connectTimeout = 10_000
            opened.readTimeout = 10_000
            opened.instanceFollowRedirects = false
            userAgent?.let { opened.setRequestProperty("User-Agent", it) }
            val code = opened.responseCode
            if (code in 300..399) {
                next = opened.getHeaderField("Location")?.let { URL(URL(next), it).toString() } ?: return null
                return@repeat
            }
            if (code !in 200..299 || opened.contentLengthLong > MAX_ART_BYTES) return null
            return opened.inputStream.use { readAtMost(it, MAX_ART_BYTES) }?.let { scaled(it, ART_PX) }
        } catch (error: Exception) {
            Log.d("RaygunAudio", "a website's artwork didn't load: ${error.message}")
            return null
        } finally {
            connection?.disconnect()
        }
    }
    return null
}

/**
 * An http(s) address of a host on the internet: not this device (`localhost`), nor its network
 * (`.local` names, private and link-local numbers).
 */
private fun publicAddress(address: String): Boolean {
    val uri = Uri.parse(address)
    val scheme = uri.scheme?.lowercase()
    if (scheme != "https" && scheme != "http") return false
    val host = uri.host?.lowercase()?.trimEnd('.')?.removePrefix("[")?.removeSuffix("]") ?: return false
    if (host.isEmpty() || host == "localhost" || host.endsWith(".localhost") || host.endsWith(".local") ||
        host.endsWith(".internal")
    ) {
        return false
    }
    // Only numbers are checked: a name isn't looked up here.
    if (!host.all { it.isDigit() || it == '.' } && !host.contains(':')) return true
    val ip = runCatching { InetAddress.getByName(host) }.getOrNull() ?: return false
    val uniqueLocal = ip is Inet6Address && (ip.address[0].toInt() and 0xFE) == 0xFC
    return !(ip.isLoopbackAddress || ip.isSiteLocalAddress || ip.isLinkLocalAddress || ip.isAnyLocalAddress ||
        ip.isMulticastAddress || uniqueLocal)
}

private fun readAtMost(input: InputStream, max: Int): ByteArray? {
    val out = ByteArrayOutputStream()
    val buffer = ByteArray(16 * 1024)
    while (true) {
        val read = input.read(buffer)
        if (read < 0) break
        if (out.size() + read > max) return null
        out.write(buffer, 0, read)
    }
    return out.toByteArray()
}

/** Decodes `bytes` no larger than needed, then scales to `max` px on the longer side. */
private fun scaled(bytes: ByteArray, max: Int): Bitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    var sample = 1
    while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= max) sample *= 2
    val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
        ?: return null
    val longest = maxOf(bitmap.width, bitmap.height)
    if (longest <= max) return bitmap
    val scale = max.toFloat() / longest
    return Bitmap.createScaledBitmap(
        bitmap,
        (bitmap.width * scale).toInt().coerceAtLeast(1),
        (bitmap.height * scale).toInt().coerceAtLeast(1),
        true,
    )
}

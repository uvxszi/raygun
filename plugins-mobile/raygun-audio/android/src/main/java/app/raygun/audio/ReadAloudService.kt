package app.raygun.audio

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
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
import org.json.JSONObject

/**
 * Reading aloud's notification and media session while a book is read aloud: the previous and
 * next paragraph, Pause or Resume, and Stop, on the notification, the lock screen, headset
 * buttons and the system's media controls. It runs in the foreground so reading goes on with the
 * screen off (a wake lock is held while it reads). Rust shows and updates it (`ttsNotice`) and
 * ends it (`ttsNoticeEnd`); its controls go back to Rust as `{"control"}` reports.
 */
class ReadAloudService : Service() {
    companion object {
        private const val CHANNEL = "raygun-read-aloud"

        /** Its own: the website player's is 7_303. */
        private const val NOTIFICATION_ID = 7_304
        private const val ACTION_PAUSE = "app.raygun.audio.read.PAUSE"
        private const val ACTION_RESUME = "app.raygun.audio.read.RESUME"
        private const val ACTION_PREVIOUS = "app.raygun.audio.read.PREVIOUS"
        private const val ACTION_NEXT = "app.raygun.audio.read.NEXT"
        private const val ACTION_STOP = "app.raygun.audio.read.STOP"

        /** The wake lock's hold, renewed with each update while reading. */
        private const val WAKE_MS = 30 * 60 * 1000L

        /** What the notification shows, set before the service hears of it. */
        @Volatile
        private var title = ""

        @Volatile
        private var text = ""

        @Volatile
        private var playing = false

        @Volatile
        private var running: ReadAloudService? = null

        /** Shows or updates the notification: [title] the book's, [text] the chapter's. */
        fun show(context: Context, title: String, text: String, playing: Boolean) {
            this.title = title
            this.text = text
            this.playing = playing
            val service = running
            if (service != null) {
                Handler(Looper.getMainLooper()).post { service.refresh() }
                return
            }
            try {
                ContextCompat.startForegroundService(context, Intent(context, ReadAloudService::class.java))
            } catch (error: Exception) {
                // Android refuses foreground services started from the background: reading goes
                // on while Raygun is on screen.
                Log.w("RaygunAudio", "reading aloud can't show its notification", error)
            }
        }

        fun end(context: Context) {
            playing = false
            val service = running
            if (service != null) {
                Handler(Looper.getMainLooper()).post { service.finish() }
            } else {
                context.stopService(Intent(context, ReadAloudService::class.java))
            }
        }

        private fun control(name: String) {
            NativeBridge.tts(JSONObject().put("control", name).toString())
        }
    }

    private lateinit var session: MediaSession
    private var foreground = false
    private var ended = false
    private var wakeLock: PowerManager.WakeLock? = null
    private var hearsNoisy = false

    private val callback = object : MediaSession.Callback() {
        override fun onPlay() = control("resume")

        override fun onPause() = control("pause")

        override fun onStop() = control("stop")

        override fun onSkipToNext() = control("next")

        override fun onSkipToPrevious() = control("previous")
    }

    /** Headphones unplugged: reading pauses, as players do, rather than going on aloud. */
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) control("pause")
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        session = MediaSession(this, "RaygunReadAloud").apply {
            setCallback(callback, Handler(Looper.getMainLooper()))
            openReader()?.let { setSessionActivity(it) }
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) handleButtons(this)
            isActive = true
        }
        running = this
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
            finish()
            return START_NOT_STICKY
        }
        when (intent?.action) {
            ACTION_PAUSE -> control("pause")
            ACTION_RESUME -> control("resume")
            ACTION_PREVIOUS -> control("previous")
            ACTION_NEXT -> control("next")
            ACTION_STOP -> control("stop")
        }
        refresh()
        return START_NOT_STICKY
    }

    private fun goForeground(): Boolean {
        NotificationManagerCompat.from(this).createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName("Reading aloud")
                .setDescription("While a book is read aloud")
                .setShowBadge(false)
                .build(),
        )
        return try {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification(),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
            )
            foreground = true
            true
        } catch (error: Exception) {
            Log.w("RaygunAudio", "reading aloud can't run in the foreground", error)
            false
        }
    }

    /** Shows what's read now: the session, the notification, the wake lock. */
    internal fun refresh() {
        if (ended) return
        session.setMetadata(
            MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, firstText(text, title) ?: "Reading aloud")
                .putString(MediaMetadata.METADATA_KEY_ARTIST, firstText(title) ?: "Raygun")
                .build(),
        )
        session.setPlaybackState(
            PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or PlaybackState.ACTION_PLAY_PAUSE or
                        PlaybackState.ACTION_STOP or PlaybackState.ACTION_SKIP_TO_NEXT or
                        PlaybackState.ACTION_SKIP_TO_PREVIOUS,
                )
                .setState(
                    if (playing) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED,
                    PlaybackState.PLAYBACK_POSITION_UNKNOWN,
                    if (playing) 1f else 0f,
                    SystemClock.elapsedRealtime(),
                )
                .build(),
        )
        if (foreground) {
            try {
                NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, notification())
            } catch (error: SecurityException) {
                Log.w("RaygunAudio", "notifications aren't allowed", error)
            }
        }
        holdWake(playing)
        hearNoisy(playing)
    }

    /** Stops: the notification goes, and media buttons go back to other apps. */
    internal fun finish() {
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
        if (running === this) running = null
        holdWake(false)
        hearNoisy(false)
        session.isActive = false
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        finish()
    }

    override fun onDestroy() {
        if (!ended) letGo()
        session.release()
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    private fun builder(): Notification.Builder =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) Notification.Builder(this, CHANNEL) else Notification.Builder(this)

    /** Previous paragraph, Pause or Resume, Next paragraph, Stop. */
    private fun notification(): Notification {
        val builder = builder()
        val actions = listOf(
            action(R.drawable.raygun_ic_previous, "Previous paragraph", ACTION_PREVIOUS),
            if (playing) {
                action(R.drawable.raygun_ic_pause, "Pause", ACTION_PAUSE)
            } else {
                action(R.drawable.raygun_ic_play, "Resume", ACTION_RESUME)
            },
            action(R.drawable.raygun_ic_next, "Next paragraph", ACTION_NEXT),
            action(R.drawable.raygun_ic_close, "Stop", ACTION_STOP),
        )
        actions.forEach { builder.addAction(it) }
        builder
            .setSmallIcon(R.drawable.ic_stat_raygun)
            // Android shows a notification without a title or text as an empty one.
            .setContentTitle(firstText(title) ?: "Reading aloud")
            .setContentText(firstText(text) ?: if (playing) "Reading aloud" else "Paused")
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setCategory(Notification.CATEGORY_TRANSPORT)
            .setShowWhen(false)
            .setOnlyAlertOnce(true)
            .setOngoing(playing)
            .setDeleteIntent(pending(ACTION_STOP))
            .setStyle(
                Notification.MediaStyle()
                    .setMediaSession(session.sessionToken)
                    .setShowActionsInCompactView(0, 1, 2),
            )
        openReader()?.let { builder.setContentIntent(it) }
        return builder.build()
    }

    private fun pending(command: String): PendingIntent = PendingIntent.getService(
        this,
        command.hashCode(),
        Intent(this, ReadAloudService::class.java).setAction(command),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun action(icon: Int, title: String, command: String): Notification.Action =
        Notification.Action.Builder(Icon.createWithResource(this, icon), title, pending(command)).build()

    /** Raygun as it was (the reader open): never a new copy of it. */
    private fun openReader(): PendingIntent? {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return null
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED)
        return PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    private fun holdWake(hold: Boolean) {
        if (!hold) {
            wakeLock?.takeIf { it.isHeld }?.let { runCatching { it.release() } }
            return
        }
        val wake = wakeLock ?: (getSystemService(Context.POWER_SERVICE) as? PowerManager)
            ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "raygun:reading-aloud")
            ?.apply { setReferenceCounted(false) }
            ?.also { wakeLock = it }
        runCatching { wake?.acquire(WAKE_MS) }
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
}

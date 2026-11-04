package app.raygun.audio

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject

/**
 * Downloads in the notification shade (Rust works out what they say, `raygun-app/src/notifications`):
 * one notification for a lone download, a summary with one per download when several run, and
 * a notice when the batch ends. They have their own channel, quiet and low. While anything
 * downloads, [RaygunDownloadService] keeps Raygun running in the background with the summary as
 * its notification.
 *
 * Android drops a package's notifications when they change faster than a few a second, so
 * changes beyond [MAX_POSTS] a second wait for the next round; Rust sends at most one plan a second.
 */
object DownloadNotices {
    const val CHANNEL = "raygun.downloads"
    private const val GROUP = "raygun.downloads"
    const val SUMMARY_ID = 0x52590001
    private const val FINISHED_ID = 0x52590002
    private const val FIRST_ITEM_ID = 0x52590100
    private const val MAX_POSTS = 4
    const val ACTION = "app.raygun.audio.DOWNLOAD_ACTION"

    private val main = Handler(Looper.getMainLooper())
    private val ids = mutableMapOf<String, Int>()
    private var nextId = FIRST_ITEM_ID
    /** What each posted notification shows, to post only what changed. */
    private val posted = mutableMapOf<Int, String>()
    private var plan: JSONObject? = null
    private val postTimes = ArrayDeque<Long>()
    private var retryScheduled = false

    /** The summary notification, for the service to stand in the foreground with. */
    @Volatile
    var summary: Notification? = null
        private set

    @Volatile
    var foreground = false
        private set

    /** Takes Rust's plan and the batch's end, if one ended. Main thread. */
    fun show(context: Context, next: JSONObject, finished: JSONObject?) {
        ensureChannel(context)
        plan = next
        finished?.let { postFinished(context, it) }
        apply(context.applicationContext)
    }

    private fun apply(context: Context) {
        val current = plan ?: return
        val manager = NotificationManagerCompat.from(context)
        val summaryJson = current.optJSONObject("summary")
        val items = current.optJSONArray("items") ?: JSONArray()
        val wanted = mutableMapOf<Int, Pair<String, () -> Notification>>()
        if (summaryJson != null) {
            val grouped = items.length() > 0
            wanted[SUMMARY_ID] = summaryJson.toString() + grouped to { build(context, summaryJson, grouped, summary = true) }
        }
        for (index in 0 until items.length()) {
            val item = items.getJSONObject(index)
            val id = ids.getOrPut(item.optString("key")) { nextId++ }
            wanted[id] = item.toString() to { build(context, item, grouped = true, summary = false) }
        }
        // Gone ones go at once: removing isn't limited.
        for (id in posted.keys.toList()) {
            if (id !in wanted) {
                if (id != SUMMARY_ID || !foreground) manager.cancel(id)
                posted.remove(id)
            }
        }
        ids.entries.removeAll { it.value !in wanted }
        val wantsForeground = current.optBoolean("foreground", false) && summaryJson != null
        var deferred = false
        // The summary first: it's the foreground notification.
        for ((id, entry) in wanted.toSortedMap(compareBy { if (it == SUMMARY_ID) 0 else 1 })) {
            val (fingerprint, make) = entry
            if (posted[id] == fingerprint) continue
            if (!mayPost()) {
                deferred = true
                continue
            }
            val notification = make()
            if (id == SUMMARY_ID) summary = notification
            notify(context, manager, id, notification)
            posted[id] = fingerprint
        }
        if (summaryJson == null) summary = null
        setForeground(context, wantsForeground)
        if (deferred && !retryScheduled) {
            retryScheduled = true
            main.postDelayed({
                retryScheduled = false
                apply(context)
            }, 1000)
        }
    }

    private fun mayPost(): Boolean {
        val now = SystemClock.elapsedRealtime()
        while (postTimes.isNotEmpty() && now - postTimes.first() > 1000) postTimes.removeFirst()
        if (postTimes.size >= MAX_POSTS) return false
        postTimes.addLast(now)
        return true
    }

    private fun notify(context: Context, manager: NotificationManagerCompat, id: Int, notification: Notification) {
        if (!manager.areNotificationsEnabled()) return
        try {
            manager.notify(id, notification)
        } catch (error: SecurityException) {
            Log.w("RaygunAudio", "notifications aren't allowed", error)
        }
    }

    private fun setForeground(context: Context, on: Boolean) {
        if (on == foreground) {
            if (on) RaygunDownloadService.refresh()
            return
        }
        foreground = on
        if (on) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, RaygunDownloadService::class.java))
            } catch (error: Exception) {
                // Android 12+ refuses from the background; the notifications still show.
                Log.w("RaygunAudio", "downloads couldn't run in the foreground", error)
                foreground = false
            }
        } else if (!RaygunDownloadService.finish()) {
            // No service stood with it (it couldn't start). A paused or waiting queue keeps its
            // notification, now one that can be swiped away; else it goes.
            summary?.let { notify(context, NotificationManagerCompat.from(context), SUMMARY_ID, it) }
                ?: NotificationManagerCompat.from(context).cancel(SUMMARY_ID)
        }
    }

    private fun postFinished(context: Context, finished: JSONObject) {
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.media3_notification_small_icon)
            .setContentTitle(finished.optString("title"))
            .setContentText(finished.optString("text"))
            .setStyle(NotificationCompat.BigTextStyle().bigText(finished.optString("text")))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setCategory(if (finished.optBoolean("failed")) NotificationCompat.CATEGORY_ERROR else NotificationCompat.CATEGORY_STATUS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
        openRaygunIntent(context, finished.optString("route", "/downloads"))?.let(builder::setContentIntent)
        addActions(context, builder, finished.optJSONArray("actions"), finished.optString("route", "/downloads"))
        notify(context, NotificationManagerCompat.from(context), FINISHED_ID, builder.build())
    }

    private fun build(context: Context, shown: JSONObject, grouped: Boolean, summary: Boolean): Notification {
        val route = shown.optString("route", "/downloads")
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.media3_notification_small_icon)
            .setContentTitle(shown.optString("title"))
            .setContentText(shown.optString("text"))
            .setOngoing(shown.optBoolean("ongoing", true))
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setShowWhen(false)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
        if (!shown.isNull("info")) builder.setSubText(shown.optString("info"))
        if (shown.optBoolean("ongoing", true)) {
            if (shown.isNull("progress")) {
                builder.setProgress(0, 0, true)
            } else {
                builder.setProgress(1000, (shown.optDouble("progress", 0.0) * 1000).toInt(), false)
            }
        }
        if (grouped) {
            builder.setGroup(GROUP)
            if (summary) builder.setGroupSummary(true)
        }
        openRaygunIntent(context, route)?.let(builder::setContentIntent)
        addActions(context, builder, shown.optJSONArray("actions"), route)
        return builder.build()
    }

    private fun addActions(context: Context, builder: NotificationCompat.Builder, actions: JSONArray?, route: String) {
        if (actions == null) return
        for (index in 0 until actions.length()) {
            val action = actions.getJSONObject(index)
            val id = action.optString("id")
            val keys = action.optJSONArray("keys") ?: JSONArray()
            val intent = Intent(context, DownloadActionReceiver::class.java)
                .setAction(ACTION)
                .putExtra("action", id)
                .putExtra("keys", keys.toString())
            // A retry from the batch's notice also opens where the downloads are.
            val request = (id + keys.toString()).hashCode()
            val pending = if (id == "retry") {
                val launch = context.packageManager.getLaunchIntentForPackage(context.packageName)
                    ?.putExtra(EXTRA_OPEN_ROUTE, route)
                    ?.putExtra(EXTRA_ACTION, JSONObject().put("action", id).put("keys", keys).toString())
                    ?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                launch?.let {
                    PendingIntent.getActivity(context, request, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
                }
            } else {
                PendingIntent.getBroadcast(context, request, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            }
            pending?.let { builder.addAction(0, action.optString("label"), it) }
        }
    }

    private fun ensureChannel(context: Context) {
        NotificationManagerCompat.from(context).createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName("Downloads")
                .setDescription("Download progress and finished downloads")
                .setShowBadge(false)
                .build(),
        )
    }

    /** A notification button pressed while the app's process might not have Rust loaded yet. */
    const val EXTRA_ACTION = "app.raygun.audio.DOWNLOAD_ACTION_JSON"
}

/** Buttons on download notifications: to Rust. */
class DownloadActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != DownloadNotices.ACTION) return
        val keys = runCatching { JSONArray(intent.getStringExtra("keys") ?: "[]") }.getOrDefault(JSONArray())
        NativeBridge.notificationAction(
            JSONObject().put("action", intent.getStringExtra("action").orEmpty()).put("keys", keys).toString(),
        )
    }
}

/**
 * Keeps Raygun running while it downloads (a `dataSync` foreground service), showing the
 * downloads' summary. Android 15 limits such services to six hours a day; past that they stop and
 * the notifications carry on without it.
 */
class RaygunDownloadService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        running = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = DownloadNotices.summary ?: placeholder()
        try {
            ServiceCompat.startForeground(
                this,
                DownloadNotices.SUMMARY_ID,
                notification,
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0,
            )
        } catch (error: Exception) {
            Log.w("RaygunAudio", "downloads couldn't run in the foreground", error)
            stopSelf()
            return START_NOT_STICKY
        }
        if (!DownloadNotices.foreground) finishNow()
        return START_NOT_STICKY
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        finishNow()
    }

    private fun placeholder(): Notification =
        NotificationCompat.Builder(this, DownloadNotices.CHANNEL)
            .setSmallIcon(R.drawable.media3_notification_small_icon)
            .setContentTitle("Downloading")
            .setOngoing(true)
            .setSilent(true)
            .setProgress(0, 0, true)
            .build()

    /**
     * Stops standing in the foreground. With nothing left to show, the notification goes with the
     * service (`REMOVE`): Android lets go of a foreground notification a moment after
     * `stopForeground`, and ignores an app's cancel until then, which left the last "Downloading"
     * on screen for good. A summary that still shows (a paused or waiting queue) stays (`DETACH`)
     * and is posted again as it is now: the service may have stood with an older one, or with
     * the placeholder when it started after the downloads had already ended.
     */
    private fun finishNow() {
        val keep = DownloadNotices.summary
        ServiceCompat.stopForeground(
            this,
            if (keep != null) ServiceCompat.STOP_FOREGROUND_DETACH else ServiceCompat.STOP_FOREGROUND_REMOVE,
        )
        stopSelf()
        if (keep != null) {
            NotificationManagerCompat.from(this).let {
                if (it.areNotificationsEnabled()) runCatching { it.notify(DownloadNotices.SUMMARY_ID, keep) }
            }
        }
    }

    override fun onDestroy() {
        if (running === this) running = null
        super.onDestroy()
    }

    companion object {
        @Volatile
        private var running: RaygunDownloadService? = null

        /** The summary changed: the foreground notification shows it. */
        fun refresh() {
            val service = running ?: return
            val notification = DownloadNotices.summary ?: return
            NotificationManagerCompat.from(service).let {
                if (it.areNotificationsEnabled()) runCatching { it.notify(DownloadNotices.SUMMARY_ID, notification) }
            }
        }

        /** Stops the service if it runs; false when none did. */
        fun finish(): Boolean {
            val service = running ?: return false
            service.finishNow()
            return true
        }
    }
}

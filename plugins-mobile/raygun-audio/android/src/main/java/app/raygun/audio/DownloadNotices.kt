package app.raygun.audio

import android.app.Notification
import android.app.NotificationManager
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
 *
 * Nothing is left behind: every plan also cancels this channel's notifications it doesn't name
 * (ones a killed process posted, or a cancel Android ignored right after `stopForeground`), and
 * all but the foreground one time out unless Rust's plan, sent again every minute, renews them.
 */
object DownloadNotices {
    const val CHANNEL = "raygun.downloads"
    private const val GROUP = "raygun.downloads"
    const val SUMMARY_ID = 0x52590001
    private const val FINISHED_ID = 0x52590002
    private const val FIRST_ITEM_ID = 0x52590100
    private const val MAX_POSTS = 4
    /** Notifications not renewed for this long go by themselves (Raygun was killed), ms. */
    private const val TIMEOUT_MS = 10 * 60 * 1000L
    /** One still showing the same is posted again after this long, renewing its timeout, ms. */
    private const val RENEW_MS = 2 * 60 * 1000L
    private const val LAST_ITEM_ID = FIRST_ITEM_ID + 0xFFFF
    const val ACTION = "app.raygun.audio.DOWNLOAD_ACTION"

    private val main = Handler(Looper.getMainLooper())
    private val ids = mutableMapOf<String, Int>()
    private var nextId = FIRST_ITEM_ID
    /** What each posted notification shows, to post only what changed, and when it was posted. */
    private val posted = mutableMapOf<Int, Pair<String, Long>>()
    /** Rust's last plan; none before this process got one. */
    @Volatile
    var plan: JSONObject? = null
        private set
    private val postTimes = ArrayDeque<Long>()
    private var retryScheduled = false

    /** The summary notification, for the service to stand in the foreground with. */
    @Volatile
    var summary: Notification? = null
        private set

    @Volatile
    var foreground = false
        private set

    /**
     * Android refused the foreground service (Android 15's daily limit for `dataSync`, or a start
     * from the background): not asked again until the downloads end.
     */
    @Volatile
    private var refused = false

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
        val wantsForeground = current.optBoolean("foreground", false) && summaryJson != null
        val wanted = mutableMapOf<Int, Pair<String, () -> Notification>>()
        if (summaryJson != null) {
            val grouped = items.length() > 0
            // The foreground one never times out: it goes with the service.
            wanted[SUMMARY_ID] = summaryJson.toString() + grouped + wantsForeground to {
                build(context, summaryJson, grouped, summary = true, expires = !wantsForeground)
            }
        }
        for (index in 0 until items.length()) {
            val item = items.getJSONObject(index)
            val id = ids.getOrPut(item.optString("key")) { nextId++ }
            wanted[id] = item.toString() to { build(context, item, grouped = true, summary = false, expires = true) }
        }
        ids.entries.removeAll { it.value !in wanted }
        // Gone ones go at once: removing isn't limited. The summary standing with the service goes
        // with it (setForeground below).
        if (summaryJson == null) summary = null
        for (id in posted.keys.toList()) {
            if (id !in wanted) {
                if (id != SUMMARY_ID || !foreground) manager.cancel(id)
                posted.remove(id)
            }
        }
        cancelStrays(context, wanted.keys)
        var deferred = false
        val now = SystemClock.elapsedRealtime()
        // The summary first: it's the foreground notification. (Ordered by id too: a comparator
        // that called every item equal kept only one of them.)
        for ((id, entry) in wanted.toSortedMap(compareBy<Int>({ if (it == SUMMARY_ID) 0 else 1 }, { it }))) {
            val (fingerprint, make) = entry
            val last = posted[id]
            if (last != null && last.first == fingerprint && now - last.second < RENEW_MS) continue
            if (!mayPost()) {
                // The service stands with the summary as it is now even before it's posted,
                // never with its placeholder.
                if (id == SUMMARY_ID) summary = make()
                deferred = true
                continue
            }
            val notification = make()
            if (id == SUMMARY_ID) summary = notification
            notify(context, manager, id, notification)
            posted[id] = fingerprint to now
        }
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

    /**
     * Cancels this channel's notifications that the plan doesn't name: ones posted by a process
     * that was killed (this one never knew them), and cancels Android ignored.
     */
    private fun cancelStrays(context: Context, wanted: Set<Int>) {
        val system = context.getSystemService(NotificationManager::class.java) ?: return
        val active = runCatching { system.activeNotifications }.getOrNull() ?: return
        val manager = NotificationManagerCompat.from(context)
        for (shown in active) {
            val id = shown.id
            val ours = id == SUMMARY_ID || id in FIRST_ITEM_ID..LAST_ITEM_ID
            if (!ours || id in wanted || shown.tag != null) continue
            // The service's own notification goes with the service.
            if (id == SUMMARY_ID && foreground && RaygunDownloadService.isRunning) continue
            manager.cancel(id)
        }
    }

    private fun setForeground(context: Context, on: Boolean) {
        if (!on) refused = false
        if (on == foreground || (on && refused)) return
        foreground = on
        if (on) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, RaygunDownloadService::class.java))
            } catch (error: Exception) {
                // Android 12+ refuses from the background; the notifications still show.
                Log.w("RaygunAudio", "downloads couldn't run in the foreground", error)
                foreground = false
                refused = true
            }
        } else if (!RaygunDownloadService.finish()) {
            // No service stood with it (it couldn't start). A paused or waiting queue keeps its
            // notification, now one that can be swiped away; else it goes.
            summary?.let { notify(context, NotificationManagerCompat.from(context), SUMMARY_ID, it) }
                ?: NotificationManagerCompat.from(context).cancel(SUMMARY_ID)
        }
    }

    /** The service stopped standing in the foreground on its own (refused, or out of time). */
    fun serviceEnded(context: Context) {
        main.post {
            if (!foreground) return@post
            foreground = false
            refused = true
            // The summary is posted again as an ordinary notification, or goes.
            val manager = NotificationManagerCompat.from(context)
            summary?.let { notify(context, manager, SUMMARY_ID, it) } ?: manager.cancel(SUMMARY_ID)
        }
    }

    /**
     * Clears every download notification but the batch's notice: a button was pressed in a
     * process that never got a plan (Raygun was killed and nothing downloads in this one).
     */
    fun clearStale(context: Context) {
        main.post {
            if (plan != null) return@post
            val manager = NotificationManagerCompat.from(context)
            manager.cancel(SUMMARY_ID)
            cancelStrays(context, emptySet())
        }
    }

    /** Cancels the summary in a moment, if it's still not wanted: Android may ignore a cancel
     * that comes right after `stopForeground`. */
    fun cancelSummaryLater(context: Context) {
        main.postDelayed({
            if (summary == null && !foreground) NotificationManagerCompat.from(context).cancel(SUMMARY_ID)
        }, 1500)
    }

    private fun postFinished(context: Context, finished: JSONObject) {
        val title = firstText(finished.optString("title")) ?: "Downloads finished"
        val text = firstText(finished.optString("text")) ?: "See them in Downloads"
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setCategory(if (finished.optBoolean("failed")) NotificationCompat.CATEGORY_ERROR else NotificationCompat.CATEGORY_STATUS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
        openRaygunIntent(context, finished.optString("route", "/downloads"))?.let(builder::setContentIntent)
        finished.optJSONObject("open")?.let { file -> openFileIntent(context, file)?.let { builder.addAction(0, "Open", it) } }
        addActions(context, builder, finished.optJSONArray("actions"), finished.optString("route", "/downloads"))
        notify(context, NotificationManagerCompat.from(context), FINISHED_ID, builder.build())
    }

    /**
     * Opens a saved file (one from a website, `{uri, mime}`) in the app Android picks, straight
     * from the notice: a button can't start it through Raygun once it's in the background.
     */
    private fun openFileIntent(context: Context, file: JSONObject): PendingIntent? {
        val uri = file.optString("uri").takeIf { it.isNotBlank() } ?: return null
        val view = runCatching { Storage(context).view(android.net.Uri.parse(uri), file.optString("mime")) }.getOrNull()
            ?: return null
        return PendingIntent.getActivity(
            context,
            uri.hashCode(),
            view,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private fun build(context: Context, shown: JSONObject, grouped: Boolean, summary: Boolean, expires: Boolean): Notification {
        val route = shown.optString("route", "/downloads")
        val ongoing = shown.optBoolean("ongoing", true)
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle(firstText(shown.optString("title")) ?: "Downloads")
            .setContentText(firstText(shown.optString("text")) ?: if (ongoing) "Downloading" else "In Downloads")
            .setOngoing(ongoing)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setShowWhen(false)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
        if (expires) builder.setTimeoutAfter(TIMEOUT_MS)
        firstText(if (shown.isNull("info")) null else shown.optString("info"))?.let(builder::setSubText)
        if (ongoing) {
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
        // Pressed on a notification left by a Raygun that was killed: nothing in this process
        // downloads, so it goes.
        if (DownloadNotices.plan == null) {
            DownloadNotices.clearStale(context.applicationContext)
            return
        }
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
            DownloadNotices.serviceEnded(applicationContext)
            return START_NOT_STICKY
        }
        if (!DownloadNotices.foreground) finishNow()
        return START_NOT_STICKY
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        finishNow()
        DownloadNotices.serviceEnded(applicationContext)
    }

    /**
     * What the service stands with when no summary is known: Android wants a notification at
     * once. It only stands while there's nothing to download (it finishes right after), so it's
     * seldom seen, but it says what's going on.
     */
    private fun placeholder(): Notification =
        NotificationCompat.Builder(this, DownloadNotices.CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle("Downloads")
            .setContentText("Checking what's left to download")
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
        val manager = NotificationManagerCompat.from(this)
        if (keep != null) {
            if (manager.areNotificationsEnabled()) runCatching { manager.notify(DownloadNotices.SUMMARY_ID, keep) }
        } else {
            // `REMOVE` only takes the notification while it's the foreground one, and Android may
            // ignore a cancel that comes right after: both, and once more in a moment.
            manager.cancel(DownloadNotices.SUMMARY_ID)
            DownloadNotices.cancelSummaryLater(applicationContext)
        }
    }

    override fun onDestroy() {
        if (running === this) running = null
        super.onDestroy()
    }

    companion object {
        @Volatile
        private var running: RaygunDownloadService? = null

        val isRunning: Boolean get() = running != null

        /** Stops the service if it runs; false when none did. */
        fun finish(): Boolean {
            val service = running ?: return false
            service.finishNow()
            return true
        }
    }
}

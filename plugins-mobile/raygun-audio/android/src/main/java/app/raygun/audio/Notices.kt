package app.raygun.audio

import android.content.Context
import android.util.Log
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject

/**
 * Notifications that open a screen when tapped, besides the downloads' own (a followed artist's
 * new release): Rust says what they say, their channel and the route a tap opens
 * (`raygun-app/src/notifications`). The tap starts or brings back Raygun with the route
 * ([openRaygunIntent]), which waits in Rust until the UI takes it.
 *
 * They're posted under [TAG], so the downloads' stray sweep (which only clears untagged ones of
 * its own ids) never takes them.
 */
object Notices {
    private const val TAG = "raygun.notice"

    /**
     * Shows `notice` (`{"id", "channel", "channelName", "channelDescription", "alerts",
     * "retiredChannels", "title", "text", "route"}`); the same id in a channel replaces the one
     * shown. Only a channel that `alerts` (news of releases) makes a sound or pops up; the rest are
     * low and silent. Android keeps a channel's importance as first made, so a channel that went
     * quiet has a new id and Rust lists the old ones in `retiredChannels`, deleted here. Main thread.
     */
    fun show(context: Context, notice: JSONObject) {
        val channel = notice.optString("channel")
        if (channel.isEmpty()) return
        val alerts = notice.optBoolean("alerts", false)
        val manager = NotificationManagerCompat.from(context)
        notice.optJSONArray("retiredChannels")?.let { retired ->
            for (index in 0 until retired.length()) {
                val old = retired.optString(index)
                if (old.isNotEmpty() && old != channel) manager.deleteNotificationChannel(old)
            }
        }
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(
                channel,
                if (alerts) NotificationManagerCompat.IMPORTANCE_DEFAULT else NotificationManagerCompat.IMPORTANCE_LOW,
            )
                .setName(notice.optString("channelName", channel))
                .setDescription(notice.optString("channelDescription"))
                .apply { if (!alerts) setSound(null, null).setVibrationEnabled(false).setShowBadge(false) }
                .build(),
        )
        // Both lines say something (Rust fills them too); one that says nothing isn't shown.
        val said = firstText(notice.optString("title"), notice.optString("text")) ?: return
        val title = firstText(notice.optString("title"), notice.optString("channelName")) ?: said
        val text = firstText(notice.optString("text"), notice.optString("channelDescription")) ?: said
        val builder = NotificationCompat.Builder(context, channel)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setAutoCancel(true)
            .setCategory(if (alerts) NotificationCompat.CATEGORY_RECOMMENDATION else NotificationCompat.CATEGORY_STATUS)
            .setPriority(if (alerts) NotificationCompat.PRIORITY_DEFAULT else NotificationCompat.PRIORITY_LOW)
            .setOnlyAlertOnce(true)
            .setSilent(!alerts)
        val route = notice.optString("route")
        if (route.isNotEmpty()) openRaygunIntent(context, route)?.let(builder::setContentIntent)
        if (!manager.areNotificationsEnabled()) return
        try {
            manager.notify(TAG, notice.optInt("id"), builder.build())
        } catch (error: SecurityException) {
            Log.w("RaygunAudio", "notifications aren't allowed", error)
        }
    }

    /** Takes away the notice shown with `id`: what it said is over. Main thread. */
    fun cancel(context: Context, id: Int) {
        NotificationManagerCompat.from(context).cancel(TAG, id)
    }
}

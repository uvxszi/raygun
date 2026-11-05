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
     * Shows `notice` (`{"id", "channel", "channelName", "channelDescription", "title", "text",
     * "route"}`); the same id in a channel replaces the one shown. Main thread.
     */
    fun show(context: Context, notice: JSONObject) {
        val channel = notice.optString("channel")
        if (channel.isEmpty()) return
        val manager = NotificationManagerCompat.from(context)
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(channel, NotificationManagerCompat.IMPORTANCE_DEFAULT)
                .setName(notice.optString("channelName", channel))
                .setDescription(notice.optString("channelDescription"))
                .build(),
        )
        val text = notice.optString("text")
        val builder = NotificationCompat.Builder(context, channel)
            .setSmallIcon(R.drawable.ic_stat_raygun)
            .setContentTitle(notice.optString("title"))
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_RECOMMENDATION)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
        val route = notice.optString("route")
        if (route.isNotEmpty()) openRaygunIntent(context, route)?.let(builder::setContentIntent)
        if (!manager.areNotificationsEnabled()) return
        try {
            manager.notify(TAG, notice.optInt("id"), builder.build())
        } catch (error: SecurityException) {
            Log.w("RaygunAudio", "notifications aren't allowed", error)
        }
    }
}

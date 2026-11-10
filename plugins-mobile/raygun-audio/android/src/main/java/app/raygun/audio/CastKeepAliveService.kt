package app.raygun.audio

import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

/**
 * Keeps Raygun running while a TV plays a video from it (Play on TV): the TV reads the stream
 * through Raygun's proxy, so an app Android froze in the background would stop it there. Shown as
 * a quiet notification ("Playing on Living Room") while it runs; Rust starts and stops it with the
 * cast.
 */
class CastKeepAliveService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        ensureChannel(this)
        val launch = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        }
        val notification = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.media3_notification_small_icon)
            // A TV without a name or a video without a title still says what runs.
            .setContentTitle(firstText(intent?.getStringExtra(EXTRA_TITLE)) ?: "Playing on a TV")
            .setContentText(firstText(intent?.getStringExtra(EXTRA_TEXT)) ?: "Raygun keeps the video going")
            .setOngoing(true)
            .setSilent(true)
            .setContentIntent(launch)
            .build()
        try {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification,
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
            )
        } catch (error: Exception) {
            Log.w("RaygunAudio", "casting couldn't run in the foreground", error)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        stopSelf()
    }

    companion object {
        private const val CHANNEL = "raygun-cast"
        /** Its own: the recording service's is 7_301, and one would replace the other. */
        private const val NOTIFICATION_ID = 7_302
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_TEXT = "text"

        /** Starts the service, or changes what its notification says. */
        fun start(context: Context, title: String, text: String) {
            val intent = Intent(context, CastKeepAliveService::class.java)
                .putExtra(EXTRA_TITLE, title)
                .putExtra(EXTRA_TEXT, text)
            try {
                ContextCompat.startForegroundService(context, intent)
            } catch (error: Exception) {
                // Android refuses foreground services started from the background; the cast goes
                // on while Raygun is open.
                Log.w("RaygunAudio", "casting can't keep Raygun running", error)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, CastKeepAliveService::class.java))
        }

        private fun ensureChannel(context: Context) {
            NotificationManagerCompat.from(context).createNotificationChannel(
                NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_LOW)
                    .setName("Play on TV")
                    .setDescription("While a TV plays a video from Raygun")
                    .setShowBadge(false)
                    .build(),
            )
        }
    }
}

package app.raygun.audio

import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.os.SystemClock
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject

/**
 * What Settings → Permissions shows (`AppPermission` in raygun-model): each permission Raygun uses
 * and its state, `granted`, `denied` (Android can ask), `blocked` (only its settings can allow
 * it), and for the battery `optimized` (Android's usual limits) or `restricted`.
 */
internal object Permissions {
    /** "Not now" to Raygun's own question, by kind: Android isn't asked right after it either. */
    private val declinedAt = mutableMapOf<String, Long>()
    private const val DECLINE_HOLDS_MS = 2 * 60 * 1000L

    fun decline(kind: String) {
        declinedAt[kind] = SystemClock.elapsedRealtime()
    }

    fun recentlyDeclined(kind: String): Boolean =
        declinedAt[kind]?.let { SystemClock.elapsedRealtime() - it < DECLINE_HOLDS_MS } == true

    fun row(kind: String, state: String, count: Int? = null): JSObject =
        JSObject().put("kind", kind).put("state", state).put("count", count)

    fun battery(context: Context): String {
        val restricted = Build.VERSION.SDK_INT >= Build.VERSION_CODES.P &&
            context.getSystemService(ActivityManager::class.java)?.isBackgroundRestricted == true
        if (restricted) return "restricted"
        val power = context.getSystemService(PowerManager::class.java)
        return if (power?.isIgnoringBatteryOptimizations(context.packageName) == true) "granted" else "optimized"
    }

    /** Below Android 13 there's no question to ask: notifications are on or off in settings. */
    fun notificationsOff(context: Context): Boolean = !NotificationManagerCompat.from(context).areNotificationsEnabled()

    /** Picked folders only: single files other apps opened with Raygun ([OpenWith]) aren't counted. */
    fun folders(context: Context): Int =
        context.contentResolver.persistedUriPermissions.count {
            it.isReadPermission && DocumentsContract.isTreeUri(it.uri)
        }

    fun canInstall(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.O || context.packageManager.canRequestPackageInstalls()

    fun list(rows: List<JSObject>): JSObject {
        val array = JSArray()
        rows.forEach { array.put(it) }
        return JSObject().put("permissions", array)
    }

    /** The system page where `kind` is changed by hand. */
    fun settingsIntent(context: Context, kind: String): Intent {
        val pkg = context.packageName
        return when (kind) {
            "notifications" -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, pkg)
            } else {
                appDetails(pkg)
            }
            "installApps" -> if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:$pkg"))
            } else {
                appDetails(pkg)
            }
            "allFiles" -> LocalStorage.allFilesIntents(context).firstOrNull() ?: appDetails(pkg)
            else -> appDetails(pkg)
        }.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    /** Android's own question about the battery ("Let Raygun always run in the background?"). */
    fun batteryQuestion(context: Context): Intent =
        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${context.packageName}"))

    private fun appDetails(pkg: String) =
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", pkg, null))
}

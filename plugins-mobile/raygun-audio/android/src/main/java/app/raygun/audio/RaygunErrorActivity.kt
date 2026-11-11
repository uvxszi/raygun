package app.raygun.audio

import android.app.ActivityManager
import android.content.ContentValues
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Process
import android.provider.MediaStore
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.addCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File

/**
 * Raygun's error screen when its page stopped working: the webview's renderer went (Android would
 * end Raygun with it) or Raygun's page couldn't load. It runs in a process of its own, so it stays
 * when Raygun's main process ends, and draws itself without the webview, in Raygun's colours.
 *
 * Try again opens Raygun as it was; Start in safe mode and Clear caches start it again as a safe
 * start (`raygun-app/src/crash_log.rs`: the page doesn't restore what it saved, queues wait, the
 * caches are emptied first for the latter); See logs shows the crash record just written, to
 * share or save in Download/Raygun/logs. Nothing is sent anywhere by itself.
 */
class RaygunErrorActivity : ComponentActivity() {
    companion object {
        const val EXTRA_KIND = "kind"
    }

    private val dark by lazy {
        (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES
    }
    private val bgColor get() = if (dark) Color.rgb(15, 15, 18) else Color.rgb(250, 250, 250)
    private val surfaceColor get() = if (dark) Color.rgb(30, 30, 36) else Color.rgb(236, 236, 240)
    private val textColor get() = if (dark) Color.rgb(242, 242, 242) else Color.rgb(24, 24, 27)
    private val mutedColor get() = if (dark) Color.rgb(161, 161, 170) else Color.rgb(82, 82, 91)
    private val accentColor get() = if (dark) Color.rgb(242, 104, 92) else Color.rgb(200, 55, 47)

    private lateinit var frame: ScrollView
    private var showingLog = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        CrashLog.install(this)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
        frame = ScrollView(this).apply {
            setBackgroundColor(bgColor)
            isFillViewport = true
        }
        ViewCompat.setOnApplyWindowInsetsListener(frame) { view, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
            )
            view.setPadding(bars.left + dp(24), bars.top + dp(24), bars.right + dp(24), bars.bottom + dp(24))
            insets
        }
        setContentView(frame)
        onBackPressedDispatcher.addCallback(this) {
            if (showingLog) showMain() else finish()
        }
        showMain()
    }

    private fun dp(value: Int): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value.toFloat(), resources.displayMetrics).toInt()

    private fun column(vararg views: View): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            views.forEach { addView(it) }
        }

    private fun label(value: String, size: Float, color: Int, bold: Boolean = false): TextView =
        TextView(this).apply {
            text = value
            textSize = size
            setTextColor(color)
            if (bold) typeface = Typeface.DEFAULT_BOLD
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { bottomMargin = dp(12) }
        }

    private fun button(title: String, primary: Boolean = false, onClick: () -> Unit): Button =
        Button(this).apply {
            text = title
            isAllCaps = false
            textSize = 16f
            setTextColor(if (primary) Color.WHITE else textColor)
            background = GradientDrawable().apply {
                cornerRadius = dp(24).toFloat()
                setColor(if (primary) accentColor else surfaceColor)
            }
            minHeight = dp(48)
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(10) }
            setOnClickListener { runCatching(onClick) }
        }

    private fun showMain() {
        showingLog = false
        val gone = intent.getStringExtra(EXTRA_KIND) == "page"
        frame.removeAllViews()
        frame.addView(
            column(
                label("Something went wrong", 24f, textColor, bold = true),
                label(
                    if (gone) {
                        "Raygun's screen couldn't load."
                    } else {
                        "Raygun's screen stopped working."
                    } + " Your library, downloads and settings are safe.",
                    16f,
                    textColor,
                ),
                label(
                    "Try again to carry on. If it happens again, start in safe mode: Raygun then " +
                        "skips what it restores at start. A crash log was kept on this device; " +
                        "nothing is sent anywhere.",
                    14f,
                    mutedColor,
                ),
                button("Try again", primary = true) { reopen(safe = false, clearCaches = false) },
                button("See logs") { showLog() },
                button("Start in safe mode") { reopen(safe = true, clearCaches = false) },
                button("Clear caches") { reopen(safe = true, clearCaches = true) },
            ),
        )
    }

    private fun showLog() {
        showingLog = true
        val file = CrashLog.latest(this)
        val raw = file?.let { runCatching { it.readText().take(200_000) }.getOrNull() }
        // Its time as this device shows dates, not milliseconds.
        val log = raw?.let(CrashLog::readable) ?: "No crash log was kept."
        val body = label(log, 12f, textColor).apply {
            typeface = Typeface.MONOSPACE
            setTextIsSelectable(true)
            background = GradientDrawable().apply {
                cornerRadius = dp(12).toFloat()
                setColor(surfaceColor)
            }
            setPadding(dp(12), dp(12), dp(12), dp(12))
        }
        frame.removeAllViews()
        frame.addView(
            column(
                label("Crash log", 22f, textColor, bold = true),
                body,
                button("Share", primary = true) { share(log) },
                button("Save in Download/Raygun/logs") { save(file, log) },
                button("Back") { showMain() },
            ),
        )
        frame.scrollTo(0, 0)
    }

    private fun share(log: String) {
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, "Raygun crash log")
            .putExtra(Intent.EXTRA_TEXT, log.take(100_000))
        startActivity(Intent.createChooser(send, "Share the crash log"))
    }

    private fun save(file: File?, log: String) {
        val name = "raygun-${file?.name ?: "crash-${System.currentTimeMillis()}.txt"}"
        val shown = runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values = ContentValues().apply {
                    put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                    put(MediaStore.MediaColumns.MIME_TYPE, "text/plain")
                    put(MediaStore.MediaColumns.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/Raygun/logs")
                }
                val uri = contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                    ?: error("no file")
                contentResolver.openOutputStream(uri)?.use { it.write(log.toByteArray()) }
                "Download/Raygun/logs/$name"
            } else {
                val folder = File(getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS), "logs").apply { mkdirs() }
                File(folder, name).writeText(log)
                folder.absolutePath + "/" + name
            }
        }
        Toast.makeText(
            this,
            shown.map { "Saved in $it" }.getOrElse { "Couldn't save the crash log" },
            Toast.LENGTH_LONG,
        ).show()
    }

    /**
     * Opens Raygun again. A safe start (or one with its caches cleared) needs a fresh start of its
     * main process, which reads the flags; Try again keeps a process that still plays music.
     */
    private fun reopen(safe: Boolean, clearCaches: Boolean) {
        if (safe) CrashLog.flag(this, CrashLog.SAFE_START) else CrashLog.unflag(this, CrashLog.RENDERER_GONE)
        if (clearCaches) CrashLog.flag(this, CrashLog.CLEAR_CACHES)
        if (safe) {
            getSystemService(ActivityManager::class.java)?.runningAppProcesses
                ?.filter { it.uid == Process.myUid() && it.pid != Process.myPid() && it.processName == packageName }
                ?.forEach {
                    // Ended on purpose: not a crash for the next start's records.
                    CrashLog.expectExit(this, it.pid)
                    Process.killProcess(it.pid)
                }
        }
        packageManager.getLaunchIntentForPackage(packageName)?.let { launch ->
            startActivity(launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
        finish()
    }
}

package app.raygun.audio

import android.app.Activity
import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import android.os.Process
import android.webkit.RenderProcessGoneDetail
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import kotlin.concurrent.thread
import kotlin.system.exitProcess

/**
 * Crash records from the Kotlin side, in `logs/crashes/` of Raygun's data folder: Rust lists
 * them (`raygun-app/src/crash_log.rs`, About → Crash logs) and writes its own there in the same
 * shape (the first lines `Kind:`, `Time:` in milliseconds, `Version:`, `Summary:`). Nothing is
 * sent anywhere.
 *
 * - Uncaught exceptions on any thread, before Android's own handler ends the process.
 * - The webview's renderer gone (`RaygunErrorActivity` shows instead of the page).
 * - How earlier runs ended, as the system tells it (Android 11+): native crashes with their
 *   tombstone, ANRs with their traces, low-memory kills. Read once each, at the next start.
 */
object CrashLog {
    private const val DIR = "logs/crashes"
    private const val EXITS_SEEN = ".exits-seen"

    /** Flags Rust takes at the next start (`crash_log.rs`). */
    const val RENDERER_GONE = "renderer-gone"
    const val SAFE_START = "safe-start"
    const val CLEAR_CACHES = "clear-caches"

    @Volatile
    private var installed = false

    /** Whether Raygun's main activity is in front (set by `MainActivity`). */
    @Volatile
    @JvmStatic
    var mainInFront = false

    /** Raygun's data folder holds the logs (Rust's data root on Android). */
    @JvmStatic
    fun dir(context: Context): File = File(context.applicationInfo.dataDir, DIR)

    /** Installs the uncaught exception handler and reads how earlier runs ended. Once a process. */
    @JvmStatic
    fun install(context: Context) {
        if (installed) return
        installed = true
        val app = context.applicationContext
        appContext = app
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { failed, error ->
            runCatching { write(app, "java", summary(error), exceptionBody(failed, error)) }
            if (previous != null) {
                previous.uncaughtException(failed, error)
            } else {
                Process.killProcess(Process.myPid())
                exitProcess(10)
            }
        }
        thread(name = "raygun-exits") { runCatching { recordExits(app) } }
    }

    /** Writes one record: the file, or null when it couldn't. */
    @JvmStatic
    fun write(context: Context, kind: String, summary: String, body: String, at: Long = System.currentTimeMillis()): File? =
        runCatching {
            val folder = dir(context).apply { mkdirs() }
            var file = File(folder, "$at-$kind.txt")
            var tries = 1
            while (file.exists() && tries < 20) {
                file = File(folder, "$at-$kind-$tries.txt")
                tries++
            }
            val version = runCatching {
                context.packageManager.getPackageInfo(context.packageName, 0).versionName
            }.getOrNull() ?: "?"
            val oneLine = summary.replace(Regex("\\s+"), " ").take(300)
            file.writeText(
                "Raygun crash record\nKind: $kind\nTime: $at\nVersion: $version (android, " +
                    "${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE})\n" +
                    "Summary: $oneLine\n\n$body",
            )
            file
        }.getOrNull()

    /** Leaves a flag for the next start. */
    @JvmStatic
    fun flag(context: Context, name: String) {
        runCatching { File(dir(context).apply { mkdirs() }, name).writeText("") }
    }

    /** Takes a flag back. */
    @JvmStatic
    fun unflag(context: Context, name: String) {
        runCatching { File(dir(context), name).delete() }
    }

    /** The newest record, if any. */
    @JvmStatic
    fun latest(context: Context): File? =
        dir(context).listFiles { file -> file.isFile && file.name.endsWith(".txt") }
            ?.maxByOrNull { it.name.substringBefore('-').toLongOrNull() ?: 0L }

    /** The webview's renderer went: a record, and the next start is a safe one unless retried. */
    @JvmStatic
    fun rendererGone(context: Context, detail: RenderProcessGoneDetail?) {
        val crashed = detail?.didCrash() ?: true
        val memory = memoryState(context)
        val priority = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && detail != null) {
            detail.rendererPriorityAtExit()
        } else {
            -1
        }
        val summary = if (crashed) {
            "The page's renderer crashed"
        } else {
            "The system ended the page's renderer to free memory"
        }
        write(
            context,
            "renderer",
            summary,
            "Raygun's page (the webview's renderer process) ended. Android would have ended " +
                "Raygun with it; Raygun showed its error screen instead.\n\n" +
                "Crashed: $crashed\nRenderer priority at exit: $priority\n" +
                "Raygun in front: $mainInFront\n$memory\n",
        )
        flag(context, RENDERER_GONE)
    }

    private fun memoryState(context: Context): String = runCatching {
        val manager = context.getSystemService(ActivityManager::class.java)
        val info = ActivityManager.MemoryInfo().also { manager.getMemoryInfo(it) }
        val runtime = Runtime.getRuntime()
        "Device memory: ${info.availMem / MB} MB free of ${info.totalMem / MB} MB" +
            " (low: ${info.lowMemory}, threshold ${info.threshold / MB} MB)\n" +
            "Java heap: ${(runtime.totalMemory() - runtime.freeMemory()) / MB} MB of ${runtime.maxMemory() / MB} MB"
    }.getOrDefault("")

    private fun summary(error: Throwable): String {
        val root = generateSequence(error) { it.cause }.last()
        val where = error.stackTrace.firstOrNull()?.let { " at ${it.className}.${it.methodName}" }.orEmpty()
        return "${root.javaClass.simpleName}: ${root.message.orEmpty()}$where"
    }

    private fun exceptionBody(failed: Thread, error: Throwable): String {
        val trace = StringWriter().also { error.printStackTrace(PrintWriter(it)) }
        val memory = appContext?.let { memoryState(it) }.orEmpty()
        return "An exception nobody caught ended Raygun.\n\nThread: ${failed.name}\n$memory\n\n" +
            "Stack trace:\n$trace"
    }

    /** For the exception handler's memory line (set at install). */
    @Volatile
    private var appContext: Context? = null

    /**
     * How earlier runs of Raygun's processes ended (Android 11+), each recorded once: crashes the
     * process couldn't record itself (native ones, with the tombstone beside the record), ANRs
     * with their traces, and the system ending it for memory.
     */
    private fun recordExits(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        val manager = context.getSystemService(ActivityManager::class.java) ?: return
        val folder = dir(context).apply { mkdirs() }
        val seenFile = File(folder, EXITS_SEEN)
        val seen = runCatching { seenFile.readText().trim().toLong() }.getOrDefault(0L)
        val exits = manager.getHistoricalProcessExitReasons(context.packageName, 0, 16)
            .filter { it.timestamp > seen }
            .sortedBy { it.timestamp }
        for (exit in exits) {
            val kind = when (exit.reason) {
                ApplicationExitInfo.REASON_CRASH_NATIVE -> "native"
                ApplicationExitInfo.REASON_ANR -> "anr"
                ApplicationExitInfo.REASON_LOW_MEMORY -> "memory"
                ApplicationExitInfo.REASON_CRASH -> "exit"
                ApplicationExitInfo.REASON_SIGNALED,
                ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
                ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
                ApplicationExitInfo.REASON_DEPENDENCY_DIED,
                -> "exit"
                else -> null
            } ?: continue
            recordExit(context, folder, exit, kind)
        }
        exits.maxOfOrNull { it.timestamp }?.let { newest -> runCatching { seenFile.writeText("$newest") } }
    }

    @androidx.annotation.RequiresApi(Build.VERSION_CODES.R)
    private fun recordExit(context: Context, folder: File, exit: ApplicationExitInfo, kind: String) {
        val reason = reasonName(exit.reason)
        val description = exit.description.orEmpty()
        var trace = ""
        var abort = ""
        runCatching {
            exit.traceInputStream?.use { stream ->
                val bytes = stream.readBytes()
                if (exit.reason == ApplicationExitInfo.REASON_CRASH_NATIVE) {
                    File(folder, "${exit.timestamp}-$kind.tombstone.pb").writeBytes(bytes)
                    abort = Tombstone.abortMessage(bytes).orEmpty()
                    trace = "The tombstone (protobuf) is beside this record: " +
                        "${exit.timestamp}-$kind.tombstone.pb\n"
                } else {
                    trace = String(bytes.copyOf(minOf(bytes.size, 512 * 1024)))
                }
            }
        }
        val summary = listOf(reason, abort.ifBlank { description })
            .filter { it.isNotBlank() }
            .joinToString(": ")
        write(
            context,
            kind,
            summary,
            "How Android says a run of Raygun ended.\n\nProcess: ${exit.processName}\n" +
                "Reason: $reason\nDescription: $description\n" +
                (if (abort.isNotBlank()) "Abort message: $abort\n" else "") +
                "Status: ${exit.status}\nImportance: ${exit.importance}\n" +
                "Memory: ${exit.pss / 1024} MB PSS, ${exit.rss / 1024} MB RSS\n\n$trace",
            exit.timestamp,
        )
    }

    private fun reasonName(reason: Int): String = when (reason) {
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "Native crash"
        ApplicationExitInfo.REASON_ANR -> "Not responding (ANR)"
        ApplicationExitInfo.REASON_LOW_MEMORY -> "Ended by the system for memory"
        ApplicationExitInfo.REASON_CRASH -> "Uncaught exception"
        ApplicationExitInfo.REASON_SIGNALED -> "Ended by a signal"
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "Used too many resources"
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "Failed to start"
        ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "A process it depends on ended"
        else -> "Reason $reason"
    }

    /** Raygun's error screen in place of a page that went (its own process: it outlives Raygun's). */
    @JvmStatic
    fun showErrorScreen(activity: Activity, kind: String) {
        runCatching {
            activity.startActivity(
                android.content.Intent(activity, RaygunErrorActivity::class.java)
                    .putExtra(RaygunErrorActivity.EXTRA_KIND, kind)
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }

    private const val MB = 1024L * 1024L
}

/** Reads the abort message from an Android tombstone (protobuf, `tombstone.proto` field 14). */
internal object Tombstone {
    fun abortMessage(bytes: ByteArray): String? {
        var position = 0
        fun varint(): Long {
            var result = 0L
            var shift = 0
            while (position < bytes.size && shift < 64) {
                val byte = bytes[position++].toInt() and 0xFF
                result = result or ((byte and 0x7F).toLong() shl shift)
                if (byte and 0x80 == 0) return result
                shift += 7
            }
            return result
        }
        while (position < bytes.size) {
            val key = varint()
            val field = (key ushr 3).toInt()
            when ((key and 7).toInt()) {
                0 -> varint()
                1 -> position += 8
                2 -> {
                    val length = varint().toInt()
                    if (length < 0 || position + length > bytes.size) return null
                    if (field == 14) return String(bytes, position, length, Charsets.UTF_8)
                    position += length
                }
                5 -> position += 4
                else -> return null
            }
        }
        return null
    }
}

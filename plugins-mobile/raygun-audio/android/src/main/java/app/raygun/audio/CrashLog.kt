package app.raygun.audio

import android.app.Activity
import android.app.ActivityManager
import android.app.Application
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import android.os.Process
import android.webkit.RenderProcessGoneDetail
import java.io.File
import java.io.PrintWriter
import java.io.RandomAccessFile
import java.io.StringWriter
import java.text.DateFormat
import java.util.Date
import kotlin.concurrent.thread
import kotlin.math.abs
import kotlin.system.exitProcess

/**
 * Crash records from the Kotlin side, in `logs/crashes/` of Raygun's data folder: Rust lists
 * them (`raygun-app/src/crash_log.rs`, About → Crash logs) and writes its own there in the same
 * shape (the first lines `Kind:`, `Time:` in milliseconds, `Version:`, `Pid:`, `Process:`,
 * `Summary:`). Nothing is sent anywhere.
 *
 * - Uncaught exceptions on any thread of any of Raygun's processes, before Android's own handler
 *   ends the process (every activity and service installs the handler: a process may start
 *   with any of them).
 * - The webview's renderer gone (`RaygunErrorActivity` shows instead of the page), once per death.
 * - How Raygun's processes ended, as the system tells it (Android 11+): crashes, native crashes
 *   with their tombstone, ANRs with their traces, signals, the system ending a process in use for
 *   memory. Read when a process starts, when Raygun comes to the front, and when the crash logs
 *   are listed; one already recorded at the time (same process, same moment) isn't again.
 */
object CrashLog {
    private const val DIR = "logs/crashes"
    private const val EXITS_SEEN = ".exits-seen"
    private const val EXITS_LOCK = ".exits-lock"
    /** Processes Raygun ended on purpose (a safe-mode restart): their exits aren't crashes. */
    private const val EXPECTED_EXITS = ".expected-exits"

    /** Flags Rust takes at the next start (`crash_log.rs`). */
    const val RENDERER_GONE = "renderer-gone"
    const val SAFE_START = "safe-start"
    const val CLEAR_CACHES = "clear-caches"

    /** An exit within this of a record of the same process is that record's. */
    private const val SAME_MOMENT_MS = 60_000L
    /** A renderer reported gone again within this is the same death. */
    private const val RENDERER_AGAIN_MS = 10_000L

    @Volatile
    private var installed = false

    /** Whether Raygun's main activity is in front (set by `MainActivity`). */
    @Volatile
    @JvmStatic
    var mainInFront = false

    @Volatile
    private var lastRendererGone = 0L

    /** For the exception handler's memory line (set at install). */
    @Volatile
    private var appContext: Context? = null

    /** Raygun's data folder holds the logs (Rust's data root on Android). */
    @JvmStatic
    fun dir(context: Context): File = File(context.applicationInfo.dataDir, DIR)

    /**
     * Installs the uncaught exception handler and reads how earlier runs ended. Once a process;
     * every activity and service of Raygun calls it first.
     */
    @JvmStatic
    fun install(context: Context) {
        if (installed) return
        installed = true
        val app = context.applicationContext ?: context
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
        recordExitsLater(app)
    }

    /** [recordExits] off the calling thread. */
    @JvmStatic
    fun recordExitsLater(context: Context) {
        val app = context.applicationContext ?: context
        thread(name = "raygun-exits") { runCatching { recordExits(app) } }
    }

    /** This process's name (`app.raygun.player`, `app.raygun.player:web`…). */
    private fun processName(): String =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            Application.getProcessName()
        } else {
            runCatching { File("/proc/self/cmdline").readText().trim('\u0000', ' ') }.getOrDefault("?")
        }

    /** Writes one record: the file, or null when it couldn't. */
    @JvmStatic
    @JvmOverloads
    fun write(
        context: Context,
        kind: String,
        summary: String,
        body: String,
        at: Long = System.currentTimeMillis(),
        pid: Int = Process.myPid(),
        process: String = processName(),
    ): File? =
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
                    "Pid: $pid\nProcess: $process\nSummary: $oneLine\n\n$body",
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

    /** Raygun is about to end process [pid] itself: its exit isn't a crash. */
    @JvmStatic
    fun expectExit(context: Context, pid: Int) {
        runCatching {
            val file = File(dir(context).apply { mkdirs() }, EXPECTED_EXITS)
            val kept = runCatching { file.readLines() }.getOrDefault(emptyList()).takeLast(19)
            file.writeText((kept + pid.toString()).joinToString("\n"))
        }
    }

    /** The newest record, if any. */
    @JvmStatic
    fun latest(context: Context): File? =
        dir(context).listFiles { file -> file.isFile && file.name.endsWith(".txt") }
            ?.maxByOrNull { it.name.substringBefore('-').toLongOrNull() ?: 0L }

    /** A record's text as people read it: its `Time:` line in this device's date and time. */
    @JvmStatic
    fun readable(text: String): String =
        Regex("(?m)^Time: (\\d{10,})$").replace(text) { match ->
            val millis = match.groupValues[1].toLongOrNull() ?: return@replace match.value
            "Time: " + DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.MEDIUM).format(Date(millis))
        }

    /**
     * The webview's renderer went: a record, and the next start is a safe one unless retried.
     * Android may report one death more than once (each view, each client): only the first is
     * recorded, and only it shows the error screen (false for the others).
     */
    @JvmStatic
    fun rendererGone(context: Context, detail: RenderProcessGoneDetail?): Boolean {
        val now = System.currentTimeMillis()
        synchronized(this) {
            if (now - lastRendererGone < RENDERER_AGAIN_MS) return false
            lastRendererGone = now
        }
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
            now,
        )
        flag(context, RENDERER_GONE)
        return true
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

    /** A record's process and moment, from its first lines. */
    private data class Recorded(val pid: Int?, val at: Long, val kind: String)

    private fun recorded(folder: File): List<Recorded> =
        folder.listFiles { file -> file.isFile && file.name.endsWith(".txt") }.orEmpty().mapNotNull { file ->
            runCatching {
                val head = file.bufferedReader().use { reader -> generateSequence { reader.readLine() }.take(12).toList() }
                fun field(name: String) = head.firstOrNull { it.startsWith("$name: ") }?.substringAfter(": ")?.trim()
                val at = field("Time")?.toLongOrNull() ?: file.name.substringBefore('-').toLongOrNull() ?: return@runCatching null
                Recorded(field("Pid")?.toIntOrNull(), at, field("Kind").orEmpty())
            }.getOrNull()
        }

    /**
     * How Raygun's processes ended (Android 11+), each recorded once: crashes (unless the process
     * recorded its own at the time: same process, same moment), native crashes with the tombstone
     * beside the record, ANRs with their traces, signals, and the system ending a process in use
     * for memory. Processes Raygun ended itself, and cached ones the system let go, aren't
     * recorded. Safe from any process at once (a lock file).
     */
    @JvmStatic
    fun recordExits(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        val manager = context.getSystemService(ActivityManager::class.java) ?: return
        val folder = dir(context).apply { mkdirs() }
        synchronized(this) {
            RandomAccessFile(File(folder, EXITS_LOCK), "rw").use { lockFile ->
                lockFile.channel.lock().use { recordExitsLocked(context, manager, folder) }
            }
        }
    }

    @androidx.annotation.RequiresApi(Build.VERSION_CODES.R)
    private fun recordExitsLocked(context: Context, manager: ActivityManager, folder: File) {
        val seenFile = File(folder, EXITS_SEEN)
        val seen = runCatching { seenFile.readText().trim().toLong() }.getOrDefault(0L)
        // A minute back from the newest seen: Android may file an exit a moment after a later one.
        // Those already recorded are recognised by their process and moment.
        val exits = manager.getHistoricalProcessExitReasons(context.packageName, 0, 16)
            .filter { it.timestamp > seen - SAME_MOMENT_MS }
            .sortedBy { it.timestamp }
        if (exits.isEmpty()) return
        val expected = runCatching { File(folder, EXPECTED_EXITS).readLines() }
            .getOrDefault(emptyList())
            .mapNotNull { it.trim().toIntOrNull() }
            .toSet()
        val known = recorded(folder)
        for (exit in exits) {
            val kind = kindOf(exit) ?: continue
            if (exit.pid in expected) continue
            val already = known.any { record ->
                abs(record.at - exit.timestamp) <= SAME_MOMENT_MS &&
                    (record.pid == exit.pid || (record.pid == null && record.kind == kind))
            }
            if (!already) recordExit(context, folder, exit, kind)
        }
        runCatching { seenFile.writeText("${maxOf(seen, exits.maxOf { it.timestamp })}") }
    }

    /** What a record of [exit] is called, or null for exits that aren't crashes. */
    @androidx.annotation.RequiresApi(Build.VERSION_CODES.R)
    private fun kindOf(exit: ApplicationExitInfo): String? = when (exit.reason) {
        ApplicationExitInfo.REASON_CRASH -> "java"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "native"
        ApplicationExitInfo.REASON_ANR -> "anr"
        // Cached processes go for memory all the time; one in use (in front, playing) matters.
        ApplicationExitInfo.REASON_LOW_MEMORY ->
            "memory".takeIf { exit.importance < ActivityManager.RunningAppProcessInfo.IMPORTANCE_CACHED }
        ApplicationExitInfo.REASON_SIGNALED,
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
        ApplicationExitInfo.REASON_DEPENDENCY_DIED,
        -> "exit"
        else -> null
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
            "How Android says a process of Raygun ended.\n\n" +
                "Reason: $reason\nDescription: $description\n" +
                (if (abort.isNotBlank()) "Abort message: $abort\n" else "") +
                "Status: ${exit.status}\nImportance: ${exit.importance}\n" +
                "Memory: ${exit.pss / 1024} MB PSS, ${exit.rss / 1024} MB RSS\n\n$trace",
            exit.timestamp,
            exit.pid,
            exit.processName,
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

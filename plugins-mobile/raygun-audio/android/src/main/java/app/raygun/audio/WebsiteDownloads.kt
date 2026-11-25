package app.raygun.audio

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.text.TextUtils
import android.text.format.Formatter
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject

/** A download as the browser shows it: what Raygun last said of it, or that it hasn't yet. */
internal class DownloadRow(val ticket: String) {
    var id = ""
    var name = ""
    var state = "queued"
    var done = 0.0
    var total: Double? = null
    var folder = ""
    var error: String? = null
    var saved: String? = null
    var mime: String? = null
    /** Why it waits, as Raygun says it. */
    var waiting: String? = null
    /** The request, kept until Raygun has heard of it (it's sent again till then). */
    var request: JSONObject? = null
    /** Raygun hasn't answered it for a while: it isn't running. */
    var unheard = false
}

/** Rows' words, apart from views (tested on the JVM). */
internal object DownloadWords {
    fun running(row: DownloadRow): Boolean = row.state == "queued" || row.state == "downloading"

    /** What a row says under its name; `size` writes a byte count. */
    fun status(row: DownloadRow, size: (Double) -> String): String {
        val total = row.total
        return when {
            row.unheard -> "Waiting for Raygun to start"
            row.state == "queued" -> row.waiting
                ?: if (row.folder.isNotBlank()) "Starting · to ${row.folder}" else "Starting…"
            row.state == "downloading" -> {
                val so = when {
                    total != null && total > 0 -> "${percent(row.done, total)}% · ${size(row.done)} of ${size(total)}"
                    row.done > 0 -> size(row.done)
                    else -> "Starting…"
                }
                row.waiting?.let { "$so · $it" } ?: so
            }
            row.state == "done" -> listOfNotNull(
                if (row.folder.isNotBlank()) "Saved in ${row.folder}" else "Saved",
                total?.let(size),
            ).joinToString(" · ")
            row.state == "failed" -> row.error ?: "Didn't download"
            else -> ""
        }
    }

    fun percent(done: Double, total: Double): Int = (done / total * 100).toInt().coerceIn(0, 100)

    /** How far the running downloads are together (0..1), -1 when a size isn't known, null with none. */
    fun overall(rows: List<DownloadRow>): Float? {
        val running = rows.filter(::running)
        if (running.isEmpty()) return null
        if (running.any { (it.total ?: 0.0) <= 0.0 }) return -1f
        val done = running.sumOf { it.done }
        val total = running.sumOf { it.total ?: 0.0 }
        return (done / total).toFloat().coerceIn(0f, 1f)
    }
}

/** A ring drawn round the Downloads button: how far, or a turning arc when that isn't known. */
private class RingView(context: Context) : View(context) {
    /** 0..1, -1 for a turning arc, null for none. */
    var fraction: Float? = null
        set(value) {
            if (field == value) return
            field = value
            invalidate()
        }
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeWidth = context.dp(3).toFloat()
    }
    private val box = RectF()

    override fun onDraw(canvas: Canvas) {
        val shown = fraction ?: return
        val inset = paint.strokeWidth
        box.set(inset, inset, width - inset, height - inset)
        paint.color = 0x33FFFFFF
        canvas.drawOval(box, paint)
        paint.color = ACCENT
        if (shown < 0f) {
            val turn = (SystemClock.uptimeMillis() % 1200L) / 1200f * 360f
            canvas.drawArc(box, turn - 90f, 90f, false, paint)
            postInvalidateOnAnimation()
        } else {
            canvas.drawArc(box, -90f, 360f * shown, false, paint)
        }
    }
}

/**
 * The downloads this browser started: a Downloads button in its toolbar (a ring for how far the
 * running ones are, a count of them) that opens a panel listing each (how far, Cancel, Retry,
 * Open, Show in folder), and a short toast as one starts and ends. Raygun downloads them in its
 * own process, which says how they go in a package-only broadcast ([WebsiteActivity.ACTION_DOWNLOADS],
 * `{"downloads": [...]}`) while the browser is on screen ([watch], [unwatch]).
 *
 * A request Raygun hasn't heard of is sent again whenever the browser comes on screen and when
 * Raygun says it started (`{"hello": true}`): Raygun's process may not be running (gone while the
 * browser was in front). After a few seconds without a word such a row says so and offers to open
 * Raygun.
 */
internal class DownloadList(
    private val activity: Activity,
    /** A question about a download to Raygun (`{"action", "ticket"}`). */
    private val ask: (JSONObject) -> Unit,
    /** A request to Raygun (`{"download": …}`). */
    private val send: (JSONObject) -> Unit,
    /** Shows a panel at the window's foot, and closes it. */
    private val host: (View) -> Unit,
    private val close: () -> Unit,
) {
    private companion object {
        /** Rows kept, the newest. */
        const val MAX_ROWS = 50
        /** Raygun not answering a request for this long is taken as not running, ms. */
        const val UNHEARD_MS = 6_000L
    }

    private class RowViews(
        val root: View,
        val name: TextView,
        val status: TextView,
        val progress: ProgressBar,
        val actions: LinearLayout,
        var shownActions: String = "",
    )

    private val main = Handler(Looper.getMainLooper())

    /** Newest first. */
    private val rows = mutableListOf<DownloadRow>()
    private val views = mutableMapOf<String, RowViews>()

    private val ring = RingView(activity)
    private val badge = TextView(activity).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
        typeface = Typeface.DEFAULT_BOLD
        gravity = Gravity.CENTER
        includeFontPadding = false
        background = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(ACCENT)
        }
        visibility = View.GONE
    }

    /** The toolbar's button. */
    val button: FrameLayout = FrameLayout(activity).apply {
        val ripple = TypedValue()
        activity.theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, ripple, true)
        setBackgroundResource(ripple.resourceId)
        isClickable = true
        isFocusable = true
        contentDescription = "Downloads"
        setOnClickListener { showPanel() }
        addView(ImageView(activity).apply {
            setImageResource(R.drawable.raygun_ic_download)
            setColorFilter(Color.WHITE)
        }, FrameLayout.LayoutParams(activity.dp(22), activity.dp(22), Gravity.CENTER))
        addView(ring, FrameLayout.LayoutParams(activity.dp(36), activity.dp(36), Gravity.CENTER))
        addView(badge, FrameLayout.LayoutParams(activity.dp(16), activity.dp(16), Gravity.TOP or Gravity.END).apply {
            topMargin = activity.dp(4)
            marginEnd = activity.dp(2)
        })
        layoutParams = LinearLayout.LayoutParams(activity.dp(44), activity.dp(44))
    }

    /** The panel while it's open: its rows' column. */
    private var panelRows: LinearLayout? = null
    private var panelEmpty: TextView? = null

    /** A download handed to Raygun just now, with its request (kept until Raygun says more). */
    fun add(ticket: String, name: String, folder: String, request: JSONObject) {
        val row = DownloadRow(ticket).apply {
            this.name = name
            this.folder = folder
            this.request = request
        }
        rows.add(0, row)
        while (rows.size > MAX_ROWS) rows.removeAt(rows.size - 1).let { views.remove(it.ticket) }
        toast("Downloading ${name.ifBlank { "the file" }}")
        expectWord(row)
        changed()
    }

    /** Raygun's word (`{"downloads": [...]}` or `{"hello": true}`); others' downloads are ignored. */
    fun update(json: String) {
        val message = runCatching { JSONObject(json) }.getOrNull() ?: return
        if (message.optBoolean("hello")) {
            resend()
            return
        }
        val list = message.optJSONArray("downloads") ?: return
        for (index in 0 until list.length()) {
            val item = list.optJSONObject(index) ?: continue
            val row = rows.firstOrNull { it.ticket == item.optString("ticket") } ?: continue
            val before = row.state
            if (item.optString("state") == "cancelled") {
                drop(row)
                continue
            }
            row.request = null
            row.unheard = false
            fill(row, item)
            if (before != row.state) {
                when (row.state) {
                    "done" -> toast("${row.name} saved" + if (row.folder.isNotBlank()) " in ${row.folder}" else "")
                    "failed" -> toast("${row.name} didn't download")
                }
            }
        }
        changed()
    }

    /**
     * The browser is on screen: requests Raygun hasn't heard go again, and Raygun says how the
     * downloads still on their way stand (it may have missed changes while away) and tells every
     * change from now on.
     */
    fun watch() {
        resend(watching = false)
        val waiting = rows.filter(DownloadWords::running).map { it.ticket }
        ask(JSONObject().put("action", "watch").put("tickets", JSONArray(waiting)))
    }

    /** The browser left the screen: Raygun stops telling (its process may be frozen meanwhile). */
    fun unwatch() {
        ask(JSONObject().put("action", "unwatch"))
    }

    /** Requests Raygun hasn't heard of, sent again; then (unless [watch] asks) the browser watches. */
    private fun resend(watching: Boolean = true) {
        val pending = rows.filter { it.request != null }
        pending.asReversed().forEach { row ->
            row.request?.let { send(JSONObject().put("download", it)) }
            expectWord(row)
        }
        if (watching) watch()
    }

    private fun expectWord(row: DownloadRow) {
        main.removeCallbacksAndMessages(row)
        main.postAtTime({
            if (row.request != null && row in rows) {
                row.unheard = true
                changed()
            }
        }, row, SystemClock.uptimeMillis() + UNHEARD_MS)
    }

    fun save(out: Bundle) {
        val list = JSONArray()
        rows.forEach { row ->
            list.put(
                JSONObject()
                    .put("ticket", row.ticket)
                    .put("id", row.id)
                    .put("name", row.name)
                    .put("state", row.state)
                    .put("doneBytes", row.done)
                    .put("totalBytes", row.total ?: JSONObject.NULL)
                    .put("folder", row.folder)
                    .put("error", row.error ?: JSONObject.NULL)
                    .put("saved", row.saved ?: JSONObject.NULL)
                    .put("mime", row.mime ?: JSONObject.NULL)
                    .put("request", row.request ?: JSONObject.NULL),
            )
        }
        out.putString("downloads", list.toString())
    }

    fun restore(saved: Bundle) {
        val list = saved.getString("downloads")?.let { runCatching { JSONArray(it) }.getOrNull() } ?: return
        for (index in 0 until list.length()) {
            val item = list.optJSONObject(index) ?: continue
            val row = DownloadRow(item.optString("ticket"))
            if (row.ticket.isEmpty()) continue
            fill(row, item)
            row.request = item.optJSONObject("request")
            rows.add(row)
        }
        changed()
    }

    fun clear() {
        main.removeCallbacksAndMessages(null)
    }

    private fun fill(row: DownloadRow, item: JSONObject) {
        item.text("id")?.let { row.id = it }
        item.text("name")?.let { row.name = it }
        item.text("state")?.let { row.state = it }
        row.done = item.optDouble("doneBytes", 0.0).takeIf { !it.isNaN() } ?: 0.0
        row.total = if (item.isNull("totalBytes")) null else item.optDouble("totalBytes").takeIf { !it.isNaN() && it > 0 }
        item.text("folder")?.let { row.folder = it }
        row.error = item.text("error")
        row.saved = item.text("saved")
        row.mime = item.text("mime")
        row.waiting = item.text("waiting")
    }

    private fun drop(row: DownloadRow) {
        main.removeCallbacksAndMessages(row)
        rows.remove(row)
        views.remove(row.ticket)
    }

    private fun toast(text: String) {
        Toast.makeText(activity, text, Toast.LENGTH_SHORT).show()
    }

    /** The button and the open panel follow the rows. */
    private fun changed() {
        ring.fraction = DownloadWords.overall(rows)
        val running = rows.count(DownloadWords::running)
        badge.visibility = if (running > 0) View.VISIBLE else View.GONE
        badge.show(if (running > 9) "9+" else running.toString())
        button.contentDescription = if (running > 0) "Downloads, $running running" else "Downloads"
        renderPanel()
    }

    // ---- The panel ----

    private fun label(size: Float, bold: Boolean = false): TextView = TextView(activity).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        if (bold) typeface = Typeface.DEFAULT_BOLD
        maxLines = 1
        ellipsize = TextUtils.TruncateAt.END
    }

    private fun showPanel() {
        val panel = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            background = rounded(PANEL, activity.dp(20).toFloat())
            setPadding(activity.dp(16), activity.dp(12), activity.dp(8), activity.dp(12))
            isClickable = true
        }
        val head = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        head.addView(label(17f, bold = true).apply { text = "Downloads" }, LinearLayout.LayoutParams(0, -2, 1f))
        head.addView(activity.pill("Clear finished") {
            rows.filter { !DownloadWords.running(it) }.forEach(::drop)
            changed()
        })
        head.addView(activity.iconButton(R.drawable.raygun_ic_close, "Close", size = 40) { closePanel() })
        panel.addView(head)
        val column = LinearLayout(activity).apply { orientation = LinearLayout.VERTICAL }
        val scroll = object : ScrollView(activity) {
            override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
                val most = (resources.displayMetrics.heightPixels * 0.6f).toInt()
                super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(most, MeasureSpec.AT_MOST))
            }
        }
        scroll.addView(column)
        panel.addView(scroll, LinearLayout.LayoutParams(-1, -2).apply { topMargin = activity.dp(8) })
        val empty = label(14f).apply {
            text = "Files you download from this site show here"
            alpha = 0.7f
            maxLines = 2
            setPadding(0, activity.dp(12), activity.dp(8), activity.dp(16))
        }
        panel.addView(empty)
        host(panel)
        panelRows = column
        panelEmpty = empty
        views.clear()
        renderPanel()
    }

    /** The host closed the panel (Back, a tap outside). */
    fun panelClosed() {
        panelRows = null
        panelEmpty = null
    }

    private fun closePanel() {
        close()
        panelClosed()
    }

    private fun renderPanel() {
        val column = panelRows ?: return
        val wanted = rows.map { row -> views.getOrPut(row.ticket) { build(row) }.root }
        val current = (0 until column.childCount).map { column.getChildAt(it) }
        if (current != wanted) {
            column.removeAllViews()
            wanted.forEach { column.addView(it, LinearLayout.LayoutParams(-1, -2)) }
        }
        rows.forEach { render(it) }
        panelEmpty?.visibility = if (rows.isEmpty()) View.VISIBLE else View.GONE
    }

    private fun build(row: DownloadRow): RowViews {
        val name = label(15f, bold = true).apply { ellipsize = TextUtils.TruncateAt.MIDDLE }
        val status = label(13f).apply {
            alpha = 0.75f
            maxLines = 2
        }
        val progress = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 1000
            progressTintList = ColorStateList.valueOf(ACCENT)
            indeterminateTintList = ColorStateList.valueOf(ACCENT)
        }
        val actions = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.END or Gravity.CENTER_VERTICAL
        }
        val box = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, activity.dp(10), activity.dp(8), activity.dp(6))
            addView(name)
            addView(status)
            addView(progress, LinearLayout.LayoutParams(-1, activity.dp(4)).apply { topMargin = activity.dp(6) })
            addView(actions, LinearLayout.LayoutParams(-1, -2).apply { topMargin = activity.dp(4) })
        }
        return RowViews(box, name, status, progress, actions)
    }

    private fun size(bytes: Double): String = Formatter.formatShortFileSize(activity, bytes.toLong())

    /** What a row's buttons are now: (label, what it does). */
    private fun actionsOf(row: DownloadRow): List<Pair<String, () -> Unit>> = when {
        row.unheard -> listOf("Open Raygun" to ::openRaygun, "Remove" to { remove(row) })
        row.state == "done" -> listOfNotNull(
            if (row.saved != null) "Open" to { open(row) } else null,
            if (row.saved != null) "Show in folder" to { showFolder(row) } else null,
            "Remove" to { remove(row) },
        )
        row.state == "failed" -> listOfNotNull(
            if (row.id.isNotEmpty()) "Retry" to { retry(row) } else null,
            "Remove" to { remove(row) },
        )
        DownloadWords.running(row) && row.id.isNotEmpty() -> listOf("Cancel" to { cancel(row) })
        else -> emptyList()
    }

    private fun render(row: DownloadRow) {
        val shown = views[row.ticket] ?: return
        shown.name.show(row.name.ifBlank { "Download" })
        shown.status.show(DownloadWords.status(row, ::size))
        val running = DownloadWords.running(row)
        val total = row.total
        shown.progress.visibility = if (running) View.VISIBLE else View.GONE
        shown.progress.isIndeterminate = running && (total == null || row.state == "queued")
        if (total != null) shown.progress.progress = (row.done / total * 1000).toInt().coerceIn(0, 1000)
        val actions = actionsOf(row)
        val key = actions.joinToString("|") { it.first }
        if (key != shown.shownActions) {
            shown.shownActions = key
            shown.actions.removeAllViews()
            actions.forEachIndexed { index, (text, act) ->
                shown.actions.addView(activity.pill(text, filled = index == 0) { act() }.apply {
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    setPadding(activity.dp(12), activity.dp(6), activity.dp(12), activity.dp(6))
                    contentDescription = "$text ${row.name}"
                    if (index != 0) background = rounded(0x22FFFFFF, activity.dp(16).toFloat())
                })
            }
            shown.actions.visibility = if (actions.isEmpty()) View.GONE else View.VISIBLE
        }
    }

    private fun cancel(row: DownloadRow) {
        ask(JSONObject().put("action", "cancel").put("ticket", row.ticket))
    }

    private fun retry(row: DownloadRow) {
        ask(JSONObject().put("action", "retry").put("ticket", row.ticket))
        row.state = "queued"
        row.error = null
        changed()
    }

    private fun remove(row: DownloadRow) {
        drop(row)
        changed()
    }

    private fun openRaygun() {
        val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName) ?: return
        runCatching { activity.startActivity(launch) }
    }

    /** Opens the saved file in the app Android picks (the same as Raygun's own Open). */
    private fun open(row: DownloadRow) {
        val saved = row.saved ?: return
        try {
            activity.startActivity(Storage(activity).view(Uri.parse(saved), row.mime ?: ""))
        } catch (error: ActivityNotFoundException) {
            toast("No app on this phone opens this kind of file")
        } catch (error: Exception) {
            toast("The file didn't open")
        }
    }

    /** The folder the file was saved in: a picked folder's own view, else the system's Downloads. */
    private fun showFolder(row: DownloadRow) {
        val saved = row.saved ?: return
        try {
            activity.startActivity(Storage(activity).folderOf(Uri.parse(saved)))
        } catch (error: Exception) {
            toast("No app on this phone shows this folder")
        }
    }
}

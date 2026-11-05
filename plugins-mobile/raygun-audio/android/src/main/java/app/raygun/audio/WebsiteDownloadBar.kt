package app.raygun.audio

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Typeface
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
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject

/**
 * The downloads this browser started, in a slim bar under the page: the file, how far it is,
 * then Open once it's saved (Retry when it failed, Cancel while it runs). Raygun downloads them
 * in its own process, which says how they go in a package-only broadcast
 * ([WebsiteActivity.ACTION_DOWNLOADS], `{"downloads": [...]}`) while the browser is on screen
 * ([watch], [unwatch]). A finished row goes by itself after a while; the download notifications
 * and Raygun's Downloads → Files keep the rest.
 */
internal class DownloadBar(
    private val activity: Activity,
    /** Sends a question about a download to Raygun (`{"action", "ticket"}`). */
    private val ask: (JSONObject) -> Unit,
    private val notice: (String) -> Unit,
) {
    private companion object {
        /** Rows shown at most, the newest. */
        const val MAX_ROWS = 3
        /** How long a finished row stays, ms. */
        const val DONE_STAYS_MS = 12_000L
    }

    private class Row(val ticket: String) {
        var id = ""
        var name = ""
        var state = "queued"
        var done = 0.0
        var total: Double? = null
        var folder = ""
        var error: String? = null
        var saved: String? = null
        var mime: String? = null
        var views: RowViews? = null
    }

    private class RowViews(
        val root: View,
        val name: TextView,
        val status: TextView,
        val action: TextView,
        val progress: ProgressBar,
    )

    private val main = Handler(Looper.getMainLooper())

    /** Newest first. */
    private val rows = mutableListOf<Row>()

    val root: LinearLayout = LinearLayout(activity).apply {
        orientation = LinearLayout.VERTICAL
        setBackgroundColor(PANEL)
        visibility = View.GONE
    }

    /** A download handed to Raygun just now: shown waiting until Raygun says more. */
    fun add(ticket: String, name: String, folder: String) {
        val row = Row(ticket).apply {
            this.name = name
            this.folder = folder
        }
        rows.add(0, row)
        render(row)
        layout()
    }

    /** Raygun's word on some of the downloads (`{"downloads": [...]}`); others' are ignored. */
    fun update(json: String) {
        val list = runCatching { JSONObject(json).optJSONArray("downloads") }.getOrNull() ?: return
        var changed = false
        for (index in 0 until list.length()) {
            val item = list.optJSONObject(index) ?: continue
            val row = rows.firstOrNull { it.ticket == item.optString("ticket") } ?: continue
            if (item.optString("state") == "cancelled") {
                drop(row)
                changed = true
                continue
            }
            fill(row, item)
            render(row)
        }
        if (changed) layout()
    }

    /**
     * The browser is on screen: Raygun says how the downloads still on their way stand (it may
     * have missed changes while away) and tells every change from now on.
     */
    fun watch() {
        val waiting = rows.filter { it.state == "queued" || it.state == "downloading" }.map { it.ticket }
        ask(JSONObject().put("action", "watch").put("tickets", JSONArray(waiting)))
    }

    /** The browser left the screen: Raygun stops telling (its process may be frozen meanwhile). */
    fun unwatch() {
        ask(JSONObject().put("action", "unwatch"))
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
                    .put("mime", row.mime ?: JSONObject.NULL),
            )
        }
        out.putString("downloads", list.toString())
    }

    fun restore(saved: Bundle) {
        val list = saved.getString("downloads")?.let { runCatching { JSONArray(it) }.getOrNull() } ?: return
        for (index in 0 until list.length()) {
            val item = list.optJSONObject(index) ?: continue
            val row = Row(item.optString("ticket"))
            if (row.ticket.isEmpty()) continue
            fill(row, item)
            rows.add(row)
            render(row)
        }
        layout()
    }

    fun clear() {
        main.removeCallbacksAndMessages(null)
    }

    private fun fill(row: Row, item: JSONObject) {
        item.text("id")?.let { row.id = it }
        item.text("name")?.let { row.name = it }
        item.text("state")?.let { row.state = it }
        row.done = item.optDouble("doneBytes", 0.0).takeIf { !it.isNaN() } ?: 0.0
        row.total = if (item.isNull("totalBytes")) null else item.optDouble("totalBytes").takeIf { !it.isNaN() && it > 0 }
        item.text("folder")?.let { row.folder = it }
        row.error = item.text("error")
        row.saved = item.text("saved")
        row.mime = item.text("mime")
        main.removeCallbacksAndMessages(row)
        if (row.state == "done") {
            main.postAtTime({
                if (row.state == "done" && row in rows) {
                    drop(row)
                    layout()
                }
            }, row, SystemClock.uptimeMillis() + DONE_STAYS_MS)
        }
    }

    private fun drop(row: Row) {
        main.removeCallbacksAndMessages(row)
        rows.remove(row)
    }

    private fun layout() {
        root.removeAllViews()
        rows.take(MAX_ROWS).forEachIndexed { index, row ->
            val views = row.views ?: build(row)
            (views.root.parent as? ViewGroup)?.removeView(views.root)
            if (index > 0) {
                root.addView(View(activity).apply { setBackgroundColor(0x1FFFFFFF) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1))
            }
            root.addView(views.root, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        root.visibility = if (rows.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun label(size: Float, bold: Boolean): TextView = TextView(activity).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        if (bold) typeface = Typeface.DEFAULT_BOLD
        maxLines = 1
    }

    private fun build(row: Row): RowViews {
        val name = label(14f, bold = true).apply { ellipsize = TextUtils.TruncateAt.MIDDLE }
        val status = label(12f, bold = false).apply {
            ellipsize = TextUtils.TruncateAt.END
            alpha = 0.75f
        }
        val texts = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(activity.dp(10), 0, activity.dp(6), 0)
            addView(name)
            addView(status)
        }
        val action = activity.pill("", filled = true) { act(row) }.apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            setPadding(activity.dp(12), activity.dp(6), activity.dp(12), activity.dp(6))
        }
        val line = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = activity.dp(52)
            setPadding(activity.dp(12), activity.dp(4), activity.dp(2), activity.dp(4))
            addView(ImageView(activity).apply {
                setImageResource(R.drawable.raygun_ic_download)
                setColorFilter(Color.WHITE)
                alpha = 0.9f
            }, LinearLayout.LayoutParams(activity.dp(20), activity.dp(20)))
            addView(texts, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            addView(action)
            addView(activity.iconButton(R.drawable.raygun_ic_close, "Hide", size = 40) {
                drop(row)
                layout()
            })
        }
        val progress = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 1000
            progressTintList = ColorStateList.valueOf(ACCENT)
            indeterminateTintList = ColorStateList.valueOf(ACCENT)
        }
        val box = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            addView(line, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
            addView(progress, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, activity.dp(3)))
        }
        return RowViews(box, name, status, action, progress).also { row.views = it }
    }

    private fun size(bytes: Double): String = Formatter.formatShortFileSize(activity, bytes.toLong())

    /** What the row says now, and what its button does. */
    private fun render(row: Row) {
        val views = row.views ?: build(row)
        views.name.show(row.name.ifBlank { "Download" })
        val total = row.total
        val status = when (row.state) {
            "queued" -> if (row.folder.isNotBlank()) "Waiting · to ${row.folder}" else "Waiting"
            "downloading" -> when {
                total != null -> "${percent(row.done, total)}% · ${size(row.done)} of ${size(total)}"
                row.done > 0 -> size(row.done)
                else -> "Starting…"
            }
            "done" -> listOfNotNull(
                if (row.folder.isNotBlank()) "Saved in ${row.folder}" else "Saved",
                total?.let(::size),
            ).joinToString(" · ")
            "failed" -> row.error ?: "Didn't download"
            else -> ""
        }
        views.status.show(status)
        val button = when (row.state) {
            "done" -> if (row.saved != null) "Open" else null
            "failed" -> if (row.id.isNotEmpty()) "Retry" else null
            "queued", "downloading" -> if (row.id.isNotEmpty()) "Cancel" else null
            else -> null
        }
        views.action.visibility = if (button == null) View.GONE else View.VISIBLE
        button?.let(views.action::show)
        views.action.contentDescription = button?.let { "$it ${row.name}" }
        val running = row.state == "queued" || row.state == "downloading"
        views.progress.visibility = if (running) View.VISIBLE else View.GONE
        views.progress.isIndeterminate = running && (total == null || row.state == "queued")
        if (total != null) views.progress.progress = (row.done / total * 1000).toInt().coerceIn(0, 1000)
    }

    private fun percent(done: Double, total: Double): Int = (done / total * 100).toInt().coerceIn(0, 100)

    private fun act(row: Row) {
        when (row.state) {
            "done" -> open(row)
            "failed" -> {
                ask(JSONObject().put("action", "retry").put("ticket", row.ticket))
                row.state = "queued"
                row.error = null
                render(row)
            }
            "queued", "downloading" -> ask(JSONObject().put("action", "cancel").put("ticket", row.ticket))
        }
    }

    /** Opens the saved file in the app Android picks (the same as Raygun's own Open). */
    private fun open(row: Row) {
        val saved = row.saved ?: return
        try {
            activity.startActivity(Storage(activity).view(Uri.parse(saved), row.mime ?: ""))
        } catch (error: ActivityNotFoundException) {
            notice("No app on this phone opens this kind of file")
        } catch (error: Exception) {
            notice("The file didn't open")
        }
    }
}

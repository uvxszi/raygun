package app.raygun.audio

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.PopupWindow
import android.widget.ScrollView
import android.widget.TextView

/** One open page in the built-in browser: its WebView, and what the bar shows for it. */
internal class BrowserTab(val id: Int, val webView: WebView, var opener: Int?) {
    var title = ""
    /** The page's address as last known (before the WebView has one, the one asked for). */
    var address = ""
    var progress = 100
    /** The site of the page shown: its own requests are never blocked. Read off the main thread. */
    @Volatile
    var pageSite = ""
    /** A jump to another site held while the tab was out of sight, said when it's shown. */
    var heldJump: Uri? = null

    val shownAddress: String get() = webView.url ?: address

    /** The host it's on, for the tab list. */
    val host: String get() = Uri.parse(shownAddress).host ?: shownAddress
}

/** What a new window a page asks for becomes. */
internal enum class NewWindow {
    /** A tab: the person tapped a link meant for one. */
    TAB,

    /** A tab once the person says so: a tapped link to another site while popups are blocked. */
    HELD_TAB,

    /** Its page opens in the tab that asked. */
    HERE,

    /** A popup, blocked (its notice opens it in the tab). */
    BLOCKED,
}

/**
 * Only a link the person tapped makes a tab: `tappedLink` says the window is for exactly the
 * address of the link under their finger. Popups, windows opened without a tap and windows a
 * script opens on a tap never do: they're blocked, or open in the tab that asked when popups are
 * allowed or the tap stays on the site.
 */
internal fun newWindowAction(
    blockPopups: Boolean,
    userGesture: Boolean,
    tappedLink: Boolean,
    sameSite: Boolean,
): NewWindow = when {
    userGesture && tappedLink -> if (sameSite || !blockPopups) NewWindow.TAB else NewWindow.HELD_TAB
    !blockPopups -> NewWindow.HERE
    userGesture && sameSite -> NewWindow.HERE
    else -> NewWindow.BLOCKED
}

/** Whether two addresses are the same page: scheme and host in any case, `/` for no path. */
internal fun sameAddress(first: String?, second: String?): Boolean {
    if (first.isNullOrBlank() || second.isNullOrBlank()) return false
    if (first.trim() == second.trim()) return true
    val one = Uri.parse(first.trim())
    val other = Uri.parse(second.trim())
    if (one.isOpaque || other.isOpaque) return false
    fun path(uri: Uri) = uri.encodedPath?.ifEmpty { "/" } ?: "/"
    return one.scheme.equals(other.scheme, ignoreCase = true) &&
        one.host.equals(other.host, ignoreCase = true) &&
        one.port == other.port &&
        path(one) == path(other) &&
        one.encodedQuery == other.encodedQuery &&
        one.encodedFragment.orEmpty() == other.encodedFragment.orEmpty()
}

/**
 * The Referer a link's tab opens with, as browsers send it by default: the whole address within
 * a site, only the origin to another, nothing from https to http or from anything but a web page.
 */
internal fun refererFor(from: String?, to: Uri, sameSite: Boolean): String? {
    val page = from?.let(Uri::parse) ?: return null
    val scheme = page.scheme?.lowercase()
    if ((scheme != "https" && scheme != "http") || page.host.isNullOrEmpty()) return null
    if (scheme == "https" && to.scheme.equals("http", ignoreCase = true)) return null
    if (sameSite) return from.substringBefore('#')
    val port = if (page.port != -1) ":${page.port}" else ""
    return "$scheme://${page.host}$port/"
}

/** The bar's tab button: how many tabs are open, in a rounded square as browsers draw it. */
internal fun Context.tabCountButton(onClick: () -> Unit): Pair<FrameLayout, TextView> {
    val count = TextView(this).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
        typeface = Typeface.DEFAULT_BOLD
        gravity = Gravity.CENTER
        includeFontPadding = false
        background = GradientDrawable().apply {
            setStroke(dp(2), Color.WHITE)
            cornerRadius = dp(5).toFloat()
        }
    }
    val button = FrameLayout(this).apply {
        val ripple = TypedValue()
        theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, ripple, true)
        setBackgroundResource(ripple.resourceId)
        isClickable = true
        isFocusable = true
        setOnClickListener { onClick() }
        addView(count, FrameLayout.LayoutParams(dp(22), dp(22), Gravity.CENTER))
        layoutParams = LinearLayout.LayoutParams(dp(44), dp(44))
    }
    return button to count
}

/**
 * The open tabs in a small list under the tab button: each one's title and site, a cross to
 * close it and a tap to switch to it, then New tab. It scrolls past a few rows and closes on a
 * tap outside or Back.
 */
internal class TabListPopup(
    private val context: Context,
    private val tabs: () -> List<BrowserTab>,
    private val current: () -> BrowserTab,
    private val onPick: (BrowserTab) -> Unit,
    private val onClose: (BrowserTab) -> Unit,
    private val onNew: () -> Unit,
) {
    private val rows = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private val scroll = object : ScrollView(context) {
        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(maxHeight(), MeasureSpec.AT_MOST))
        }
    }.apply {
        isVerticalScrollBarEnabled = true
        addView(rows)
    }
    private val window: PopupWindow
    var onDismiss: (() -> Unit)? = null

    init {
        val panel = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = rounded(PANEL, context.dp(14).toFloat())
            setPadding(0, context.dp(6), 0, context.dp(6))
            addView(scroll, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
            addView(View(context).apply { setBackgroundColor(0x22FFFFFF) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 1).apply {
                topMargin = context.dp(4)
                bottomMargin = context.dp(4)
            })
            addView(newTabRow(), LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        val width = minOf(context.resources.displayMetrics.widthPixels - context.dp(16), context.dp(320))
        window = PopupWindow(panel, width, ViewGroup.LayoutParams.WRAP_CONTENT, true).apply {
            elevation = context.dp(12).toFloat()
            setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            isOutsideTouchable = true
            setOnDismissListener { onDismiss?.invoke() }
        }
        fill()
    }

    /** About six rows, or half the screen when that's less. */
    private fun maxHeight(): Int =
        minOf((context.resources.displayMetrics.heightPixels * 0.5f).toInt(), context.dp(56 * 6))

    fun show(anchor: View) {
        window.showAsDropDown(anchor, 0, 0, Gravity.END)
        val index = tabs().indexOf(current())
        scroll.post {
            rows.getChildAt(index)?.let { row -> scroll.scrollTo(0, (row.top - context.dp(8)).coerceAtLeast(0)) }
        }
    }

    fun dismiss() {
        if (window.isShowing) window.dismiss()
    }

    /** After tabs open, close or change their titles; with one tab left there's nothing to list. */
    fun refresh() {
        if (tabs().size <= 1) {
            dismiss()
            return
        }
        fill()
    }

    private fun fill() {
        rows.removeAllViews()
        val shown = current()
        for (tab in tabs()) {
            rows.addView(row(tab, tab === shown), LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
    }

    private fun text(size: Float, bold: Boolean): TextView = TextView(context).apply {
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        if (bold) typeface = Typeface.DEFAULT_BOLD
        maxLines = 1
        ellipsize = TextUtils.TruncateAt.END
    }

    private fun row(tab: BrowserTab, selected: Boolean): View {
        val title = tab.title.ifBlank { tab.host.ifBlank { "New tab" } }
        val line = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = context.dp(52)
            setPadding(context.dp(14), context.dp(2), context.dp(4), context.dp(2))
            if (selected) {
                setBackgroundColor(0x1FFFFFFF)
            } else {
                val ripple = TypedValue()
                context.theme.resolveAttribute(android.R.attr.selectableItemBackground, ripple, true)
                setBackgroundResource(ripple.resourceId)
            }
            isClickable = true
            isFocusable = true
            contentDescription = if (selected) "$title, shown" else title
            setOnClickListener {
                dismiss()
                onPick(tab)
            }
        }
        val texts = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
        texts.addView(text(14f, bold = true).apply {
            text = title
            if (selected) setTextColor(ACCENT)
        })
        texts.addView(text(12f, bold = false).apply {
            text = tab.host
            alpha = 0.7f
        })
        line.addView(texts, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        line.addView(context.iconButton(R.drawable.raygun_ic_close, "Close $title", size = 40) { onClose(tab) })
        return line
    }

    private fun newTabRow(): View = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = context.dp(48)
        setPadding(context.dp(14), 0, context.dp(14), 0)
        val ripple = TypedValue()
        context.theme.resolveAttribute(android.R.attr.selectableItemBackground, ripple, true)
        setBackgroundResource(ripple.resourceId)
        isClickable = true
        isFocusable = true
        addView(ImageView(context).apply {
            setImageResource(R.drawable.raygun_ic_add)
            setColorFilter(Color.WHITE)
        }, LinearLayout.LayoutParams(context.dp(20), context.dp(20)))
        addView(text(14f, bold = true).apply {
            text = "New tab"
            setPadding(context.dp(12), 0, 0, 0)
        })
        setOnClickListener {
            dismiss()
            onNew()
        }
    }
}

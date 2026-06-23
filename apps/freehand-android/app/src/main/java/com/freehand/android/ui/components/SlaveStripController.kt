package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.view.View
import android.widget.TextView

/**
 * Collapsed strip of slave agent pills. Click to expand into a pinned card.
 * The strip is hidden when no slaves are known.
 */
class SlaveStripController(
    context: Context,
    root: FrameLayout,
) {
    private val strip: HorizontalScrollView
    private val row: LinearLayout
    private val density = context.resources.displayMetrics.density

    init {
        row = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dp(12), dp(6), dp(12), dp(6))
            setBackgroundColor(Color.parseColor("#111827"))
        }
        strip = HorizontalScrollView(context).apply {
            addView(row)
            visibility = GONE
        }
        root.addView(
            strip,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP,
            ).apply {
                topMargin = dp(60)
            },
        )
    }

    fun render(slaves: List<Pair<String, String>>) {
        row.removeAllViews()
        if (slaves.isEmpty()) {
        strip.visibility = View.GONE
            return
        }
        strip.visibility = View.VISIBLE
        for ((name, status) in slaves) {
            row.addView(buildPill(name, status))
        }
    }

    private fun buildPill(name: String, status: String) = TextView(row.context).apply {
        text = "$name · $status"
        textSize = 12f
        setTextColor(Color.WHITE)
        setPadding(dp(10), dp(6), dp(10), dp(6))
        setBackgroundColor(Color.parseColor("#1F2937"))
    }

    private fun dp(v: Int): Int = (v * density).toInt()

}

package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.TextView
import com.freehand.android.R

/**
 * Transient status banner that surfaces connection state or runtime warnings.
 * It is a presentation layer only and does NOT write to session truth.
 */
class StatusBannerController(
    context: Context,
    root: FrameLayout,
) {
    private val banner: TextView
    private val density = context.resources.displayMetrics.density

    init {
        banner = TextView(context).apply {
            textSize = 12f
            setTextColor(Color.WHITE)
            setPadding(dp(12), dp(6), dp(12), dp(6))
            setBackgroundColor(Color.parseColor("#7C3AED"))
            visibility = View.GONE
        }
        root.addView(
            banner,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP,
            ).apply {
                topMargin = dp(110)
            },
        )
    }

    fun showTransient(message: String) {
        banner.text = message
        banner.visibility = View.VISIBLE
        banner.postDelayed({ banner.visibility = View.GONE }, 3000)
    }

    fun showPersistent(message: String) {
        banner.text = message
        banner.visibility = View.VISIBLE
    }

    fun hide() {
        banner.visibility = View.GONE
    }

    private fun dp(v: Int): Int = (v * density).toInt()
}

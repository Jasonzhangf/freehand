package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.TextView
import com.freehand.android.R

/**
 * Native shell banner for blocking connection/configuration problems only.
 * Once the remote WebUI is active, user-visible lifecycle/status should stay in WebUI.
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
            setBackgroundColor(Color.parseColor("#1F2937"))
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

    fun showTransient(@Suppress("UNUSED_PARAMETER") message: String) {
        banner.visibility = View.GONE
    }

    fun showPersistent(message: String) {
        banner.text = message
        banner.visibility = View.VISIBLE
    }

    /** Show turn-level progress (thinking / tool execution / streaming).
     *  Auto-hides when state is terminal or idle. */
    fun showTurnProgress(turnState: String) {
        val message = when (turnState) {
            "idle", "waiting" -> null
            "done", "error", "blocked", "cancelled" -> null
            "thinking", "reasoning" -> "agent is thinking…"
            "running tools", "tool_executing" -> "using tool…"
            "streaming", "writing response" -> "writing response…"
            else -> turnState.takeIf { it.isNotBlank() }
        }
        if (message == null) {
            hide()
            return
        }
        banner.text = message
        banner.setBackgroundColor(Color.parseColor("#1F2937"))
        banner.visibility = View.VISIBLE
    }

    fun hide() {
        banner.visibility = View.GONE
    }

    private fun dp(v: Int): Int = (v * density).toInt()
}

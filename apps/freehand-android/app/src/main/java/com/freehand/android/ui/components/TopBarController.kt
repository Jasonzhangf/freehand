package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import com.freehand.android.R

class TopBarController(
    context: Context,
    root: FrameLayout,
    onMenuClick: () -> Unit,
) {
    private val bar: LinearLayout
    private val agentName: TextView
    private val agentStatus: TextView

    init {
        val density = context.resources.displayMetrics.density
        fun dp(v: Int) = (v * density).toInt()

        bar = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dp(12), dp(10), dp(12), dp(10))
            setBackgroundColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_VERTICAL
        }

        val menu = ImageButton(context).apply {
            setImageResource(R.drawable.ic_menu)
            setBackgroundColor(Color.TRANSPARENT)
            setOnClickListener { onMenuClick() }
        }
        bar.addView(menu, LinearLayout.LayoutParams(dp(40), dp(40)))

        val nameCol = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12), 0, 0, 0)
        }
        agentName = TextView(context).apply {
            textSize = 14f
            setTextColor(Color.WHITE)
            text = context.getString(R.string.drawer_title)
        }
        agentStatus = TextView(context).apply {
            textSize = 11f
            setTextColor(Color.parseColor("#94A3B8"))
            text = "idle"
        }
        nameCol.addView(agentName)
        nameCol.addView(agentStatus)
        bar.addView(
            nameCol,
            LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
        )

        root.addView(
            bar,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP,
            ),
        )
    }

    fun setAgent(name: String, status: String) {
        agentName.text = name
        agentStatus.text = status
    }

    fun root(): View = bar
}

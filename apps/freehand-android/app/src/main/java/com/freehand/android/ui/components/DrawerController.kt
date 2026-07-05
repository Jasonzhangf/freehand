package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import com.freehand.android.R
import com.freehand.android.data.HostConfig

/**
 * Right-slide drawer for low-frequency connection settings only.
 * No fake session switching, no demo actions, no reason/session mutation.
 */
class DrawerController(
    context: Context,
    root: FrameLayout,
    private val onHostChanged: (HostConfig) -> Unit,
    private val initialHost: HostConfig,
) {
    private val panel: LinearLayout
    private val density = context.resources.displayMetrics.density
    private var open = false

    init {
        panel = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(28), dp(20), dp(20))
            setBackgroundColor(Color.parseColor("#0F172A"))
            visibility = View.GONE
        }
        buildContents()
        root.addView(
            panel,
            FrameLayout.LayoutParams(
                dp(300),
                FrameLayout.LayoutParams.MATCH_PARENT,
                Gravity.END,
            ),
        )
    }

    private fun buildContents() {
        val title = TextView(panel.context).apply {
            text = panel.context.getString(R.string.drawer_title)
            textSize = 18f
            setTextColor(Color.WHITE)
        }
        panel.addView(title)
        panel.addView(spacer(8))

        panel.addView(sectionLabel(panel.context.getString(R.string.section_connection)))
        panel.addView(spacer(6))
        panel.addView(TextView(panel.context).apply {
            text = "profile: ${initialHost.profileId} · ${initialHost.mode}"
            textSize = 12f
            setTextColor(Color.parseColor("#94A3B8"))
        })
        panel.addView(spacer(8))
        val hostInput = EditText(panel.context).apply {
            inputType = InputType.TYPE_CLASS_TEXT
            setText(initialHost.host)
            setTextColor(Color.WHITE)
            setHintTextColor(Color.parseColor("#94A3B8"))
        }
        val portInput = EditText(panel.context).apply {
            inputType = InputType.TYPE_CLASS_NUMBER
            setText(initialHost.port.toString())
            setTextColor(Color.WHITE)
            setHintTextColor(Color.parseColor("#94A3B8"))
        }
        panel.addView(hostInput)
        panel.addView(spacer(6))
        panel.addView(portInput)
        panel.addView(spacer(8))
        panel.addView(actionButton(panel.context.getString(R.string.action_save_connection)) {
            val newHost = HostConfig(
                host = hostInput.text.toString().ifBlank { initialHost.host },
                port = portInput.text.toString().toIntOrNull() ?: initialHost.port,
                profileId = initialHost.profileId,
                mode = initialHost.mode,
                adpPath = initialHost.adpPath,
                healthPath = initialHost.healthPath,
                commandPath = initialHost.commandPath,
                queryPath = initialHost.queryPath,
                subscribePath = initialHost.subscribePath,
            )
            onHostChanged(newHost)
            toggle()
        })
    }

    private fun sectionLabel(text: String) = TextView(panel.context).apply {
        this.text = text
        textSize = 12f
        setTextColor(Color.parseColor("#94A3B8"))
    }

    private fun actionButton(text: String, onClick: () -> Unit) = Button(panel.context).apply {
        this.text = text
        setOnClickListener { onClick() }
    }

    private fun spacer(height: Int) = View(panel.context).apply {
        layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, dp(height))
    }

    fun toggle() {
        open = !open
        panel.visibility = if (open) View.VISIBLE else View.GONE
    }

    fun open() {
        open = true
        panel.visibility = View.VISIBLE
    }

    fun close() {
        open = false
        panel.visibility = View.GONE
    }

    fun isOpen(): Boolean = open

    private fun dp(v: Int): Int = (v * density).toInt()
}

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
import com.freehand.android.data.HostStore

/**
 * Right-slide drawer for quick control: switch host, switch session, run a quick action.
 * The drawer only mutates local UI state; it never touches session/reason/provider truth.
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

        panel.addView(sectionLabel(panel.context.getString(R.string.section_agents)))
        panel.addView(spacer(6))
        panel.addView(actionButton(panel.context.getString(R.string.action_switch_main)) {
            toggle()
        })
        panel.addView(spacer(12))

        panel.addView(sectionLabel(panel.context.getString(R.string.section_sessions)))
        panel.addView(spacer(6))
        panel.addView(actionButton(panel.context.getString(R.string.action_new_session)) {
            toggle()
        })
        panel.addView(spacer(16))

        panel.addView(sectionLabel("Host"))
        panel.addView(spacer(6))
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
        panel.addView(actionButton("保存") {
            val newHost = HostConfig(
                host = hostInput.text.toString().ifBlank { HostStore.DEFAULT_HOST },
                port = portInput.text.toString().toIntOrNull() ?: HostStore.DEFAULT_PORT,
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

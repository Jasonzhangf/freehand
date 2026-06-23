package com.freehand.android.ui.components

import android.content.Context
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import com.freehand.android.R

/**
 * Native input bar at the bottom. User text is sent via the protocol-owned
 * command ingress (HTTP POST /ui/command). Local state stays in the bar only.
 */
class InputBarController(
    context: Context,
    root: FrameLayout,
    private val onSubmit: (String) -> Unit,
) {
    private val bar: LinearLayout
    private val input: EditText
    private val send: ImageButton

    init {
        val density = context.resources.displayMetrics.density
        fun dp(v: Int) = (v * density).toInt()

        bar = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dp(12), dp(8), dp(12), dp(8))
            setBackgroundColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_VERTICAL
        }

        input = EditText(context).apply {
            hint = context.getString(R.string.input_hint)
            setHintTextColor(Color.parseColor("#94A3B8"))
            setTextColor(Color.WHITE)
            textSize = 15f
            minLines = 1
            maxLines = 4
            imeOptions = EditorInfo.IME_ACTION_SEND
            setSingleLine(false)
            setBackgroundColor(Color.TRANSPARENT)
            setOnEditorActionListener { _, actionId, _ ->
                if (actionId == EditorInfo.IME_ACTION_SEND) {
                    submit()
                    true
                } else {
                    false
                }
            }
        }

        send = ImageButton(context).apply {
            setImageResource(R.drawable.ic_send)
            setBackgroundColor(Color.TRANSPARENT)
            setOnClickListener { submit() }
        }

        bar.addView(
            input,
            LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
        )
        bar.addView(send, LinearLayout.LayoutParams(dp(40), dp(40)))

        root.addView(
            bar,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM,
            ),
        )
    }

    private fun submit() {
        val text = input.text.toString().trim()
        if (text.isEmpty()) return
        onSubmit(text)
    }

    fun clear() {
        input.setText("")
    }

    fun markSendError(reason: String) {
        input.error = reason
    }

    fun root(): View = bar
}

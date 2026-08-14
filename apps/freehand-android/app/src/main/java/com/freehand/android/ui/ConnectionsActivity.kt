package com.freehand.android.ui

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import com.freehand.android.data.ClientConfig
import com.freehand.android.data.DaemonConnectionConfig
import com.freehand.android.data.DaemonConnectionConfigException
import com.freehand.android.data.DaemonConnectionProfile

/**
 * Native connection configuration page.
 *
 * Android owns the connection bootstrap surface: which daemon host/port the
 * canonical daemon WebUI should attach to. It lists saved daemon profiles,
 * allows add / edit / delete / switch, and persists everything to the
 * app-owned DaemonConnectionConfigStore. It must not render a second
 * conversation UI or settings drawer; those stay in the daemon WebUI.
 */
class ConnectionsActivity : AppCompatActivity() {

    private lateinit var listContainer: LinearLayout
    private lateinit var emptyState: TextView
    private var config: DaemonConnectionConfig? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(buildContentView())
        reloadProfiles()
    }

    private fun buildContentView(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.rgb(15, 23, 42))
        }
        root.addView(buildHeader(), LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT,
        ))
        val scroll = ScrollView(this).apply {
            isFillViewport = true
        }
        listContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(16), dp(16), dp(24))
        }
        emptyState = TextView(this).apply {
            text = "No configured daemons"
            setTextColor(Color.rgb(148, 163, 184))
            gravity = Gravity.CENTER
            textSize = 14f
            visibility = View.GONE
        }
        listContainer.addView(
            emptyState,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(120),
            ),
        )
        scroll.addView(
            listContainer,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        root.addView(
            scroll,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f,
            ),
        )
        return root
    }

    private fun buildHeader(): View {
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(12), dp(12), dp(12))
        }
        val back = Button(this).apply {
            text = "返回"
            setOnClickListener { finishWithResult() }
        }
        val title = TextView(this).apply {
            text = "Daemon 连接配置"
            setTextColor(Color.rgb(226, 232, 240))
            textSize = 18f
            gravity = Gravity.CENTER
        }
        val add = Button(this).apply {
            text = "添加"
            setOnClickListener { showHostForm(null) }
        }
        header.addView(
            back,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        header.addView(
            title,
            LinearLayout.LayoutParams(
                0,
                LinearLayout.LayoutParams.WRAP_CONTENT,
                1f,
            ),
        )
        header.addView(
            add,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        return header
    }

    private fun reloadProfiles() {
        val loaded = try {
            ClientConfig.load(applicationContext)
        } catch (error: DaemonConnectionConfigException) {
            showError("加载配置失败：${error.message}")
            return
        }
        config = loaded
        listContainer.removeAllViews()
        listContainer.addView(
            emptyState,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(120),
            ),
        )
        emptyState.visibility = if (loaded.profiles.isEmpty()) View.VISIBLE else View.GONE
        loaded.profiles.forEach { profile ->
            listContainer.addView(renderProfileRow(loaded, profile))
        }
    }

    private fun renderProfileRow(
        current: DaemonConnectionConfig,
        profile: DaemonConnectionProfile,
    ): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(14), dp(12), dp(14), dp(12))
            setBackgroundColor(Color.rgb(30, 41, 59))
        }
        val top = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val label = TextView(this).apply {
            text = profile.id
            setTextColor(Color.rgb(226, 232, 240))
            textSize = 15f
        }
        val endpoint = TextView(this).apply {
            text = "${profile.host}:${profile.port}"
            setTextColor(Color.rgb(148, 163, 184))
            textSize = 13f
        }
        val active = if (profile.id == current.activeProfile) {
            TextView(this).apply {
                text = "当前"
                setTextColor(Color.rgb(34, 197, 94))
                textSize = 12f
            }
        } else {
            null
        }
        top.addView(
            label,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        active?.let {
            top.addView(
                it,
                LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                ).apply { leftMargin = dp(8) },
            )
        }
        top.addView(
            endpoint,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { leftMargin = dp(8) },
        )

        val actions = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.END
        }
        if (profile.id != current.activeProfile) {
            actions.addView(
                actionButton("切换") { switchProfile(profile.id) },
                LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                ).apply { rightMargin = dp(8) },
            )
        }
        actions.addView(
            actionButton("编辑") { showHostForm(profile) },
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { rightMargin = dp(8) },
        )
        actions.addView(
            actionButton("删除") { removeProfile(profile.id) },
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )

        row.addView(
            top,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        row.addView(
            actions,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(10) },
        )
        return row
    }

    private fun actionButton(label: String, onClick: () -> Unit): Button =
        Button(this).apply {
            text = label
            setOnClickListener { onClick() }
        }

    private fun showHostForm(existing: DaemonConnectionProfile?) {
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24), dp(8), dp(24), dp(0))
        }
        val idInput = EditText(this).apply {
            hint = "名称 (e.g. tailscale-main)"
            setText(existing?.id ?: "")
            isEnabled = existing == null
            inputType = InputType.TYPE_CLASS_TEXT
        }
        val hostInput = EditText(this).apply {
            hint = "Daemon host (e.g. 100.66.1.82)"
            setText(existing?.host ?: "")
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
        }
        val portInput = EditText(this).apply {
            hint = "Port (e.g. 4042)"
            setText(existing?.port?.toString() ?: "")
            inputType = InputType.TYPE_CLASS_NUMBER
        }
        container.addView(
            idInput,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        container.addView(
            hostInput,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(12) },
        )
        container.addView(
            portInput,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(12) },
        )
        AlertDialog.Builder(this)
            .setTitle(if (existing == null) "添加 Daemon" else "编辑 Daemon")
            .setView(container)
            .setNegativeButton("取消", null)
            .setPositiveButton("保存") { _, _ ->
                saveProfile(
                    id = idInput.text.toString().trim(),
                    host = hostInput.text.toString().trim(),
                    port = portInput.text.toString().trim().toIntOrNull(),
                )
            }
            .show()
    }

    private fun saveProfile(id: String, host: String, port: Int?) {
        if (id.isEmpty() || host.isEmpty() || port == null) {
            showError("配置无效：名称、host、port 均不能为空")
            return
        }
        try {
            val store = ClientConfig.store(applicationContext)
            val updated = store.load().addOrReplaceProfile(
                DaemonConnectionProfile(
                    id = id,
                    mode = "tailscale",
                    host = host,
                    port = port,
                ),
            )
            store.write(updated)
            reloadProfiles()
        } catch (error: DaemonConnectionConfigException) {
            showError("保存失败：${error.message}")
        }
    }

    private fun switchProfile(id: String) {
        try {
            val store = ClientConfig.store(applicationContext)
            val updated = store.load().switchActiveProfile(id)
            store.write(updated)
            reloadProfiles()
        } catch (error: DaemonConnectionConfigException) {
            showError("切换失败：${error.message}")
        }
    }

    private fun removeProfile(id: String) {
        try {
            val store = ClientConfig.store(applicationContext)
            val updated = store.load().removeProfile(id)
            store.write(updated)
            reloadProfiles()
        } catch (error: DaemonConnectionConfigException) {
            showError("删除失败：${error.message}")
        }
    }

    private fun finishWithResult() {
        setResult(RESULT_OK, Intent().putExtra(EXTRA_CONFIG_CHANGED, true))
        finish()
    }

    private fun showError(message: String) {
        AlertDialog.Builder(this)
            .setTitle("Freehand")
            .setMessage(message)
            .setPositiveButton("确定", null)
            .show()
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    companion object {
        const val EXTRA_CONFIG_CHANGED = "freehand_connections_config_changed"

        fun changedIntent(context: Context): Intent {
            val intent = Intent(context, ConnectionsActivity::class.java)
            return intent
        }
    }
}

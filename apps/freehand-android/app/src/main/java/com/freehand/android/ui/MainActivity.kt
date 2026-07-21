package com.freehand.android.ui

import android.animation.AnimatorSet
import android.animation.ObjectAnimator
import android.content.Intent
import android.content.res.ColorStateList
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.Gravity
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.data.ClientConfig
import com.freehand.android.data.DaemonConnectionConfigException
import org.json.JSONArray
import org.json.JSONObject

/**
 * Thin Android host for the canonical daemon WebUI.
 *
 * Android owns only platform integration: endpoint bootstrap, WebView hosting,
 * window insets, and the system file picker. Conversation, settings, status,
 * lifecycle, and error rendering are owned by the daemon-hosted WebUI.
 */
class MainActivity : AppCompatActivity() {
    private lateinit var webView: WebView
    private lateinit var startupOverlay: FrameLayout
    private lateinit var startupStatus: TextView
    private var startupAnimator: AnimatorSet? = null
    private var fileChooserCallback: ValueCallback<Array<Uri>>? = null
    private var nativeAttachmentKind: String? = null
    private lateinit var fileChooserLauncher: ActivityResultLauncher<Intent>
    private lateinit var apkUpdater: AndroidApkUpdater
    private var lastApkUpdateStatus: ApkUpdateStatus? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val host = loadHostConfigFromStartupIntent()
        apkUpdater = AndroidApkUpdater(applicationContext, host)

        fileChooserLauncher = registerForActivityResult(
            ActivityResultContracts.StartActivityForResult(),
        ) { result ->
            val attachmentKind = nativeAttachmentKind
            if (attachmentKind != null) {
                nativeAttachmentKind = null
                injectAndroidAttachmentSelection(attachmentKind, result.data)
                return@registerForActivityResult
            }
            val callback = fileChooserCallback ?: return@registerForActivityResult
            fileChooserCallback = null
            callback.onReceiveValue(
                WebChromeClient.FileChooserParams.parseResult(result.resultCode, result.data),
            )
        }

        val root = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            )
        }

        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            settings.cacheMode = WebSettings.LOAD_DEFAULT
            settings.useWideViewPort = false
            settings.loadWithOverviewMode = false
            settings.textZoom = 100
            addJavascriptInterface(AndroidFilePickerBridge(), "FreehandAndroidFilePicker")
            addJavascriptInterface(AndroidApkUpdateBridge(), "FreehandAndroidApkUpdate")
            webChromeClient = AndroidWebChromeClient()
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    reportCanonicalWebUiLayout(view)
                    lastApkUpdateStatus?.let { emitAndroidApkUpdateStatus(it) }
                }

                override fun onReceivedError(
                    view: WebView?,
                    request: WebResourceRequest?,
                    error: WebResourceError?,
                ) {
                    super.onReceivedError(view, request, error)
                    if (request?.isForMainFrame == true) {
                        showStartupError(error?.description?.toString() ?: "WebUI load failed")
                    }
                }
            }
        }
        root.addView(
            webView,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        startupOverlay = buildStartupOverlay()
        root.addView(
            startupOverlay,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        applyInsets(root)
        setContentView(root)
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                handleAndroidBackPressed()
            }
        })
        startAndroidApkUpdateCheck()
        webView.loadUrl(host.webUiUrl)
    }

    private fun loadHostConfigFromStartupIntent() =
        try {
            val store = ClientConfig.store(applicationContext)
            val importLink = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.data?.toString()
            val config = if (!importLink.isNullOrBlank()) {
                store.importBootstrapLink(importLink)
            } else {
                store.load()
            }
            config.activeHostConfig()
        } catch (error: DaemonConnectionConfigException) {
            throw error
        }

    private fun handleAndroidBackPressed() {
        if (!::webView.isInitialized || isFinishing) {
            finish()
            return
        }
        webView.evaluateJavascript(
            "(" +
                "function(){" +
                "try{" +
                "return !!(window.__freehandHandleAndroidBack&&window.__freehandHandleAndroidBack());" +
                "}catch(error){return false;}" +
                "}" +
                ")()",
        ) { handled ->
            if (handled == "true") return@evaluateJavascript
            if (::webView.isInitialized && webView.canGoBack()) {
                webView.goBack()
            } else {
                finish()
            }
        }
    }

    private inner class AndroidWebChromeClient : WebChromeClient() {
        override fun onShowFileChooser(
            webView: WebView?,
            filePathCallback: ValueCallback<Array<Uri>>?,
            fileChooserParams: FileChooserParams?,
        ): Boolean {
            fileChooserCallback?.onReceiveValue(null)
            fileChooserCallback = filePathCallback
            val intent = try {
                fileChooserParams?.createIntent()
            } catch (error: Exception) {
                Log.e(LOG_TAG, "file picker request failed", error)
                fileChooserCallback?.onReceiveValue(null)
                fileChooserCallback = null
                return false
            }
            if (intent == null) {
                fileChooserCallback?.onReceiveValue(null)
                fileChooserCallback = null
                return false
            }
            return try {
                fileChooserLauncher.launch(intent)
                true
            } catch (error: Exception) {
                Log.e(LOG_TAG, "file picker unavailable", error)
                fileChooserCallback?.onReceiveValue(null)
                fileChooserCallback = null
                false
            }
        }
    }

    private inner class AndroidFilePickerBridge {
        @JavascriptInterface
        fun request(kind: String) {
            runOnUiThread { openAndroidAttachmentPicker(kind) }
        }
    }

    private inner class AndroidApkUpdateBridge {
        @JavascriptInterface
        fun check() {
            runOnUiThread { startAndroidApkUpdateCheck() }
        }
    }

    private fun startAndroidApkUpdateCheck() {
        if (!::apkUpdater.isInitialized) {
            recordAndroidApkUpdateStatus(
                ApkUpdateStatus(
                    phase = "failed",
                    message = "APK updater is not initialized",
                ),
            )
            return
        }
        apkUpdater.checkForUpdateAsync { status ->
            runOnUiThread { recordAndroidApkUpdateStatus(status) }
        }
    }

    private fun recordAndroidApkUpdateStatus(status: ApkUpdateStatus) {
        lastApkUpdateStatus = status
        emitAndroidApkUpdateStatus(status)
    }

    private fun emitAndroidApkUpdateStatus(status: ApkUpdateStatus) {
        if (!::webView.isInitialized || isFinishing) return
        val payload = JSONObject()
            .put("phase", status.phase)
            .put("message", status.message)
        status.versionCode?.let { payload.put("versionCode", it) }
        status.versionName?.let { payload.put("versionName", it) }
        status.apkUrl?.let { payload.put("apkUrl", it) }
        status.bytes?.let { payload.put("bytes", it) }
        webView.evaluateJavascript(
            "window.__freehandAndroidApkUpdateStatus && " +
                "window.__freehandAndroidApkUpdateStatus($payload);",
            null,
        )
    }

    private fun openAndroidAttachmentPicker(kind: String) {
        val normalizedKind = when (kind) {
            "image", "video", "file" -> kind
            else -> "file"
        }
        nativeAttachmentKind = normalizedKind
        fileChooserCallback?.onReceiveValue(null)
        fileChooserCallback = null
        val mimeType = when (normalizedKind) {
            "image" -> "image/*"
            "video" -> "video/*"
            else -> "*/*"
        }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = mimeType
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        }
        try {
            fileChooserLauncher.launch(intent)
        } catch (error: Exception) {
            nativeAttachmentKind = null
            Log.e(LOG_TAG, "attachment picker unavailable", error)
        }
    }

    private fun injectAndroidAttachmentSelection(kind: String, data: Intent?) {
        val files = JSONArray()
        selectedUris(data).forEach { uri ->
            files.put(
                JSONObject()
                    .put("name", displayName(uri))
                    .put("size", displaySize(uri))
                    .put("type", contentResolver.getType(uri) ?: "application/octet-stream")
                    .put("uri", uri.toString()),
            )
        }
        if (files.length() == 0) return
        webView.evaluateJavascript(
            "window.__freehandAndroidAttachmentSelected && " +
                "window.__freehandAndroidAttachmentSelected(${JSONObject.quote(kind)}, $files);",
            null,
        )
    }

    private fun selectedUris(data: Intent?): List<Uri> {
        val clipData = data?.clipData
        if (clipData != null && clipData.itemCount > 0) {
            return (0 until clipData.itemCount).mapNotNull { clipData.getItemAt(it).uri }
        }
        return data?.data?.let(::listOf) ?: emptyList()
    }

    private fun displayName(uri: Uri): String {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (index >= 0) {
                    val name = cursor.getString(index)
                    if (!name.isNullOrBlank()) return name
                }
            }
        }
        return uri.lastPathSegment ?: "attachment"
    }

    private fun displaySize(uri: Uri): Long {
        contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val index = cursor.getColumnIndex(OpenableColumns.SIZE)
                if (index >= 0 && !cursor.isNull(index)) return cursor.getLong(index)
            }
        }
        return -1L
    }

    private fun reportCanonicalWebUiLayout(view: WebView?) {
        view?.evaluateJavascript(
            "(" +
                "function(){" +
                "const shell=document.querySelector('[data-webui-shell=true]');" +
                "const shellStyle=shell?window.getComputedStyle(shell):null;" +
                "return {" +
                "webuiShell:!!shell," +
                "layoutClient:document.body.dataset.layoutClient||''," +
                "layoutShape:document.body.dataset.layoutShape||''," +
                "readyState:document.readyState||''," +
                "stylesheetCount:(document.styleSheets||[]).length," +
                "webuiCssApplied:!!(shellStyle&&shellStyle.display==='grid')," +
                "webuiJsReady:document.body.dataset.webuiJsReady==='true'," +
                "stylesheetHrefs:Array.from(document.styleSheets||[]).map(function(sheet){return sheet.href||'inline';}).slice(0,4)" +
                "};" +
                "}" +
                ")()",
        ) { value ->
            Log.i(WEBUI_LAYOUT_TAG, value ?: "null")
            val verdict = WebUiStartupGate.evaluate(value)
            if (verdict.ready) {
                dismissStartupOverlay()
            } else {
                showStartupError(verdict.status)
            }
        }
    }

    private fun buildStartupOverlay(): FrameLayout {
        val overlay = FrameLayout(this).apply {
            setBackgroundColor(Color.rgb(23, 23, 23))
            contentDescription = "Freehand is starting"
        }
        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
        }
        val logo = ImageView(this).apply {
            setImageResource(com.freehand.android.R.mipmap.ic_launcher)
            contentDescription = getString(com.freehand.android.R.string.app_name)
        }
        content.addView(
            logo,
            LinearLayout.LayoutParams(dp(76), dp(76)).apply { bottomMargin = dp(18) },
        )
        content.addView(
            TextView(this).apply {
                text = getString(com.freehand.android.R.string.app_name)
                setTextColor(Color.rgb(245, 245, 245))
                textSize = 24f
                gravity = Gravity.CENTER
            },
        )
        startupStatus = TextView(this).apply {
            text = "Connecting to workspace"
            setTextColor(Color.rgb(163, 163, 163))
            textSize = 13f
            gravity = Gravity.CENTER
        }
        content.addView(
            startupStatus,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(8) },
        )
        content.addView(
            ProgressBar(this).apply {
                isIndeterminate = true
                indeterminateTintList = ColorStateList.valueOf(Color.rgb(37, 99, 235))
            },
            LinearLayout.LayoutParams(dp(26), dp(26)).apply { topMargin = dp(20) },
        )
        overlay.addView(
            content,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER,
            ),
        )
        startupAnimator = AnimatorSet().apply {
            playTogether(
                ObjectAnimator.ofFloat(logo, View.SCALE_X, 1f, 1.06f),
                ObjectAnimator.ofFloat(logo, View.SCALE_Y, 1f, 1.06f),
                ObjectAnimator.ofFloat(logo, View.ALPHA, 0.72f, 1f),
            )
            duration = 900L
            childAnimations.forEach { animator ->
                if (animator is ObjectAnimator) {
                    animator.repeatCount = ObjectAnimator.INFINITE
                    animator.repeatMode = ObjectAnimator.REVERSE
                }
            }
            start()
        }
        return overlay
    }

    private fun dismissStartupOverlay() {
        if (!::startupOverlay.isInitialized || startupOverlay.parent == null) return
        startupStatus.text = "Workspace ready"
        startupOverlay.animate()
            .alpha(0f)
            .setDuration(180L)
            .withEndAction {
                startupAnimator?.cancel()
                startupAnimator = null
                (startupOverlay.parent as? FrameLayout)?.removeView(startupOverlay)
            }
            .start()
    }

    private fun showStartupError(message: String) {
        if (!::startupStatus.isInitialized || startupOverlay.parent == null) return
        startupStatus.text = message
        startupStatus.setTextColor(Color.rgb(203, 213, 225))
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    private fun applyInsets(root: FrameLayout) {
        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val navigation = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            root.setPadding(0, 0, 0, navigation)
            insets
        }
    }

    override fun onDestroy() {
        fileChooserCallback?.onReceiveValue(null)
        fileChooserCallback = null
        startupAnimator?.cancel()
        startupAnimator = null
        if (::webView.isInitialized) webView.destroy()
        super.onDestroy()
    }

    companion object {
        private const val LOG_TAG = "FreehandAndroid"
        private const val WEBUI_LAYOUT_TAG = "FreehandWebUiLayout"
    }
}

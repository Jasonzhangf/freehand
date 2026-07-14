package com.freehand.android.ui

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.data.ClientConfig
import com.freehand.android.data.HostConfig
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
    private var fileChooserCallback: ValueCallback<Array<Uri>>? = null
    private var nativeAttachmentKind: String? = null
    private lateinit var fileChooserLauncher: ActivityResultLauncher<Intent>

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val host = ClientConfig.load(applicationContext).activeHostConfig()

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
            webChromeClient = AndroidWebChromeClient()
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    reportCanonicalWebUiLayout(view)
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
        applyInsets(root)
        setContentView(root)
        webView.loadUrl(host.webUiUrl)
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
            "({" +
                "webuiShell:!!document.querySelector('[data-webui-shell=true]')," +
                "layoutClient:document.body.dataset.layoutClient||''," +
                "layoutShape:document.body.dataset.layoutShape||''" +
                "})",
        ) { value -> Log.i(WEBUI_LAYOUT_TAG, value ?: "null") }
    }

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
        if (::webView.isInitialized) webView.destroy()
        super.onDestroy()
    }

    companion object {
        private const val LOG_TAG = "FreehandAndroid"
        private const val WEBUI_LAYOUT_TAG = "FreehandWebUiLayout"
    }
}

private val HostConfig.webUiUrl: String
    get() = "$baseUrl/?client=android-webview"

package com.freehand.android.ui

import android.os.Bundle
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.R
import com.freehand.android.data.CommandIngress
import com.freehand.android.data.HostConfig
import com.freehand.android.data.HostStore
import com.freehand.android.data.ProtocolClient
import com.freehand.android.data.TimelineProjector
import com.freehand.android.ui.components.DrawerController
import com.freehand.android.ui.components.InputBarController
import com.freehand.android.ui.components.SlaveStripController
import com.freehand.android.ui.components.StatusBannerController
import com.freehand.android.ui.components.TopBarController
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

class MainActivity : AppCompatActivity() {

    private lateinit var webView: WebView
    private lateinit var inputBar: InputBarController
    private lateinit var topBar: TopBarController
    private lateinit var slaveStrip: SlaveStripController
    private lateinit var statusBanner: StatusBannerController
    private lateinit var drawer: DrawerController
    private lateinit var ingress: CommandIngress
    private lateinit var projector: TimelineProjector
    private lateinit var hostStore: HostStore
    private lateinit var httpClient: OkHttpClient

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        httpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .build()

        hostStore = HostStore(applicationContext)
        val host: HostConfig = hostStore.load()
        val client = ProtocolClient(httpClient, host)
        projector = TimelineProjector()
        ingress = CommandIngress(client) { ok, reason ->
            runOnUiThread {
                if (ok) {
                    inputBar.clear()
                } else {
                    inputBar.markSendError(reason)
                }
            }
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
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    val snapshot = projector.snapshot()
                    if (snapshot != null) {
                        view?.evaluateJavascript(
                            "if(window.__freehand && window.__freehand.applySnapshot){__freehand.applySnapshot(${'$'}snapshotJs)}",
                            null,
                        )
                    }
                }
            }
            loadUrl("file:///android_asset/mobile-shell.html")
        }
        root.addView(
            webView,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )

        statusBanner = StatusBannerController(this, root)
        slaveStrip = SlaveStripController(this, root)
        topBar = TopBarController(this, root) { drawer.toggle() }
        inputBar = InputBarController(this, root, ingress::submit)
        drawer = DrawerController(
            this,
            root,
            onHostChanged = { newHost ->
                hostStore.save(newHost)
                recreate()
            },
            initialHost = host,
        )

        applyInsets(root)
        setContentView(root)
        setContentView(root)
    }

    private fun applyInsets(root: View) {
        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val nav = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            root.setPadding(0, 0, 0, if (ime > 0) 0 else nav)
            insets
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK && drawer.isOpen()) {
            drawer.close()
            return true
        }
        if (keyCode == KeyEvent.KEYCODE_ESCAPE) {
            if (drawer.isOpen()) {
                drawer.close()
            } else {
                ingress.cancelLatest()
            }
            return true
        }
        return super.onKeyDown(keyCode, event)
    }
}

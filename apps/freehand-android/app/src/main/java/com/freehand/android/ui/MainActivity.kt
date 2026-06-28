package com.freehand.android.ui

import android.content.res.Configuration
import android.os.Bundle
import android.view.KeyEvent
import android.view.View
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.R
import com.freehand.android.data.AdpEventStream
import com.freehand.android.data.ClientConfig
import com.freehand.android.data.CommandIngress
import com.freehand.android.data.HostConfig
import com.freehand.android.data.HostStore
import com.freehand.android.data.SlaveState
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
    private lateinit var projector: TimelineProjector
    private lateinit var hostStore: HostStore
    private lateinit var httpClient: OkHttpClient
    private lateinit var clientConfig: ClientConfig
    private lateinit var ingress: CommandIngress
    private var adp: AdpEventStream? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        httpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .build()
        hostStore = HostStore(applicationContext)
        projector = TimelineProjector()
        clientConfig = ClientConfig.load(applicationContext)

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
            clearCache(true)
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    applyInitialTheme(view)
                    pushSnapshotToWebView()
                }
            }
            // bridge.html is the live WebView host page; it consumes
            // `window.__freehand.applySnapshot(...)` pushed from native
            // with ADP UiSubscriptionEvent-shaped JSON. The server-side
            // mobile-mock.html is a static design preview served at
            // /mock/android and is NOT loaded here.
            loadUrl("file:///android_asset/bridge.html")
        }
        root.addView(webView, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT,
        ))

        statusBanner = StatusBannerController(this, root)
        slaveStrip = SlaveStripController(this, root)
        topBar = TopBarController(this, root) { drawer.toggle() }
        // ingress placeholder, rebuilt on connect
        ingress = CommandIngress(
            client = null,
            { ok, reason ->
                runOnUiThread {
                    if (ok) inputBar.clear() else inputBar.markSendError(reason)
                }
            },
        )
        inputBar = InputBarController(this, root) { text -> ingress.submit(text) }
        drawer = DrawerController(this, root, onHostChanged = { newHost ->
            hostStore.save(newHost)
            connectToDaemon(newHost)
        }, initialHost = hostStore.load())

        applyInsets(root)
        setContentView(root)

        // Auto-discover daemon
        discoverDaemon(hostStore.load())
    }

    override fun onResume() {
        super.onResume()
        adp?.start()
    }

    override fun onPause() {
        super.onPause()
        adp?.stop()
    }

    private fun discoverDaemon(saved: HostConfig?) {
        // Connection state machine: connecting -> connected (ADP onOpen only)
        // -> error/closed (ADP onError/onClosed only).
        // discoverDaemon only decides whether to start ADP; it never sets
        // "connected" directly, eliminating the race where health-check pass
        // sets connected while ADP immediately fails and sets unreachable.
        val configHost = clientConfig.toHostConfig()
        val target = selectPreferredHost(saved, configHost)
        topBar.setAgent("freehand", "connecting")
        // Always try to connect directly to the configured host.
        // Health check is advisory only; real connection state comes from SSE.
        connectToDaemon(target)
    }

    private fun connectToDaemon(host: HostConfig) {
        adp?.stop()
        var newAdp: AdpEventStream? = null
        ingress = CommandIngress(
            client = null,
            onResult = { ok, reason ->
                runOnUiThread {
                    if (ok) inputBar.clear() else inputBar.markSendError(reason)
                }
            },
            sendCommand = { command ->
                newAdp?.sendCommand(command)
                    ?: com.freehand.android.data.CommandResponse(false, "adp_not_ready", "ADP not ready")
            },
        )
        topBar.setAgent("${host.host}:${host.port}", "connecting")
        newAdp = AdpEventStream(httpClient, host,
            onEvent = { event ->
                runOnUiThread {
                    projector.applyAdp(event)
                    pushSnapshotToWebView()
                }
            },
            onCommandResult = { result ->
                runOnUiThread {
                    if (result.ok) inputBar.clear() else inputBar.markSendError(result.message.ifBlank { result.code })
                }
            },
            onError = { _ ->
                runOnUiThread {
                    projector.setConnectionState("error")
                    statusBanner.showPersistent("daemon unreachable: ${host.host}:${host.port}")
                    topBar.setAgent("${host.host}:${host.port}", "offline")
                }
            },
            onOpen = {
                runOnUiThread {
                    projector.setConnectionState("open")
                    statusBanner.hide()
                    topBar.setAgent("${host.host}:${host.port}", "connected")
                }
            },
            onClosed = {
                runOnUiThread {
                    projector.setConnectionState("closed")
                    topBar.setAgent("${host.host}:${host.port}", "offline")
                }
            },
        )
        adp = newAdp
        newAdp.start()
    }

    private fun pushSnapshotToWebView() {
        if (!::webView.isInitialized) return
        val snapshot = projector.snapshot()
        topBar.setAgent(
            name = (snapshot["agent_name"] as? String)?.ifBlank { "agent" } ?: "agent",
            status = snapshot["connection"] as? String ?: "idle",
        )
        val slaves = (snapshot["slaves"] as? Map<*, *>)?.entries?.mapNotNull { entry ->
            val id = entry.key as? String ?: return@mapNotNull null
            val v = entry.value as? Map<*, *> ?: return@mapNotNull null
            id to SlaveState(v["node_id"] as? String ?: id, v["pairing_state"] as? String ?: "unknown")
        } ?: emptyList()
        slaveStrip.render(slaves.map { it.first to it.second.pairingState })
        // The bridge expects the canonical UiPublicTurnProjection shape
        // { turn: UiTurnProjection, public_conversation: [...] } emitted by
        // the daemon ADP turn projection. Projector.snapshot() exposes it under
        // `latest_turn`; fall back to the legacy flat `turns` list so the
        // bridge still has something to render before the first `turn` event
        // arrives.
        val json = projector.latestTurnProjectionJson() ?: projector.fallbackTurnsJson()
        webView.evaluateJavascript(
            "if(window.__freehand&&window.__freehand.applySnapshot){window.__freehand.applySnapshot($json);}else{window.__freehandPending=$json;}",
            null,
        )
    }

    private fun selectPreferredHost(saved: HostConfig?, bundled: HostConfig): HostConfig {
        if (saved == null) return bundled
        val savedHost = saved.host.trim()
        val bundledHost = bundled.host.trim()
        val savedLooksLegacy = savedHost == "127.0.0.1" || savedHost.startsWith("192.168.")
        val savedUsesLegacyPort = saved.port == 4040
        val shouldOverrideLegacyHost = savedLooksLegacy && bundledHost.startsWith("100.")
        val shouldOverrideLegacyPort = savedHost == bundledHost && savedUsesLegacyPort && bundled.port != saved.port
        if (shouldOverrideLegacyHost || shouldOverrideLegacyPort) {
            hostStore.save(bundled)
            ClientConfig.saveOverride(applicationContext, bundled.host, bundled.port)
            return bundled
        }
        return saved
    }

    private fun applyInitialTheme(view: WebView?) {
        val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val v = if (night) "true" else "false"
        view?.evaluateJavascript("document.body.classList.toggle('theme-dark',$v);", null)
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
            if (drawer.isOpen()) drawer.close() else ingress.cancelLatest()
            return true
        }
        return super.onKeyDown(keyCode, event)
    }
}

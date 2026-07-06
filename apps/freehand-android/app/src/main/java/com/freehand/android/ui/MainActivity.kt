package com.freehand.android.ui

import android.content.res.Configuration
import android.os.Bundle
import android.view.KeyEvent
import android.view.View
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
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
import com.freehand.android.data.DaemonConnectionConfig
import com.freehand.android.data.DaemonConnectionConfigException
import com.freehand.android.data.DaemonConnectionConfigStore
import com.freehand.android.data.HostConfig
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
    private lateinit var configStore: DaemonConnectionConfigStore
    private lateinit var httpClient: OkHttpClient
    private var clientConfig: DaemonConnectionConfig? = null
    private lateinit var ingress: CommandIngress
    private var adp: AdpEventStream? = null
    private var configLoadError: String? = null
    private var remoteWebUiLoaded = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        httpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .build()
        projector = TimelineProjector()
        configStore = ClientConfig.store(applicationContext)
        val loadedConfig = try {
            configStore.load()
        } catch (e: DaemonConnectionConfigException) {
            configLoadError = e.message ?: "invalid daemon connection config"
            null
        }
        clientConfig = loadedConfig
        val initialHost = loadedConfig
            ?.let { runCatching { it.activeHostConfig() }.getOrNull() }
            ?: DaemonConnectionConfig.defaultTailscale().activeHostConfig()

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
            clearCache(true)
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    if (!remoteWebUiLoaded) {
                        applyInitialTheme(view)
                        pushSnapshotToWebView()
                    }
                }

                override fun onReceivedError(
                    view: WebView?,
                    request: WebResourceRequest?,
                    error: WebResourceError?,
                ) {
                    super.onReceivedError(view, request, error)
                    if (request?.isForMainFrame == true) {
                        showNativeShell(true)
                        statusBanner.showPersistent("webui unreachable: ${error?.description ?: "load failed"}")
                    }
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
            if (saveHostConfig(newHost)) {
                connectToDaemon(newHost)
            }
        }, initialHost = initialHost)

        applyInsets(root)
        setContentView(root)

        val configError = configLoadError
        if (configError != null) {
            projector.setConnectionState("config_error")
            statusBanner.showPersistent("daemon config error: $configError")
            topBar.setAgent("freehand", "config error")
            inputBar.setEnabledState(false)
        } else {
            discoverDaemon()
        }
    }

    override fun onResume() {
        super.onResume()
        adp?.start()
    }

    override fun onPause() {
        super.onPause()
        adp?.stop()
    }

    private fun discoverDaemon() {
        // Connection state machine: connecting -> connected (ADP onOpen only)
        // -> error/closed (ADP onError/onClosed only).
        // discoverDaemon only decides whether to start ADP; it never sets
        // "connected" directly, eliminating the race where health-check pass
        // sets connected while ADP immediately fails and sets unreachable.
        val target = try {
            clientConfig?.activeHostConfig()
                ?: throw DaemonConnectionConfigException("daemon connection config is not loaded")
        } catch (e: DaemonConnectionConfigException) {
            projector.setConnectionState("config_error")
            statusBanner.showPersistent("daemon config error: ${e.message}")
            topBar.setAgent("freehand", "config error")
            inputBar.setEnabledState(false)
            return
        }
        topBar.setAgent("freehand", "connecting")
        connectToDaemon(target)
    }

    private fun connectToDaemon(host: HostConfig) {
        adp?.stop()
        projector.clearAccumulatedTurns()
        var newAdp: AdpEventStream? = null
        // Disable input until ADP signals ready — prevents submit before stream is live.
        inputBar.setEnabledState(false)
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
        topBar.setAgent(host.endpointLabel, "connecting")
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
            onError = { error ->
                runOnUiThread {
                    showNativeShell(true)
                    projector.setConnectionState("error")
                    val errorClass = error::class.java.simpleName.ifBlank { "ConnectionError" }
                    statusBanner.showPersistent("daemon unreachable: ${host.endpointLabel} · $errorClass")
                    topBar.setAgent(host.endpointLabel, "offline")
                }
            },
            onOpen = {
                runOnUiThread {
                    projector.setConnectionState("open")
                    statusBanner.hide()
                    topBar.setAgent(host.endpointLabel, "connected")
                    loadRemoteWebUi(host)
                }
            },
            onClosed = {
                runOnUiThread {
                    projector.setConnectionState("closed")
                    topBar.setAgent(host.endpointLabel, "offline")
                }
            },
        )
        adp = newAdp
        newAdp.start()
    }

    private fun loadRemoteWebUi(host: HostConfig) {
        remoteWebUiLoaded = true
        showNativeShell(false)
        webView.loadUrl("${host.baseUrl}/")
    }

    private fun showNativeShell(visible: Boolean) {
        val visibility = if (visible) View.VISIBLE else View.GONE
        topBar.root().visibility = visibility
        slaveStrip.root().visibility = if (visible) View.GONE else View.GONE
        inputBar.root().visibility = visibility
    }

    private fun pushSnapshotToWebView() {
        if (!::webView.isInitialized) return
        if (remoteWebUiLoaded) return
        val snapshot = projector.snapshot()
        topBar.setAgent(
            name = (snapshot["agent_name"] as? String)?.ifBlank { "agent" } ?: "agent",
            status = snapshot["connection"] as? String ?: "idle",
        )

        // Sync turn lifecycle state to input bar: disable while agent is working,
        // re-enable on terminal states or error. Connection-down also locks out.
        val connectionState = snapshot["connection"] as? String ?: "idle"
        val turnState = snapshot["turn_state"] as? String ?: "idle"
        val shouldEnableInput = connectionState == "open" && when (turnState) {
            "done", "error", "blocked", "cancelled", "idle", "waiting" -> true
            else -> false  // running / thinking / streaming → disable
        }
        inputBar.setEnabledState(shouldEnableInput)

        // Surface turn-level progress on the status banner. Hides automatically
        // when state is terminal/idle so we don't leave stale text hanging.
        if (connectionState == "open") {
            statusBanner.showTurnProgress(turnState)
        } else if (connectionState != "config_error" && connectionState != "error") {
            statusBanner.hide()
        }

        val slaves = (snapshot["slaves"] as? Map<*, *>)?.entries?.mapNotNull { entry ->
            val id = entry.key as? String ?: return@mapNotNull null
            val v = entry.value as? Map<*, *> ?: return@mapNotNull null
            id to SlaveState(v["node_id"] as? String ?: id, v["pairing_state"] as? String ?: "unknown")
        } ?: emptyList()
        slaveStrip.render(slaves.map { it.first to it.second.pairingState })

        // Build bridge snapshot with all accumulated turns for multi-turn rendering.
        // Preferred shape: { all_turns: [ UiPublicTurnProjection, ... ] }.
        // Falls back to latest single turn or legacy flat list if no projections yet.
        val allTurnsJson = projector.allTurnsProjectionJson()
            ?: projector.latestTurnProjectionJson()
            ?: projector.fallbackTurnsJson()

        webView.evaluateJavascript(
            "if(window.__freehand&&window.__freehand.applySnapshot){window.__freehand.applySnapshot($allTurnsJson);}else{window.__freehandPending=$allTurnsJson;}",
            null,
        )
    }

    private fun saveHostConfig(host: HostConfig): Boolean {
        val current = clientConfig
        val updated = if (current == null) {
            DaemonConnectionConfig(
                connectionMode = host.mode,
                activeProfile = host.profileId,
                profiles = listOf(host.toConnectionProfile()),
                relay = com.freehand.android.data.DaemonRelayConfig(enabled = false, url = "", authRef = ""),
            )
        } else {
            val profiles = current.profiles.map {
                if (it.id == host.profileId) host.toConnectionProfile() else it
            }
            current.copy(
                connectionMode = host.mode,
                activeProfile = host.profileId,
                profiles = if (profiles.any { it.id == host.profileId }) profiles else profiles + host.toConnectionProfile(),
            )
        }
        return try {
            configStore.write(updated)
            clientConfig = updated
            true
        } catch (e: DaemonConnectionConfigException) {
            projector.setConnectionState("config_error")
            statusBanner.showPersistent("daemon config error: ${e.message}")
            topBar.setAgent(host.endpointLabel, "config error")
            inputBar.setEnabledState(false)
            false
        }
    }

    private fun HostConfig.toConnectionProfile(): com.freehand.android.data.DaemonConnectionProfile =
        com.freehand.android.data.DaemonConnectionProfile(
            id = profileId,
            mode = mode,
            host = host,
            port = port,
            adpPath = adpPath,
            healthPath = healthPath,
            commandPath = commandPath,
            queryPath = queryPath,
            subscribePath = subscribePath,
        )

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

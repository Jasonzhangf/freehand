package com.freehand.android.ui

import android.content.res.Configuration
import android.content.Intent
import android.provider.OpenableColumns
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.view.KeyEvent
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
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.BuildConfig
import com.freehand.android.R
import com.freehand.android.data.AdpEventStream
import com.freehand.android.data.ApkUpdateClient
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
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
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
    private lateinit var updateClient: ApkUpdateClient
    private var updateCheckInFlight = false
    private var fileChooserCallback: ValueCallback<Array<Uri>>? = null
    private var nativeAttachmentKind: String? = null
    private lateinit var fileChooserLauncher: ActivityResultLauncher<Intent>

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        fileChooserLauncher = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            val attachmentKind = nativeAttachmentKind
            if (attachmentKind != null) {
                nativeAttachmentKind = null
                injectAndroidAttachmentSelection(attachmentKind, result.data)
                return@registerForActivityResult
            }
            val callback = fileChooserCallback ?: return@registerForActivityResult
            fileChooserCallback = null
            callback.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(result.resultCode, result.data))
        }

        httpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .build()
        updateClient = ApkUpdateClient(httpClient, BuildConfig.VERSION_CODE.toLong())
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
            addJavascriptInterface(AndroidFilePickerBridge(), "FreehandAndroidFilePicker")
            webChromeClient = object : WebChromeClient() {
                override fun onShowFileChooser(
                    webView: WebView?,
                    filePathCallback: ValueCallback<Array<Uri>>?,
                    fileChooserParams: FileChooserParams?,
                ): Boolean {
                    fileChooserCallback?.onReceiveValue(null)
                    fileChooserCallback = filePathCallback
                    val intent = try {
                        fileChooserParams?.createIntent()
                    } catch (e: Exception) {
                        fileChooserCallback?.onReceiveValue(null)
                        fileChooserCallback = null
                        showUpdateStatus("file picker request failed: ${e.message ?: e::class.java.simpleName}")
                        return false
                    }
                    if (intent == null) {
                        fileChooserCallback?.onReceiveValue(null)
                        fileChooserCallback = null
                        showUpdateStatus("file picker request failed")
                        return false
                    }
                    return try {
                        fileChooserLauncher.launch(intent)
                        true
                    } catch (e: Exception) {
                        fileChooserCallback?.onReceiveValue(null)
                        fileChooserCallback = null
                        showUpdateStatus("file picker unavailable: ${e.message ?: e::class.java.simpleName}")
                        false
                    }
                }
            }
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    super.onPageFinished(view, url)
                    if (remoteWebUiLoaded) {
                        reportRemoteWebUiLayout(view)
                    } else {
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
        drawer = DrawerController(
            this,
            root,
            onHostChanged = { newHost ->
                if (saveHostConfig(newHost)) {
                    connectToDaemon(newHost)
                }
            },
            onCheckUpdate = { checkForApkUpdate(auto = false) },
            initialHost = initialHost,
        )

        applyInsets(root)
        setContentView(root)

        val configError = configLoadError
        if (configError != null) {
            projector.setConnectionState("config_error")
            statusBanner.showPersistent("daemon config error: $configError")
            topBar.setAgent("freehand", "config error")
            inputBar.setEnabledState(false)
        } else {
            checkForApkUpdate(auto = true)
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
                    ?: com.freehand.android.data.CommandResponse(false, "adp_not_ready", "service connection is not ready")
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
        webView.loadUrl("${host.baseUrl}/?client=android-webview")
    }

    private inner class AndroidFilePickerBridge {
        @JavascriptInterface
        fun request(kind: String) {
            runOnUiThread {
                openAndroidAttachmentPicker(kind)
            }
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
        } catch (e: Exception) {
            nativeAttachmentKind = null
            showUpdateStatus("file picker unavailable: ${e.message ?: e::class.java.simpleName}")
        }
    }

    private fun injectAndroidAttachmentSelection(kind: String, data: Intent?) {
        val uris = selectedUris(data)
        if (uris.isEmpty()) {
            return
        }
        val files = JSONArray()
        uris.forEach { uri ->
            files.put(
                JSONObject()
                    .put("name", displayName(uri))
                    .put("size", displaySize(uri))
                    .put("type", contentResolver.getType(uri) ?: "application/octet-stream")
                    .put("uri", uri.toString()),
            )
        }
        val script = "window.__freehandAndroidAttachmentSelected && window.__freehandAndroidAttachmentSelected(${JSONObject.quote(kind)}, $files);"
        webView.evaluateJavascript(script, null)
    }

    private fun selectedUris(data: Intent?): List<Uri> {
        val clipData = data?.clipData
        if (clipData != null && clipData.itemCount > 0) {
            return (0 until clipData.itemCount).mapNotNull { clipData.getItemAt(it).uri }
        }
        return data?.data?.let { listOf(it) } ?: emptyList()
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
                if (index >= 0 && !cursor.isNull(index)) {
                    return cursor.getLong(index)
                }
            }
        }
        return -1L
    }

    private fun showNativeShell(visible: Boolean) {
        val visibility = if (visible) View.VISIBLE else View.GONE
        topBar.root().visibility = visibility
        slaveStrip.root().visibility = if (visible) View.GONE else View.GONE
        inputBar.root().visibility = visibility
        if (!visible) {
            statusBanner.hide()
        }
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

        // Native banner is pre-connection shell only. Once remote WebUI owns the
        // screen, do not overlay legacy Android status chrome on top of it.
        if (!remoteWebUiLoaded && connectionState == "open") {
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

        // Build the single bridge projection shape for multi-turn rendering.
        val allTurnsJson = projector.allTurnsProjectionJson()

        webView.evaluateJavascript(
            "if(window.__freehand&&window.__freehand.applySnapshot){window.__freehand.applySnapshot($allTurnsJson);}else{window.__freehandPending=$allTurnsJson;}",
            null,
        )
    }

    private fun reportRemoteWebUiLayout(view: WebView?) {
        val webView = view ?: return
        webView.evaluateJavascript(
            """
            (() => ({
              shape: document.body.dataset.layoutShape || "",
              mobileDrawer: document.body.dataset.mobileDrawer || "",
              innerWidth: window.innerWidth,
              innerHeight: window.innerHeight,
              visualWidth: window.visualViewport ? window.visualViewport.width : null,
              visualHeight: window.visualViewport ? window.visualViewport.height : null,
              conversationPrimary: !!document.querySelector(".workspace"),
              sessionDrawerFixed: !!document.querySelector(".sidebar") &&
                getComputedStyle(document.querySelector(".sidebar")).position === "fixed",
              detailDrawerFixed: !!document.querySelector(".inspector") &&
                getComputedStyle(document.querySelector(".inspector")).position === "fixed",
              sessionDrawerInViewport: (() => {
                const node = document.querySelector(".sidebar");
                if (!node) return false;
                const box = node.getBoundingClientRect();
                return box.right > 0 && box.left < window.innerWidth;
              })(),
              detailDrawerInViewport: (() => {
                const node = document.querySelector(".inspector");
                if (!node) return false;
                const box = node.getBoundingClientRect();
                return box.right > 0 && box.left < window.innerWidth;
              })()
            }))();
            """.trimIndent(),
        ) { value ->
            Log.i("FreehandWebUiLayout", value ?: "null")
        }
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

    private fun checkForApkUpdate(auto: Boolean) {
        if (updateCheckInFlight) {
            if (!auto) showUpdateStatus("update check already running")
            return
        }
        val host = try {
            clientConfig?.activeHostConfig()
                ?: throw DaemonConnectionConfigException("daemon connection config is not loaded")
        } catch (e: DaemonConnectionConfigException) {
            showUpdateStatus("update check failed: ${e.message}")
            return
        }
        updateCheckInFlight = true
        showUpdateStatus("checking for Android update...")
        Thread {
            try {
                val result = updateClient.check(host.updateManifestUrl)
                val manifest = result.manifest
                if (!result.updateAvailable || manifest == null) {
                    runOnUiThread { showUpdateStatus("app is up to date") }
                    return@Thread
                }
                runOnUiThread {
                    showUpdateStatus("downloading Freehand ${manifest.versionName}...")
                }
                val apk = updateClient.download(
                    manifest,
                    File(cacheDir, "apk-updates/freehand-${manifest.versionCode}.apk"),
                )
                runOnUiThread {
                    showUpdateStatus("downloaded ${manifest.versionName}; opening installer.")
                    openApkInstaller(apk)
                }
            } catch (e: Exception) {
                runOnUiThread {
                    if (!auto) {
                        showUpdateStatus("update failed: ${e.message ?: e::class.java.simpleName}")
                    }
                }
            } finally {
                updateCheckInFlight = false
            }
        }.start()
    }

    private fun showUpdateStatus(message: String) {
        drawer.setUpdateStatus(message)
    }

    private fun openApkInstaller(apk: File) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !packageManager.canRequestPackageInstalls()) {
            val settingsIntent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                data = Uri.parse("package:$packageName")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            startActivity(settingsIntent)
            showUpdateStatus("allow Freehand to install updates, then check again.")
            return
        }
        val uri: Uri = FileProvider.getUriForFile(
            this,
            "${BuildConfig.APPLICATION_ID}.apkupdates",
            apk,
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        try {
            startActivity(intent)
        } catch (e: Exception) {
            showUpdateStatus("installer unavailable: ${e.message ?: e::class.java.simpleName}")
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

package com.freehand.android.ui

import android.Manifest
import android.animation.AnimatorSet
import android.animation.ObjectAnimator
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.AlertDialog
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.ColorStateList
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.provider.Settings
import android.util.Base64
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import android.webkit.WebResourceResponse
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.EditText
import android.widget.ProgressBar
import android.widget.Button
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.freehand.android.BuildConfig
import com.freehand.android.R
import com.freehand.android.data.ClientConfig
import com.freehand.android.data.DaemonConnectionConfig
import com.freehand.android.data.DaemonConnectionConfigStore
import com.freehand.android.data.DaemonConnectionConfigException
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
    private lateinit var startupOverlay: FrameLayout
    private lateinit var startupStatus: TextView
    private val mainHandler = Handler(Looper.getMainLooper())
    private var startupAnimator: AnimatorSet? = null
    private var fileChooserCallback: ValueCallback<Array<Uri>>? = null
    private var nativeAttachmentKind: String? = null
    private lateinit var fileChooserLauncher: ActivityResultLauncher<Intent>
    private lateinit var fileAccessPermissionLauncher: ActivityResultLauncher<Array<String>>
    private lateinit var allFilesAccessSettingsLauncher: ActivityResultLauncher<Intent>
    private lateinit var notificationPermissionLauncher: ActivityResultLauncher<String>
    private lateinit var connectionsLauncher: ActivityResultLauncher<Intent>
    private lateinit var apkUpdater: AndroidApkUpdater
    private var lastApkUpdateStatus: ApkUpdateStatus? = null
    private var currentHostConfig: HostConfig? = null
    private var endpointConfigAutoPrompted: Boolean = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val host = loadHostConfigFromStartupIntent().also { currentHostConfig = it }
        apkUpdater = AndroidApkUpdater(applicationContext, host)

        notificationPermissionLauncher = registerForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            logNotificationStatus(
                phase = if (granted) "runtime_permission_granted" else "runtime_permission_restricted",
            )
            requestInstallFileAccessIfNeeded()
        }
        fileAccessPermissionLauncher = registerForActivityResult(
            ActivityResultContracts.RequestMultiplePermissions(),
        ) { grants ->
            val denied = grants.filterValues { granted -> !granted }.keys.sorted()
            logFileAccessStatus(
                phase = if (denied.isEmpty()) "runtime_permissions_granted" else "runtime_permissions_restricted",
                extra = if (denied.isEmpty()) "" else "denied=${denied.joinToString("|")}",
            )
            openAllFilesAccessSettingsIfNeeded("runtime_permissions")
        }
        allFilesAccessSettingsLauncher = registerForActivityResult(
            ActivityResultContracts.StartActivityForResult(),
        ) {
            logFileAccessStatus(
                phase = if (needsAllFilesAccess()) "all_files_restricted" else "all_files_granted",
            )
        }
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
        connectionsLauncher = registerForActivityResult(
            ActivityResultContracts.StartActivityForResult(),
        ) { result ->
            if (result.resultCode != RESULT_OK) return@registerForActivityResult
            try {
                val newHost = ClientConfig.load(applicationContext).activeHostConfig()
                applyActiveHost(newHost)
            } catch (error: DaemonConnectionConfigException) {
                Log.e(LOG_TAG, "failed to reload config after connections page", error)
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
            settings.useWideViewPort = false
            settings.loadWithOverviewMode = false
            settings.textZoom = 100
            addJavascriptInterface(AndroidFilePickerBridge(), "FreehandAndroidFilePicker")
            addJavascriptInterface(AndroidApkUpdateBridge(), "FreehandAndroidApkUpdate")
            addJavascriptInterface(AndroidNotificationsBridge(), "FreehandAndroidNotifications")
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

                override fun onReceivedHttpError(
                    view: WebView?,
                    request: WebResourceRequest?,
                    errorResponse: WebResourceResponse?,
                ) {
                    super.onReceivedHttpError(view, request, errorResponse)
                    val statusCode = errorResponse?.statusCode ?: return
                    if (statusCode < 400) return
                    Log.e(
                        WEBUI_ASSET_TAG,
                        "http_status=$statusCode main_frame=${request?.isForMainFrame == true}",
                    )
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
        if (!requestInstallNotificationPermissionIfNeeded()) {
            requestInstallFileAccessIfNeeded()
        }
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
        override fun onConsoleMessage(consoleMessage: android.webkit.ConsoleMessage?): Boolean {
            val message = consoleMessage ?: return super.onConsoleMessage(consoleMessage)
            Log.i(
                WEBUI_CONSOLE_TAG,
                "event=console level=${message.messageLevel()} line=${message.lineNumber()}",
            )
            return true
        }

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

        @JavascriptInterface
        fun manifestUrl(): String = apkUpdater.updateManifestUrl
    }

    private inner class AndroidNotificationsBridge {
        @JavascriptInterface
        fun turnFinished(payloadJson: String) {
            runOnUiThread { showTurnFinishedNotification(payloadJson) }
        }
    }

    private fun requestInstallNotificationPermissionIfNeeded(): Boolean {
        createNotificationChannel()
        val permission = NotificationPermissionPolicy.runtimePermissionForSdk(Build.VERSION.SDK_INT)
            ?: run {
                logNotificationStatus(phase = "not_applicable")
                return false
            }
        val missing = ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
        val preferences = getSharedPreferences(
            NotificationPermissionPolicy.PREFS_NAME,
            Context.MODE_PRIVATE,
        )
        val currentInstallMarker = currentInstallMarker()
        val promptedInstallMarker = preferences.getLong(
            NotificationPermissionPolicy.PROMPTED_INSTALL_MARKER_KEY,
            -1L,
        )
        if (!NotificationPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = promptedInstallMarker,
                currentInstallMarker = currentInstallMarker,
                permissionMissing = missing,
            )
        ) {
            logNotificationStatus(
                phase = if (missing) "previously_requested_restricted" else "already_granted",
            )
            return false
        }
        preferences.edit()
            .putLong(NotificationPermissionPolicy.PROMPTED_INSTALL_MARKER_KEY, currentInstallMarker)
            .apply()
        logNotificationStatus(phase = "startup_request")
        notificationPermissionLauncher.launch(permission)
        return true
    }

    private fun requestInstallFileAccessIfNeeded() {
        val missingRuntimePermissions = missingRuntimeFilePermissions()
        val needsAllFilesAccess = needsAllFilesAccess()
        val preferences = getSharedPreferences(
            FileAccessPermissionPolicy.PREFS_NAME,
            Context.MODE_PRIVATE,
        )
        val promptedInstallMarker = preferences.getLong(
            FileAccessPermissionPolicy.PROMPTED_INSTALL_MARKER_KEY,
            -1L,
        )
        val currentInstallMarker = currentInstallMarker()
        val shouldPrompt = FileAccessPermissionPolicy.shouldPromptForInstall(
            promptedInstallMarker = promptedInstallMarker,
            currentInstallMarker = currentInstallMarker,
            missingRuntimePermissionCount = missingRuntimePermissions.size,
            needsAllFilesAccess = needsAllFilesAccess,
        )
        if (!shouldPrompt) {
            logFileAccessStatus(
                phase = if (missingRuntimePermissions.isEmpty() && !needsAllFilesAccess) {
                    "already_granted"
                } else {
                    "previously_requested_restricted"
                },
                missingRuntimePermissions = missingRuntimePermissions,
                needsAllFilesAccess = needsAllFilesAccess,
            )
            return
        }
        preferences.edit()
            .putLong(FileAccessPermissionPolicy.PROMPTED_INSTALL_MARKER_KEY, currentInstallMarker)
            .apply()
        logFileAccessStatus(
            phase = "startup_request",
            missingRuntimePermissions = missingRuntimePermissions,
            needsAllFilesAccess = needsAllFilesAccess,
        )
        if (missingRuntimePermissions.isNotEmpty()) {
            fileAccessPermissionLauncher.launch(missingRuntimePermissions.toTypedArray())
            return
        }
        openAllFilesAccessSettingsIfNeeded("startup")
    }

    private fun missingRuntimeFilePermissions(): List<String> =
        FileAccessPermissionPolicy.runtimePermissionsForSdk(Build.VERSION.SDK_INT).filter { permission ->
            ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
        }

    private fun needsAllFilesAccess(): Boolean =
        FileAccessPermissionPolicy.allFilesSettingsAvailableForSdk(Build.VERSION.SDK_INT) &&
            !Environment.isExternalStorageManager()

    private fun currentInstallMarker(): Long =
        packageManager.getPackageInfo(packageName, 0).lastUpdateTime

    private fun openAllFilesAccessSettingsIfNeeded(source: String) {
        if (!needsAllFilesAccess()) {
            logFileAccessStatus(phase = "all_files_not_needed_after_$source")
            return
        }
        val intent = Intent(
            Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
            Uri.parse("package:$packageName"),
        )
        logFileAccessStatus(phase = "all_files_settings_request_after_$source")
        try {
            allFilesAccessSettingsLauncher.launch(intent)
        } catch (error: ActivityNotFoundException) {
            logFileAccessStatus(
                phase = "all_files_settings_unavailable",
                extra = error.message ?: "activity_not_found",
            )
        } catch (error: SecurityException) {
            logFileAccessStatus(
                phase = "all_files_settings_unavailable",
                extra = error.message ?: "security_exception",
            )
        }
    }

    private fun logFileAccessStatus(
        phase: String,
        missingRuntimePermissions: List<String> = missingRuntimeFilePermissions(),
        needsAllFilesAccess: Boolean = needsAllFilesAccess(),
        extra: String = "",
    ) {
        val allFilesState = when {
            !FileAccessPermissionPolicy.allFilesSettingsAvailableForSdk(Build.VERSION.SDK_INT) -> "not_applicable"
            needsAllFilesAccess -> "missing"
            else -> "granted"
        }
        val runtimeState = if (missingRuntimePermissions.isEmpty()) {
            "granted"
        } else {
            missingRuntimePermissions.joinToString("|")
        }
        val parts = mutableListOf(
            "phase=$phase",
            "versionCode=${BuildConfig.VERSION_CODE}",
            "installMarker=${currentInstallMarker()}",
            "runtime=$runtimeState",
            "allFiles=$allFilesState",
        )
        if (extra.isNotBlank()) {
            parts.add(extra)
        }
        Log.i(FILE_ACCESS_TAG, parts.joinToString(" "))
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < 26) return
        val manager = getSystemService(NotificationManager::class.java)
        val channel = NotificationChannel(
            TURN_FINISHED_CHANNEL_ID,
            "Freehand task completion",
            NotificationManager.IMPORTANCE_DEFAULT,
        ).apply {
            description = "Notifications when a Freehand turn finishes"
        }
        manager.createNotificationChannel(channel)
    }

    private fun showTurnFinishedNotification(payloadJson: String) {
        createNotificationChannel()
        val payload = try {
            JSONObject(payloadJson)
        } catch (error: Exception) {
            Log.e(NOTIFICATION_TAG, "invalid turn-finished payload", error)
            return
        }
        val sessionId = payload.optString("sessionId", "")
        val turnId = payload.optString("turnId", "")
        val status = payload.optString("status", "")
        if (sessionId.isBlank() || turnId.isBlank() || status.isBlank()) {
            logNotificationStatus(phase = "turn_finished_invalid", extra = "turn=$turnId")
            return
        }
        val dedupeKey = "$sessionId:$turnId:$status"
        if (notificationAlreadyShown(dedupeKey)) {
            logNotificationStatus(phase = "turn_finished_duplicate", extra = "turn=$turnId")
            return
        }
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            logNotificationStatus(phase = "turn_finished_permission_missing", extra = "turn=$turnId")
            return
        }
        val openIntent = Intent(this, MainActivity::class.java).apply {
            action = Intent.ACTION_MAIN
            addCategory(Intent.CATEGORY_LAUNCHER)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
            putExtra("freehand_session_id", sessionId)
            putExtra("freehand_turn_id", turnId)
        }
        val pendingIntent = PendingIntent.getActivity(
            this,
            dedupeKey.hashCode(),
            openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(this, TURN_FINISHED_CHANNEL_ID)
            .setSmallIcon(com.freehand.android.R.mipmap.ic_launcher)
            .setContentTitle(payload.optString("title", "任务已经完成"))
            .setContentText(payload.optString("text", "Freehand turn finished"))
            .setStyle(NotificationCompat.BigTextStyle().bigText(payload.optString("text", "")))
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .build()
        try {
            NotificationManagerCompat.from(this).notify(NOTIFICATION_TAG, dedupeKey.hashCode(), notification)
            markNotificationShown(dedupeKey)
            logNotificationStatus(phase = "turn_finished_posted", extra = "turn=$turnId")
        } catch (error: SecurityException) {
            Log.e(NOTIFICATION_TAG, "turn-finished notification permission failed", error)
            logNotificationStatus(phase = "turn_finished_permission_failed", extra = "turn=$turnId")
        }
    }

    private fun notificationAlreadyShown(key: String): Boolean {
        val preferences = getSharedPreferences(
            NotificationPermissionPolicy.PREFS_NAME,
            Context.MODE_PRIVATE,
        )
        return preferences.getStringSet(NotificationPermissionPolicy.NOTIFIED_TURNS_KEY, emptySet())
            ?.contains(key) == true
    }

    private fun markNotificationShown(key: String) {
        val preferences = getSharedPreferences(
            NotificationPermissionPolicy.PREFS_NAME,
            Context.MODE_PRIVATE,
        )
        val next = preferences
            .getStringSet(NotificationPermissionPolicy.NOTIFIED_TURNS_KEY, emptySet())
            ?.toMutableSet()
            ?: mutableSetOf()
        next.add(key)
        preferences.edit()
            .putStringSet(NotificationPermissionPolicy.NOTIFIED_TURNS_KEY, next.toList().takeLast(500).toSet())
            .apply()
    }

    private fun logNotificationStatus(phase: String, extra: String = "") {
        val permission = NotificationPermissionPolicy.runtimePermissionForSdk(Build.VERSION.SDK_INT)
        val state = when {
            permission == null -> "not_applicable"
            ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED -> "granted"
            else -> "missing"
        }
        val parts = mutableListOf(
            "phase=$phase",
            "versionCode=${BuildConfig.VERSION_CODE}",
            "installMarker=${currentInstallMarker()}",
            "permission=$state",
        )
        if (extra.isNotBlank()) {
            parts.add(extra)
        }
        Log.i(NOTIFICATION_TAG, parts.joinToString(" "))
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
        checkApkUpdateFor(apkUpdater)
    }

    private fun checkApkUpdateFor(updater: AndroidApkUpdater) {
        updater.checkForUpdateAsync { status ->
            runOnUiThread {
                if (updater.isCurrent()) recordAndroidApkUpdateStatus(status)
            }
        }
    }

    private fun recordAndroidApkUpdateStatus(status: ApkUpdateStatus) {
        lastApkUpdateStatus = status
        emitAndroidApkUpdateStatus(status)
        if (status.phase == "failed") {
            promptEndpointConfigIfNeeded()
        }
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
            val mediaType = contentResolver.getType(uri) ?: "application/octet-stream"
            val file = JSONObject()
                .put("name", displayName(uri))
                .put("size", displaySize(uri))
                .put("type", mediaType)
                .put("uri", uri.toString())
            if (kind == "image" || mediaType.startsWith("image/")) {
                base64ForUri(uri)?.let { file.put("data_base64", it) }
            }
            files.put(
                file,
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

    private fun base64ForUri(uri: Uri): String? =
        try {
            contentResolver.openInputStream(uri)?.use { stream ->
                Base64.encodeToString(stream.readBytes(), Base64.NO_WRAP)
            }
        } catch (error: Exception) {
            Log.e(LOG_TAG, "image attachment read failed for $uri", error)
            null
        }

    private fun reportCanonicalWebUiLayout(view: WebView?) {
        if (view == null) return
        webUiLayoutRetriesRemaining = WEBUI_LAYOUT_PROBE_RETRIES
        mainHandler.removeCallbacks(webUiLayoutRetry)
        mainHandler.postDelayed(webUiLayoutRetry, WEBUI_LAYOUT_PROBE_RETRY_MS)
    }

    private val webUiLayoutRetry = object : Runnable {
        override fun run() {
            if (!isFinishing) {
                evaluateCanonicalWebUiLayout(webView)
            }
        }
    }
    private var webUiLayoutRetriesRemaining = 0

    private fun evaluateCanonicalWebUiLayout(targetView: WebView) {
        targetView.evaluateJavascript(
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
                "webuiRoute:document.body.dataset.webuiRoute||''," +
                "composerVisible:(function(){" +
                "var composer=document.getElementById('composer-card')||document.querySelector('.composer-card');" +
                "if(!composer){return false;}" +
                "var style=window.getComputedStyle(composer);" +
                "if(style.display==='none'||style.visibility==='hidden'){return false;}" +
                "var rect=composer.getBoundingClientRect();" +
                "return rect.width>0&&rect.height>0;" +
                "})()," +
                "focusedEditable:(function(){" +
                "var active=document.activeElement;" +
                "if(!active){return false;}" +
                "var tag=(active.tagName||'').toLowerCase();" +
                "return tag==='input'||tag==='textarea'||!!active.isContentEditable;" +
                "})()," +
                "stylesheetHrefs:Array.from(document.styleSheets||[]).map(function(sheet){return sheet.href||'inline';}).slice(0,4)" +
                "};" +
                "}" +
                ")()",
        ) { value ->
            Log.i(WEBUI_LAYOUT_TAG, value ?: "null")
            val verdict = WebUiStartupGate.evaluate(value)
            if (verdict.ready) {
                mainHandler.removeCallbacks(webUiLayoutRetry)
                dismissStartupOverlay {
                    requestAndroidComposerEntry(targetView)
                }
            } else if (webUiLayoutRetriesRemaining > 0 && !isFinishing) {
                webUiLayoutRetriesRemaining -= 1
                mainHandler.postDelayed(webUiLayoutRetry, WEBUI_LAYOUT_PROBE_RETRY_MS)
            } else {
                mainHandler.removeCallbacks(webUiLayoutRetry)
                showStartupError(verdict.status)
            }
        }
    }

    private fun requestAndroidComposerEntry(targetView: WebView) {
        targetView.requestFocus()
        targetView.evaluateJavascript(
            "(" +
                "function(){" +
                "try{" +
                "const bridge=window.__freehandOpenAndroidComposerForReadyHost;" +
                "if(typeof bridge==='function'){bridge();return true;}" +
                "return false;" +
                "}catch(error){return false;}" +
                "}" +
                ")()",
        ) { requested ->
            Log.i(WEBUI_LAYOUT_TAG, "android_composer_entry_requested=$requested")
            showAndroidImeAfterComposerEntry(targetView)
        }
    }

    private fun showAndroidImeAfterComposerEntry(targetView: WebView, attempt: Int = 0) {
        targetView.evaluateJavascript(
            "(" +
                "function(){" +
                "var active=document.activeElement;" +
                "if(!active){return false;}" +
                "var tag=(active.tagName||'').toLowerCase();" +
                "return tag==='input'||tag==='textarea'||!!active.isContentEditable;" +
                "}" +
                ")()",
        ) { focused ->
            val focusedEditable = focused == "true"
            val inputMethodManager =
                getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
            if (focusedEditable && inputMethodManager != null &&
                inputMethodManager.showSoftInput(targetView, InputMethodManager.SHOW_IMPLICIT)
            ) {
                return@evaluateJavascript
            }
            if (attempt < ANDROID_COMPOSER_IME_RETRY_ATTEMPTS) {
                mainHandler.postDelayed(
                    {
                        showAndroidImeAfterComposerEntry(targetView, attempt + 1)
                    },
                    ANDROID_COMPOSER_IME_RETRY_DELAY_MS,
                )
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

    private fun dismissStartupOverlay(onDismissed: (() -> Unit)? = null) {
        if (!::startupOverlay.isInitialized || startupOverlay.parent == null) {
            onDismissed?.invoke()
            return
        }
        startupStatus.text = "Workspace ready"
        startupOverlay.animate()
            .alpha(0f)
            .setDuration(180L)
            .withEndAction {
                startupAnimator?.cancel()
                startupAnimator = null
                (startupOverlay.parent as? FrameLayout)?.removeView(startupOverlay)
                webView.requestFocus()
                onDismissed?.invoke()
            }
            .start()
    }

    private fun showStartupError(message: String) {
        if (!::startupStatus.isInitialized || startupOverlay.parent == null) return
        startupStatus.text = message
        startupStatus.setTextColor(Color.rgb(203, 213, 225))
        promptEndpointConfigIfNeeded()
        showOpenConnectionsButtonIfNeeded()
    }

    private fun showOpenConnectionsButtonIfNeeded() {
        if (!::startupOverlay.isInitialized || startupOverlay.parent == null) return
        if (isRemoteRegistryConfig()) return
        if (startupOverlay.findViewById<View>(R.id.open_connections_button) != null) return
        val button = Button(this).apply {
            id = R.id.open_connections_button
            text = "打开连接配置"
            setOnClickListener { openConnectionsPage() }
        }
        val params = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT,
        ).apply {
            gravity = Gravity.CENTER_HORIZONTAL or Gravity.BOTTOM
            bottomMargin = dp(48)
        }
        startupOverlay.addView(button, params)
    }

    private fun openConnectionsPage() {
        connectionsLauncher.launch(ConnectionsActivity.changedIntent(this))
    }

    private fun promptEndpointConfigIfNeeded() {
        if (endpointConfigAutoPrompted || !::startupOverlay.isInitialized || startupOverlay.parent == null) return
        if (isRemoteRegistryConfig()) return
        endpointConfigAutoPrompted = true
        val current = currentHostConfig
        showEndpointConfigDialog(
            initialHost = current?.host ?: "",
            initialPort = current?.port ?: 0,
        )
    }

    private fun showEndpointConfigDialog(initialHost: String, initialPort: Int) {
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24), dp(16), dp(24), dp(0))
        }
        val hostInput = EditText(this).apply {
            hint = "Daemon host (e.g. 100.66.1.82)"
            setText(initialHost)
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI
        }
        val portInput = EditText(this).apply {
            hint = "Port (e.g. 4042)"
            setText(if (initialPort > 0) initialPort.toString() else "")
            inputType = android.text.InputType.TYPE_CLASS_NUMBER
        }
        container.addView(
            hostInput,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ),
        )
        container.addView(
            portInput,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(12) },
        )
        AlertDialog.Builder(this)
            .setTitle("Daemon 配置")
            .setMessage("配置 Freehand daemon 地址，保存后重新连接。")
            .setView(container)
            .setNegativeButton("取消", null)
            .setPositiveButton("保存") { _, _ ->
                val host = hostInput.text.toString().trim()
                val port = portInput.text.toString().trim().toIntOrNull()
                if (host.isNotEmpty() && port != null) {
                    saveEndpointConfig(host, port)
                } else {
                    showStartupError("配置无效：host 或 port 不能为空")
                }
            }
            .show()
    }

    private fun saveEndpointConfig(host: String, port: Int) {
        try {
            val store = ClientConfig.store(applicationContext)
            val config = store.load()
            val updated = config.updateActiveProfile(host, port)
            store.write(updated)
            val newHost = updated.activeHostConfig()
            endpointConfigAutoPrompted = false
            applyActiveHost(newHost)
        } catch (error: DaemonConnectionConfigException) {
            Log.e(LOG_TAG, "endpoint config save failed", error)
            showStartupError("配置保存失败：${error.message}")
        }
    }

    private fun applyActiveHost(host: HostConfig) {
        currentHostConfig = host
        lastApkUpdateStatus = null
        val updater = AndroidApkUpdater(applicationContext, host)
        apkUpdater = updater
        reloadWebUi(host)
        checkApkUpdateFor(updater)
    }

    private fun isRemoteRegistryConfig(): Boolean =
        try {
            ClientConfig.store(applicationContext).load().connectionMode == "remote_registry"
        } catch (_: DaemonConnectionConfigException) {
            false
        }

    private fun reloadWebUi(host: HostConfig) {
        if (::startupStatus.isInitialized && startupOverlay.parent != null) {
            startupStatus.text = "正在连接 ${host.baseUrl}"
            startupStatus.setTextColor(Color.rgb(163, 163, 163))
        }
        if (::webView.isInitialized) {
            webView.loadUrl(host.webUiUrl)
        }
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
        if (::webView.isInitialized) {
            mainHandler.removeCallbacks(webUiLayoutRetry)
            webView.destroy()
        }
        super.onDestroy()
    }

    companion object {
        private const val LOG_TAG = "FreehandAndroid"
        private const val WEBUI_LAYOUT_TAG = "FreehandWebUiLayout"
        private const val WEBUI_CONSOLE_TAG = "FreehandWebConsole"
        private const val WEBUI_ASSET_TAG = "FreehandWebAsset"
        private const val FILE_ACCESS_TAG = "FreehandFileAccess"
        private const val NOTIFICATION_TAG = "FreehandNotification"
        private const val TURN_FINISHED_CHANNEL_ID = "freehand_turn_finished"
        private const val WEBUI_LAYOUT_PROBE_RETRY_MS = 500L
        private const val WEBUI_LAYOUT_PROBE_RETRIES = 20
        private const val ANDROID_COMPOSER_IME_RETRY_ATTEMPTS = 10
        private const val ANDROID_COMPOSER_IME_RETRY_DELAY_MS = 200L
    }
}

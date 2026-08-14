package com.freehand.android.ui

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.util.Log
import androidx.core.content.FileProvider
import com.freehand.android.BuildConfig
import com.freehand.android.data.ApkUpdateManifest
import com.freehand.android.data.HostConfig
import java.io.File
import java.io.FileInputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

class AndroidApkUpdater(
    private val context: Context,
    private val hostConfig: HostConfig,
    private val currentVersionCode: Long = BuildConfig.VERSION_CODE.toLong(),
) {
    private val instanceGeneration = nextGeneration()

    val updateManifestUrl: String
        get() = hostConfig.updateManifestUrl

    private val checking = AtomicBoolean(false)

    fun checkForUpdateAsync(onStatus: (ApkUpdateStatus) -> Unit = {}) {
        if (!checking.compareAndSet(false, true)) {
            onStatus(ApkUpdateStatus.alreadyChecking())
            return
        }
        val checkGeneration = instanceGeneration
        fun emit(status: ApkUpdateStatus) {
            if (isGenerationCurrent(checkGeneration)) onStatus(status)
        }
        Thread {
            try {
                emit(ApkUpdateStatus.checking(updateManifestUrl))
                val manifestJson = httpGetText(updateManifestUrl)
                val plan = ApkUpdateManifest.parse(manifestJson).updatePlan(currentVersionCode, updateManifestUrl)
                if (plan == null) {
                    Log.i(LOG_TAG, "apk_update_current versionCode=$currentVersionCode")
                    emit(ApkUpdateStatus.current(currentVersionCode))
                    return@Thread
                }
                Log.i(LOG_TAG, "apk_update_available versionCode=${plan.versionCode} url=${plan.apkUrl}")
                emit(ApkUpdateStatus.available(plan.versionCode, plan.versionName, plan.apkUrl))
                emit(ApkUpdateStatus.downloading(plan.versionCode, plan.apkUrl))
                val apkFile = downloadApk(
                    plan.apkUrl,
                    plan.versionCode,
                    plan.sha256,
                    plan.size,
                    plan.signerSha256,
                )
                emit(ApkUpdateStatus.downloaded(plan.versionCode, apkFile.length()))
                val installIntent = buildInstallIntent(apkFile)
                runIfCurrent(checkGeneration) {
                    context.startActivity(installIntent)
                    Log.i(LOG_TAG, "apk_update_install_intent_started versionCode=${plan.versionCode}")
                    onStatus(ApkUpdateStatus.installerStarted(plan.versionCode, plan.versionName))
                }
            } catch (error: Exception) {
                Log.e(LOG_TAG, "apk_update_failed", error)
                emit(ApkUpdateStatus.failed(error))
            } finally {
                checking.set(false)
            }
        }.apply {
            name = "freehand-apk-update"
            isDaemon = true
            start()
        }
    }

    private fun downloadApk(
        apkUrl: String,
        versionCode: Long,
        expectedSha256: String,
        expectedSize: Long,
        expectedSignerSha256: String,
    ): File {
        val outputDir = File(
            context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS),
            "apk-updates",
        ).apply { mkdirs() }
        val output = File(outputDir, "freehand-android-$versionCode.apk")
        if (output.exists() && !output.delete()) {
            throw IllegalStateException("cannot clear previous apk cache file")
        }
        val downloadManager = context.getSystemService(Context.DOWNLOAD_SERVICE) as android.app.DownloadManager
        val request = android.app.DownloadManager.Request(Uri.parse(apkUrl)).apply {
            setAllowedOverMetered(true)
            setAllowedOverRoaming(true)
            setNotificationVisibility(android.app.DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
            setMimeType(APK_MIME_TYPE)
            setDestinationUri(Uri.fromFile(output))
            setTitle("Freehand APK update")
            setDescription("Downloading Freehand Android update")
        }
        val downloadId = downloadManager.enqueue(request)
        waitForDownload(downloadManager, downloadId)
        if (output.length() <= 0L) {
            throw IllegalStateException("apk download produced an empty file")
        }
        if (output.length() != expectedSize) {
            throw IllegalStateException(
                "apk size mismatch expected=$expectedSize actual=${output.length()}",
            )
        }
        val actualSha256 = sha256Hex(output)
        if (actualSha256 != expectedSha256) {
            throw IllegalStateException(
                "apk sha256 mismatch expected=$expectedSha256 actual=$actualSha256",
            )
        }
        verifyApkSigner(apkFile = output, expectedSignerSha256 = expectedSignerSha256)
        Log.i(LOG_TAG, "apk_hash_verified versionCode=$versionCode sha256=$actualSha256")
        return output
    }

    private fun verifyApkSigner(apkFile: File, expectedSignerSha256: String) {
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            PackageManager.GET_SIGNING_CERTIFICATES
        } else {
            @Suppress("DEPRECATION")
            PackageManager.GET_SIGNATURES
        }
        val packageInfo = context.packageManager.getPackageArchiveInfo(apkFile.absolutePath, flags)
            ?: throw IllegalStateException("cannot read APK package metadata")
        if (packageInfo.packageName != context.packageName) {
            throw IllegalStateException(
                "apk package name mismatch expected=${context.packageName} actual=${packageInfo.packageName}",
            )
        }
        val signatures = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            packageInfo.signingInfo?.apkContentsSigners
        } else {
            @Suppress("DEPRECATION")
            packageInfo.signatures
        }.orEmpty()
        if (signatures.size != 1) {
            throw IllegalStateException("APK must contain exactly one signer")
        }
        val actualSignerSha256 = sha256Hex(signatures.single().toByteArray())
        if (actualSignerSha256 != expectedSignerSha256) {
            throw IllegalStateException(
                "apk signerSha256 mismatch expected=$expectedSignerSha256 actual=$actualSignerSha256",
            )
        }
        Log.i(LOG_TAG, "apk_signer_verified signerSha256=$actualSignerSha256")
    }

    private fun sha256Hex(file: File): String {
        return FileInputStream(file).use { input ->
            sha256Hex(input)
        }
    }

    private fun sha256Hex(bytes: ByteArray): String {
        return sha256Hex(bytes.inputStream())
    }

    private fun sha256Hex(input: java.io.InputStream): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val buffer = ByteArray(8192)
        while (true) {
            val read = input.read(buffer)
            if (read <= 0) break
            digest.update(buffer, 0, read)
        }
        return digest.digest().joinToString("") { byte -> "%02x".format(byte) }
    }

    private fun waitForDownload(
        downloadManager: android.app.DownloadManager,
        downloadId: Long,
    ) {
        val query = android.app.DownloadManager.Query().setFilterById(downloadId)
        val deadline = System.currentTimeMillis() + HTTP_TIMEOUT_MS
        while (System.currentTimeMillis() < deadline) {
            val cursor = downloadManager.query(query)
            cursor.use {
                if (!it.moveToFirst()) {
                    throw IllegalStateException("apk download record missing")
                }
                when (it.getInt(it.getColumnIndexOrThrow(android.app.DownloadManager.COLUMN_STATUS))) {
                    android.app.DownloadManager.STATUS_SUCCESSFUL -> return
                    android.app.DownloadManager.STATUS_FAILED -> {
                        val reason = it.getInt(it.getColumnIndexOrThrow(android.app.DownloadManager.COLUMN_REASON))
                        throw IllegalStateException("apk download failed with reason $reason")
                    }
                    else -> Unit
                }
            }
            Thread.sleep(1000L)
        }
        throw IllegalStateException("apk download timed out after ${HTTP_TIMEOUT_MS}ms")
    }

    private fun buildInstallIntent(apkFile: File): Intent {
        val uri = FileProvider.getUriForFile(
            context,
            "${BuildConfig.APPLICATION_ID}.apkupdate.fileprovider",
            apkFile,
        )
        return Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, APK_MIME_TYPE)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                putExtra(Intent.EXTRA_NOT_UNKNOWN_SOURCE, true)
            }
        }
    }

    private fun httpGetText(url: String): String {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = HTTP_TIMEOUT_MS
        connection.readTimeout = HTTP_TIMEOUT_MS
        connection.requestMethod = "GET"
        connection.useCaches = false
        connection.setRequestProperty("Cache-Control", "no-cache")
        connection.setRequestProperty("Pragma", "no-cache")
        try {
            val status = connection.responseCode
            if (status !in 200..299) {
                throw IllegalStateException("manifest request failed with HTTP $status")
            }
            return connection.inputStream.bufferedReader().use { it.readText() }
        } finally {
            connection.disconnect()
        }
    }

    companion object {
        private val GENERATION_LOCK = Any()
        private var generation = 0L
        internal fun nextGeneration(): Long = synchronized(GENERATION_LOCK) { ++generation }
        internal fun isGenerationCurrent(checkGeneration: Long): Boolean =
            synchronized(GENERATION_LOCK) { generation == checkGeneration }
        internal fun runIfCurrent(checkGeneration: Long, action: () -> Unit) {
            synchronized(GENERATION_LOCK) {
                if (generation == checkGeneration) action()
            }
        }
        private const val LOG_TAG = "FreehandApkUpdate"
        private const val HTTP_TIMEOUT_MS = 60000
        private const val APK_MIME_TYPE = "application/vnd.android.package-archive"
    }
}

data class ApkUpdateStatus(
    val phase: String,
    val message: String,
    val versionCode: Long? = null,
    val versionName: String? = null,
    val apkUrl: String? = null,
    val bytes: Long? = null,
) {
    companion object {
        fun checking(manifestUrl: String) = ApkUpdateStatus(
            phase = "checking",
            message = "Checking update manifest at $manifestUrl",
        )

        fun current(currentVersionCode: Long) = ApkUpdateStatus(
            phase = "current",
            message = "Current APK is up to date at versionCode=$currentVersionCode",
            versionCode = currentVersionCode,
        )

        fun available(versionCode: Long, versionName: String?, apkUrl: String) = ApkUpdateStatus(
            phase = "available",
            message = "Update available versionCode=$versionCode",
            versionCode = versionCode,
            versionName = versionName,
            apkUrl = apkUrl,
        )

        fun downloading(versionCode: Long, apkUrl: String) = ApkUpdateStatus(
            phase = "downloading",
            message = "Downloading APK versionCode=$versionCode",
            versionCode = versionCode,
            apkUrl = apkUrl,
        )

        fun downloaded(versionCode: Long, bytes: Long) = ApkUpdateStatus(
            phase = "downloaded",
            message = "Downloaded APK versionCode=$versionCode bytes=$bytes",
            versionCode = versionCode,
            bytes = bytes,
        )

        fun installerStarted(versionCode: Long, versionName: String?) = ApkUpdateStatus(
            phase = "installer_started",
            message = "Android package installer opened for versionCode=$versionCode",
            versionCode = versionCode,
            versionName = versionName,
        )

        fun failed(error: Throwable) = ApkUpdateStatus(
            phase = "failed",
            message = error.message ?: error.javaClass.simpleName,
        )

        fun alreadyChecking() = ApkUpdateStatus(
            phase = "already_checking",
            message = "APK update check is already running",
        )
    }
}

package com.freehand.android.data

import com.google.gson.JsonParseException
import com.google.gson.JsonParser
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.HttpUrl.Companion.toHttpUrl
import java.io.File

data class ApkUpdateManifest(
    val versionCode: Long,
    val versionName: String,
    val apkUrl: String,
    val releaseNotes: String,
    val required: Boolean,
) {
    fun isNewerThan(currentVersionCode: Long): Boolean = versionCode > currentVersionCode

    companion object {
        fun parse(json: String): ApkUpdateManifest {
            val root = try {
                JsonParser.parseString(json).asJsonObject
            } catch (e: JsonParseException) {
                throw ApkUpdateException("invalid update manifest json: ${e.message}", e)
            } catch (e: IllegalStateException) {
                throw ApkUpdateException("invalid update manifest json: ${e.message}", e)
            }
            return ApkUpdateManifest(
                versionCode = root.requiredLong("versionCode"),
                versionName = root.requiredString("versionName"),
                apkUrl = root.requiredString("apkUrl"),
                releaseNotes = root.optionalString("releaseNotes", ""),
                required = root.optionalBoolean("required", false),
            ).validated()
        }

        private fun ApkUpdateManifest.validated(): ApkUpdateManifest {
            if (versionCode <= 0) throw ApkUpdateException("update versionCode must be positive")
            if (versionName.isBlank()) throw ApkUpdateException("update versionName is required")
            if (!apkUrl.startsWith("http://") && !apkUrl.startsWith("https://") && !apkUrl.startsWith("/")) {
                throw ApkUpdateException("update apkUrl must be http(s) or absolute path")
            }
            return this
        }

        private fun com.google.gson.JsonObject.requiredString(field: String): String {
            val value = get(field)
            if (value == null || value.isJsonNull || value.asString.isBlank()) {
                throw ApkUpdateException("update $field is required")
            }
            return value.asString
        }

        private fun com.google.gson.JsonObject.requiredLong(field: String): Long {
            val value = get(field)
            if (value == null || value.isJsonNull) {
                throw ApkUpdateException("update $field is required")
            }
            return value.asLong
        }

        private fun com.google.gson.JsonObject.optionalString(field: String, default: String): String {
            val value = get(field)
            return if (value == null || value.isJsonNull) default else value.asString
        }

        private fun com.google.gson.JsonObject.optionalBoolean(field: String, default: Boolean): Boolean {
            val value = get(field)
            return if (value == null || value.isJsonNull) default else value.asBoolean
        }
    }
}

data class ApkUpdateCheckResult(
    val manifest: ApkUpdateManifest?,
    val updateAvailable: Boolean,
)

class ApkUpdateClient(
    private val httpClient: OkHttpClient,
    private val currentVersionCode: Long,
) {
    fun check(manifestUrl: String): ApkUpdateCheckResult {
        val request = Request.Builder().url(manifestUrl).get().build()
        httpClient.newCall(request).execute().use { response ->
            if (response.code == 204 || response.code == 404) {
                return ApkUpdateCheckResult(manifest = null, updateAvailable = false)
            }
            if (!response.isSuccessful) {
                throw ApkUpdateException("update check failed: http ${response.code}")
            }
            val body = response.body?.string()
                ?: throw ApkUpdateException("update manifest response was empty")
            val manifest = ApkUpdateManifest.parse(body).withResolvedApkUrl(manifestUrl)
            return ApkUpdateCheckResult(
                manifest = manifest,
                updateAvailable = manifest.isNewerThan(currentVersionCode),
            )
        }
    }

    private fun ApkUpdateManifest.withResolvedApkUrl(manifestUrl: String): ApkUpdateManifest {
        if (apkUrl.startsWith("http://") || apkUrl.startsWith("https://")) return this
        val resolved = manifestUrl.toHttpUrl().resolve(apkUrl)
            ?: throw ApkUpdateException("update apkUrl cannot be resolved")
        return copy(apkUrl = resolved.toString())
    }

    fun download(manifest: ApkUpdateManifest, targetFile: File): File {
        val request = Request.Builder().url(manifest.apkUrl).get().build()
        httpClient.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                throw ApkUpdateException("apk download failed: http ${response.code}")
            }
            val body = response.body ?: throw ApkUpdateException("apk download response was empty")
            targetFile.parentFile?.mkdirs()
            targetFile.outputStream().use { output ->
                body.byteStream().use { input -> input.copyTo(output) }
            }
        }
        if (targetFile.length() <= 0L) {
            throw ApkUpdateException("downloaded apk is empty")
        }
        return targetFile
    }
}

class ApkUpdateException(
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause)

package com.freehand.android.data

import com.google.gson.JsonObject
import com.google.gson.JsonParseException
import com.google.gson.JsonParser
import java.net.URI

data class ApkUpdateManifest(
    val versionCode: Long,
    val versionName: String,
    val apkUrl: String,
    val required: Boolean,
    val releaseNotes: String,
) {
    fun updatePlan(currentVersionCode: Long, hostConfig: HostConfig): ApkUpdatePlan? {
        if (versionCode <= currentVersionCode) return null
        return ApkUpdatePlan(
            versionCode = versionCode,
            versionName = versionName,
            apkUrl = hostConfig.resolveDaemonUrl(apkUrl),
            required = required,
            releaseNotes = releaseNotes,
        )
    }

    companion object {
        fun parse(json: String): ApkUpdateManifest {
            val root = try {
                JsonParser.parseString(json).asJsonObject
            } catch (error: JsonParseException) {
                throw ApkUpdateManifestException("invalid apk update manifest json: ${error.message}", error)
            } catch (error: IllegalStateException) {
                throw ApkUpdateManifestException("invalid apk update manifest json: ${error.message}", error)
            }
            root.rejectUnknownFields(
                setOf("versionCode", "versionName", "apkUrl", "releaseNotes", "required"),
            )
            val versionCode = root.requiredLong("versionCode")
            if (versionCode <= 0L) {
                throw ApkUpdateManifestException("versionCode must be positive")
            }
            val apkUrl = root.requiredString("apkUrl")
            validateApkUrl(apkUrl)
            return ApkUpdateManifest(
                versionCode = versionCode,
                versionName = root.optionalString("versionName") ?: "",
                apkUrl = apkUrl,
                required = root.optionalBoolean("required") ?: false,
                releaseNotes = root.optionalString("releaseNotes") ?: "",
            )
        }

        private fun validateApkUrl(value: String) {
            val uri = try {
                URI(value)
            } catch (error: Exception) {
                throw ApkUpdateManifestException("apkUrl must be a relative path or http(s) URL", error)
            }
            if (uri.isAbsolute && uri.scheme != "http" && uri.scheme != "https") {
                throw ApkUpdateManifestException("apkUrl absolute URL scheme must be http(s)")
            }
        }

        private fun JsonObject.rejectUnknownFields(allowed: Set<String>) {
            val unknown = keySet().filterNot(allowed::contains).sorted()
            if (unknown.isNotEmpty()) {
                throw ApkUpdateManifestException(
                    "apk update manifest contains unsupported fields: ${unknown.joinToString(", ")}",
                )
            }
        }

        private fun JsonObject.requiredString(field: String): String {
            val value = get(field)
            if (value == null || value.isJsonNull || !value.isJsonPrimitive || value.asString.isBlank()) {
                throw ApkUpdateManifestException("$field is required")
            }
            return value.asString.trim()
        }

        private fun JsonObject.optionalString(field: String): String? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            return value.asString.trim().takeIf { it.isNotBlank() }
        }

        private fun JsonObject.requiredLong(field: String): Long {
            val value = get(field)
            if (value == null || value.isJsonNull || !value.isJsonPrimitive) {
                throw ApkUpdateManifestException("$field is required")
            }
            return value.asLong
        }

        private fun JsonObject.optionalBoolean(field: String): Boolean? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            return value.asBoolean
        }
    }
}

data class ApkUpdatePlan(
    val versionCode: Long,
    val versionName: String,
    val apkUrl: String,
    val required: Boolean,
    val releaseNotes: String,
)

class ApkUpdateManifestException(
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause)

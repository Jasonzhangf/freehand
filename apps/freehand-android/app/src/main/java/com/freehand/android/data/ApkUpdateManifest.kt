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
    val sha256: String? = null,
    val size: Long? = null,
    val signerSha256: String? = null,
) {
    fun updatePlan(currentVersionCode: Long, manifestUrl: String): ApkUpdatePlan? {
        if (versionCode <= currentVersionCode) return null
        val verifiedSha256 = sha256
            ?: throw ApkUpdateManifestException("sha256 is required for a higher-version APK")
        val verifiedSize = size
            ?: throw ApkUpdateManifestException("size is required for a higher-version APK")
        val verifiedSignerSha256 = signerSha256
            ?: throw ApkUpdateManifestException("signerSha256 is required for a higher-version APK")
        return ApkUpdatePlan(
            versionCode = versionCode,
            versionName = versionName,
            apkUrl = resolveManifestUrl(manifestUrl, apkUrl),
            required = required,
            releaseNotes = releaseNotes,
            sha256 = verifiedSha256,
            size = verifiedSize,
            signerSha256 = verifiedSignerSha256,
        )
    }

    private fun resolveManifestUrl(manifestUrl: String, apkUrl: String): String {
        val resolved = URI(manifestUrl).resolve(apkUrl)
        if (resolved.scheme != "http" && resolved.scheme != "https") {
            throw ApkUpdateManifestException("resolved apkUrl scheme must be http(s)")
        }
        val manifestUri = URI(manifestUrl)
        val relayRoot = manifestUri.rawPath
            ?.takeIf { it.endsWith("/updates/latest.json") }
            ?.substringBefore("/updates/latest.json")
            .orEmpty()
        val resolvedPath = resolved.rawPath.orEmpty()
        val nestedPath = resolvedPath.removePrefix("/relay")
        if (resolvedPath.startsWith("/relay/") && relayRoot.isNotEmpty() && nestedPath.isNotEmpty()) {
            return URI(
                resolved.scheme,
                resolved.rawAuthority,
                "$relayRoot$nestedPath",
                resolved.rawQuery,
                resolved.rawFragment,
            ).toString()
        }
        return resolved.toString()
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
                setOf(
                    "versionCode",
                    "versionName",
                    "apkUrl",
                    "releaseNotes",
                    "required",
                    "sha256",
                    "size",
                    "signerSha256",
                ),
            )
            val versionCode = root.requiredLong("versionCode")
            if (versionCode <= 0L) {
                throw ApkUpdateManifestException("versionCode must be positive")
            }
            val apkUrl = root.requiredString("apkUrl")
            validateApkUrl(apkUrl)
            val sha256 = root.optionalSha256("sha256")
            val size = root.optionalPositiveLong("size")
            val signerSha256 = root.optionalSha256("signerSha256")
            return ApkUpdateManifest(
                versionCode = versionCode,
                versionName = root.optionalString("versionName") ?: "",
                apkUrl = apkUrl,
                required = root.optionalBoolean("required") ?: false,
                releaseNotes = root.optionalString("releaseNotes") ?: "",
                sha256 = sha256,
                size = size,
                signerSha256 = signerSha256,
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

        private fun JsonObject.optionalSha256(field: String): String? {
            val value = optionalString(field) ?: return null
            val normalized = value.lowercase()
            if (!SHA256_HEX.matches(normalized)) {
                throw ApkUpdateManifestException("$field must be a 64-char lowercase hex digest")
            }
            return normalized
        }

        private fun JsonObject.optionalPositiveLong(field: String): Long? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            if (!value.isJsonPrimitive) {
                throw ApkUpdateManifestException("$field must be a positive integer")
            }
            val parsed = value.asLong
            if (parsed <= 0L) {
                throw ApkUpdateManifestException("$field must be positive")
            }
            return parsed
        }

        private val SHA256_HEX = Regex("^[0-9a-f]{64}$")
    }
}

data class ApkUpdatePlan(
    val versionCode: Long,
    val versionName: String,
    val apkUrl: String,
    val required: Boolean,
    val releaseNotes: String,
    val sha256: String,
    val size: Long,
    val signerSha256: String,
)

class ApkUpdateManifestException(
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause)

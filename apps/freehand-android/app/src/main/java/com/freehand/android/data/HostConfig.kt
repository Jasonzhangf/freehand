package com.freehand.android.data

import java.net.URI

data class HostConfig(
    val host: String,
    val port: Int,
    val webUrlOverride: String? = null,
) {
    val baseUrl: String get() = webUrlOverride ?: "http://$host:$port"
    val webUiUrl: String get() = appendWebUiParams(baseUrl)
    val updateManifestUrl: String get() = resolveDaemonUrl("android/update.json")

    fun resolveDaemonUrl(pathOrUrl: String): String {
        val trimmed = pathOrUrl.trim()
        val parsed = URI(trimmed)
        if (parsed.isAbsolute) return trimmed
        return URI(ensureTrailingSlash(baseUrl)).resolve(trimmed.removePrefix("/")).toString()
    }

    private companion object {
        fun appendWebUiParams(url: String): String {
            val normalized = ensureTrailingSlash(url)
            val separator = if (normalized.contains("?")) "&" else "?"
            return "$normalized${separator}client=android-webview"
        }

        fun ensureTrailingSlash(url: String): String {
            if (url.contains("?")) return url
            return if (url.substringAfter("://", "").contains("/")) url else "$url/"
        }
    }
}

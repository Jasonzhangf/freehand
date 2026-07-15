package com.freehand.android.data

data class HostConfig(
    val host: String,
    val port: Int,
) {
    val baseUrl: String get() = "http://$host:$port"
    val webUiUrl: String get() = "$baseUrl/?client=android-webview&v=$WEBUI_BOOTSTRAP_VERSION"

    private companion object {
        const val WEBUI_BOOTSTRAP_VERSION = "20260715-agent-dashboard-2"
    }
}

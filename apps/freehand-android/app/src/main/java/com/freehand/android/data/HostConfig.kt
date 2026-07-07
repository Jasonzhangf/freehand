package com.freehand.android.data

data class HostConfig(
    val host: String,
    val port: Int,
    val profileId: String = "tailscale-main",
    val mode: String = "tailscale",
    val adpPath: String = "/adp",
    val healthPath: String = "/health",
    val commandPath: String = "/ui/command",
    val queryPath: String = "/ui/query/latest-active-turn",
    val subscribePath: String = "/ui/subscribe/turn/latest",
) {
    val baseUrl: String get() = "http://$host:$port"
    val adpUrl: String get() = "ws://$host:$port$adpPath"
    val healthUrl: String get() = "$baseUrl$healthPath"
    val commandUrl: String get() = "$baseUrl$commandPath"
    val latestTurnUrl: String get() = "$baseUrl$queryPath"
    val latestTurnSseUrl: String get() = "$baseUrl$subscribePath"
    val updateManifestUrl: String get() = "$baseUrl/android/update.json"
    fun debugSnapshotUrl(turnId: String): String = "$baseUrl/ui/query/debug/$turnId"
    fun debugSnapshotSseUrl(turnId: String): String = "$baseUrl/ui/subscribe/debug/$turnId"

    val endpointLabel: String get() = "$profileId $mode $host:$port$adpPath"
}

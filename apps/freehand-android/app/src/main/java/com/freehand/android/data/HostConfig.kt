package com.freehand.android.data

data class HostConfig(
    val host: String,
    val port: Int,
) {
    val baseUrl: String get() = "http://$host:$port"
    val commandUrl: String get() = "$baseUrl/ui/command"
    val latestTurnUrl: String get() = "$baseUrl/ui/query/turn/latest"
    val latestTurnSseUrl: String get() = "$baseUrl/ui/subscribe/turn/latest"
    val debugSnapshotUrl: String get() = "$baseUrl/ui/query/debug/latest"
    val debugSnapshotSseUrl: String get() = "$baseUrl/ui/subscribe/debug/latest"
}

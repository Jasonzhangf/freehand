package com.freehand.android.data

data class HostConfig(
    val host: String,
    val port: Int,
) {
    val baseUrl: String get() = "http://$host:$port"
    val commandUrl: String get() = "$baseUrl/ui/command"
    val latestTurnUrl: String get() = "$baseUrl/ui/query/latest-active-turn"
    val latestTurnSseUrl: String get() = "$baseUrl/ui/subscribe/turn/latest"
    fun debugSnapshotUrl(turnId: String): String = "$baseUrl/ui/query/debug/$turnId"
    fun debugSnapshotSseUrl(turnId: String): String = "$baseUrl/ui/subscribe/debug/$turnId"
}

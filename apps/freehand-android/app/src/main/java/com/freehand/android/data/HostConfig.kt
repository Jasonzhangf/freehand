package com.freehand.android.data

data class HostConfig(
    val host: String,
    val port: Int,
) {
    val baseUrl: String get() = "http://$host:$port"
}

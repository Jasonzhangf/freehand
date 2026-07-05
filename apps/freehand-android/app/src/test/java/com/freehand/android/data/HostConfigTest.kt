package com.freehand.android.data

import org.junit.Assert.*
import org.junit.Test

class HostConfigTest {

    @Test
    fun `baseUrl constructs correct URL`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("http://100.66.1.82:4041", config.baseUrl)
    }

    @Test
    fun `commandUrl points to ui-command`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("http://100.66.1.82:4041/ui/command", config.commandUrl)
    }

    @Test
    fun `adpUrl points to daemon ADP websocket`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("ws://100.66.1.82:4041/adp", config.adpUrl)
    }

    @Test
    fun `healthUrl points to selected profile health path`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("http://100.66.1.82:4041/health", config.healthUrl)
    }

    @Test
    fun `latestTurnUrl points to query endpoint`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("http://100.66.1.82:4041/ui/query/latest-active-turn", config.latestTurnUrl)
    }

    @Test
    fun `latestTurnSseUrl points to subscribe endpoint`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals("http://100.66.1.82:4041/ui/subscribe/turn/latest", config.latestTurnSseUrl)
    }

    @Test
    fun `different port produces correct URL`() {
        val config = HostConfig("127.0.0.1", 8080)
        assertEquals("http://127.0.0.1:8080", config.baseUrl)
        assertEquals("http://127.0.0.1:8080/ui/command", config.commandUrl)
        assertEquals("ws://127.0.0.1:8080/adp", config.adpUrl)
    }

    @Test
    fun `custom profile paths produce correct endpoint URLs`() {
        val config = HostConfig(
            host = "freehand-tailnet",
            port = 4042,
            profileId = "dev-s",
            mode = "tailscale",
            adpPath = "/custom-adp",
            healthPath = "/custom-health",
            commandPath = "/custom-command",
            queryPath = "/custom-query",
            subscribePath = "/custom-subscribe",
        )
        assertEquals("ws://freehand-tailnet:4042/custom-adp", config.adpUrl)
        assertEquals("http://freehand-tailnet:4042/custom-health", config.healthUrl)
        assertEquals("http://freehand-tailnet:4042/custom-command", config.commandUrl)
        assertEquals("http://freehand-tailnet:4042/custom-query", config.latestTurnUrl)
        assertEquals("http://freehand-tailnet:4042/custom-subscribe", config.latestTurnSseUrl)
        assertEquals("dev-s tailscale freehand-tailnet:4042/custom-adp", config.endpointLabel)
    }

    @Test
    fun `debug urls require explicit turn id and match server routes`() {
        val config = HostConfig("100.66.1.82", 4041)
        assertEquals(
            "http://100.66.1.82:4041/ui/query/debug/turn-1",
            config.debugSnapshotUrl("turn-1"),
        )
        assertEquals(
            "http://100.66.1.82:4041/ui/subscribe/debug/turn-1",
            config.debugSnapshotSseUrl("turn-1"),
        )
    }
}

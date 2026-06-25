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
    }
}

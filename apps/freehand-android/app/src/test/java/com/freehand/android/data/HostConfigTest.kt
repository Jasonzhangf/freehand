package com.freehand.android.data

import org.junit.Assert.assertEquals
import org.junit.Test

class HostConfigTest {
    @Test
    fun `baseUrl constructs canonical daemon origin`() {
        assertEquals(
            "http://100.66.1.82:4041",
            HostConfig("100.66.1.82", 4041).baseUrl,
        )
    }

    @Test
    fun `different host and port produce their exact daemon origin`() {
        assertEquals(
            "http://freehand-tailnet:4042",
            HostConfig("freehand-tailnet", 4042).baseUrl,
        )
    }

    @Test
    fun `webUiUrl loads the canonical Android WebView shell`() {
        assertEquals(
            "http://100.66.1.82:4041/?client=android-webview",
            HostConfig("100.66.1.82", 4041).webUiUrl,
        )
    }

    @Test
    fun `relay web url override still receives Android WebView shell params`() {
        assertEquals(
            "https://relay.freehand.local/daemon/studio/web?client=android-webview",
            HostConfig(
                host = "relay.freehand.local",
                port = 443,
                webUrlOverride = "https://relay.freehand.local/daemon/studio/web",
            ).webUiUrl,
        )
    }

    @Test
    fun `updateManifestUrl uses daemon android route`() {
        assertEquals(
            "http://100.66.1.82:4041/android/update.json",
            HostConfig("100.66.1.82", 4041).updateManifestUrl,
        )
    }

    @Test
    fun `relay updateManifestUrl uses account relay root`() {
        assertEquals(
            "http://100.66.1.82:44042/relay/updates/latest.json",
            HostConfig(
                host = "100.66.1.82",
                port = 44042,
                webUrlOverride = "http://100.66.1.82:44042/relay/daemon/studio-host/",
                relayUpdateManifestUrl = "http://100.66.1.82:44042/relay/updates/latest.json",
            ).updateManifestUrl,
        )
    }
}

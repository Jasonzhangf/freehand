package com.freehand.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.File
import java.nio.file.Files
import java.nio.charset.StandardCharsets
import java.util.Base64

class DaemonConnectionConfigTest {

    @Test
    fun `bundled config parses as tailscale active profile`() {
        val config = DaemonConnectionConfig.parse(readBundledConfig())
        val host = config.activeHostConfig()

        assertEquals("tailscale", config.connectionMode)
        assertEquals("tailscale-main", config.activeProfile)
        assertEquals("100.66.1.82", host.host)
        assertEquals(4041, host.port)
    }

    @Test
    fun `store bootstraps bundled config into app owned json file`() {
        val dir = Files.createTempDirectory("freehand-android-config").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }

        val config = store.load()

        assertTrue(configFile.exists())
        assertEquals("tailscale-main", config.activeProfile)
        assertEquals("tailscale-main", DaemonConnectionConfig.parse(configFile.readText()).activeProfile)
    }

    @Test
    fun `edited tailscale profile round trips through app owned json file`() {
        val dir = Files.createTempDirectory("freehand-android-config-edit").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }
        val edited = store.load().copy(
            profiles = listOf(
                DaemonConnectionProfile(
                    id = "tailscale-main",
                    mode = "tailscale",
                    host = "freehand-tailnet",
                    port = 4042,
                ),
            ),
        )

        store.write(edited)
        val reloaded = store.load().activeHostConfig()

        assertEquals("freehand-tailnet", reloaded.host)
        assertEquals(4042, reloaded.port)
    }

    @Test
    fun `removed native transport fields fail explicitly`() {
        val json = """
            {
              "connectionMode": "tailscale",
              "activeProfile": "tailscale-main",
              "profiles": [
                {
                  "id": "tailscale-main",
                  "mode": "tailscale",
                  "host": "100.66.1.82",
                  "port": 4042,
                  "adpPath": "/adp",
                  "healthPath": "/health",
                  "commandPath": "/ui/command",
                  "queryPath": "/ui/query/latest-active-turn",
                  "subscribePath": "/ui/subscribe/turn/latest"
                }
              ]
            }
        """.trimIndent()

        val error = expectConfigError { DaemonConnectionConfig.parse(json) }

        assertTrue(error.message.orEmpty().contains("unsupported fields"))
        assertTrue(error.message.orEmpty().contains("adpPath"))
    }

    @Test
    fun `store preserves user edited tailscale port`() {
        val dir = Files.createTempDirectory("freehand-android-config-user-port").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        configFile.writeText(
            """
            {
              "connectionMode": "tailscale",
              "activeProfile": "tailscale-main",
              "profiles": [
                {
                  "id": "tailscale-main",
                  "mode": "tailscale",
                  "host": "custom-tailnet",
                  "port": 4042
                }
              ]
            }
            """.trimIndent(),
        )
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }

        val host = store.load().activeHostConfig()

        assertEquals("custom-tailnet", host.host)
        assertEquals(4042, host.port)
    }

    @Test
    fun `malformed existing app owned config fails explicitly`() {
        val dir = Files.createTempDirectory("freehand-android-config-bad").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        configFile.writeText("""{"connectionMode":"tailscale","profiles":[]}""")
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }

        val error = expectConfigError { store.load() }

        assertTrue(error.message.orEmpty().contains("activeProfile is required"))
    }

    @Test
    fun `removed relay routing field fails explicitly`() {
        val json = """
            {
              "connectionMode": "tailscale",
              "activeProfile": "tailscale-main",
              "profiles": [
                {
                  "id": "tailscale-main",
                  "mode": "tailscale",
                  "host": "100.66.1.82",
                  "port": 4041
                }
              ],
              "relay": { "enabled": true, "url": "https://relay.invalid", "authRef": "secret" }
            }
        """.trimIndent()

        val error = expectConfigError { DaemonConnectionConfig.parse(json) }

        assertTrue(error.message.orEmpty().contains("unsupported fields"))
        assertTrue(error.message.orEmpty().contains("relay"))
    }

    @Test
    fun `missing active profile is an explicit config error`() {
        val json = """
            {
              "connectionMode": "tailscale",
              "activeProfile": "missing-profile",
              "profiles": [
                {
                  "id": "tailscale-main",
                  "mode": "tailscale",
                  "host": "100.66.1.82",
                  "port": 4041
                }
              ]
            }
        """.trimIndent()

        val error = expectConfigError { DaemonConnectionConfig.parse(json) }

        assertTrue(error.message.orEmpty().contains("active profile"))
    }

    @Test
    fun `remote registry selects active tailscale daemon endpoint`() {
        val config = DaemonConnectionConfig.parse(
            """
            {
              "schemaVersion": 1,
              "connectionMode": "remote_registry",
              "activeAccount": "jason",
              "activeDaemon": "studio",
              "accounts": [
                {
                  "id": "jason",
                  "label": "Jason",
                  "relayUrl": "https://relay.freehand.local/relay/"
                }
              ],
              "daemons": [
                {
                  "id": "studio",
                  "accountId": "jason",
                  "label": "Mac Studio",
                  "nodeId": "studio-node",
                  "activeEndpoint": "tailscale-main",
                  "endpoints": [
                    {
                      "id": "tailscale-main",
                      "kind": "tailscale",
                      "host": "100.66.1.82",
                      "port": 4042
                    },
                    {
                      "id": "relay-web",
                      "kind": "relay",
                      "webUrl": "https://relay.freehand.local/daemon/studio/web",
                      "relayHostId": "studio-host"
                    }
                  ]
                }
              ]
            }
            """.trimIndent(),
        )

        val host = config.activeHostConfig()

        assertEquals("remote_registry", config.connectionMode)
        assertEquals("100.66.1.82", host.host)
        assertEquals(4042, host.port)
        assertEquals(1, config.accounts.size)
        assertEquals(1, config.daemons.size)
    }

    @Test
    fun `remote registry relay endpoint loads explicit relay WebUI url`() {
        val config = DaemonConnectionConfig.parse(
            """
            {
              "connectionMode": "remote_registry",
              "activeAccount": "jason",
              "activeDaemon": "studio",
              "accounts": [
                {
                  "id": "jason",
                  "relayUrl": "https://relay.freehand.local/relay/"
                }
              ],
              "daemons": [
                {
                  "id": "studio",
                  "accountId": "jason",
                  "nodeId": "studio-node",
                  "activeEndpoint": "relay-web",
                  "endpoints": [
                    {
                      "id": "relay-web",
                      "kind": "relay",
                      "webUrl": "https://relay.freehand.local/daemon/studio/web"
                    }
                  ]
                }
              ]
            }
            """.trimIndent(),
        )

        val host = config.activeHostConfig()

        assertEquals("relay.freehand.local", host.host)
        assertEquals(443, host.port)
        assertEquals(
            "https://relay.freehand.local/daemon/studio/web?client=android-webview",
            host.webUiUrl,
        )
    }

    @Test
    fun `remote registry missing active endpoint fails explicitly`() {
        val json = """
            {
              "connectionMode": "remote_registry",
              "activeAccount": "jason",
              "activeDaemon": "studio",
              "accounts": [{ "id": "jason" }],
              "daemons": [
                {
                  "id": "studio",
                  "accountId": "jason",
                  "nodeId": "studio-node",
                  "activeEndpoint": "missing",
                  "endpoints": [
                    {
                      "id": "tailscale-main",
                      "kind": "tailscale",
                      "host": "100.66.1.82",
                      "port": 4042
                    }
                  ]
                }
              ]
            }
        """.trimIndent()

        val error = expectConfigError { DaemonConnectionConfig.parse(json) }

        assertTrue(error.message.orEmpty().contains("active endpoint"))
    }

    @Test
    fun `remote registry relay endpoint requires account relay url`() {
        val json = """
            {
              "connectionMode": "remote_registry",
              "activeAccount": "jason",
              "activeDaemon": "studio",
              "accounts": [{ "id": "jason" }],
              "daemons": [
                {
                  "id": "studio",
                  "accountId": "jason",
                  "nodeId": "studio-node",
                  "activeEndpoint": "relay-web",
                  "endpoints": [
                    {
                      "id": "relay-web",
                      "kind": "relay",
                      "webUrl": "https://relay.freehand.local/daemon/studio/web"
                    }
                  ]
                }
              ]
            }
        """.trimIndent()

        val error = expectConfigError { DaemonConnectionConfig.parse(json) }

        assertTrue(error.message.orEmpty().contains("requires account 'jason' relayUrl"))
    }

    @Test
    fun `bootstrap link imports account daemon endpoint and one time credential`() {
        val link = buildBootstrapLink(expiresAtUnix = 200)

        val config = DaemonConnectionConfig.parseBootstrapLink(link, nowUnix = 100)
        val host = config.activeHostConfig()

        assertEquals("remote_registry", config.connectionMode)
        assertEquals("jason", config.activeAccount)
        assertEquals("studio", config.activeDaemon)
        assertEquals("one-time-secret", config.accounts.first().authToken)
        assertEquals("relay.freehand.local", host.host)
        assertEquals(443, host.port)
    }

    @Test
    fun `store imports bootstrap link into app owned config file`() {
        val dir = Files.createTempDirectory("freehand-android-bootstrap").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }

        store.importBootstrapLink(buildBootstrapLink(expiresAtUnix = 200), nowUnix = 100)

        assertTrue(configFile.exists())
        assertTrue(File(dir, DaemonConnectionConfig.REMOTE_REGISTRY_CONFIG_FILE).exists())
        val reloaded = store.load()
        assertEquals("remote_registry", reloaded.connectionMode)
        assertEquals("studio", reloaded.activeDaemon)
        assertEquals(
            "tailscale",
            DaemonConnectionConfig.parse(configFile.readText()).connectionMode,
        )
        assertEquals(
            "remote_registry",
            DaemonConnectionConfig.parse(File(dir, DaemonConnectionConfig.REMOTE_REGISTRY_CONFIG_FILE).readText()).connectionMode,
        )
    }

    @Test
    fun `expired bootstrap link fails explicitly`() {
        val error = expectConfigError {
            DaemonConnectionConfig.parseBootstrapLink(buildBootstrapLink(expiresAtUnix = 100), nowUnix = 100)
        }

        assertTrue(error.message.orEmpty().contains("expired"))
    }

    @Test
    fun `load prefers remote registry sidecar over legacy compatibility config`() {
        val dir = Files.createTempDirectory("freehand-android-bootstrap-sidecar").toFile()
        val configFile = File(dir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE)
        val registryFile = File(dir, DaemonConnectionConfig.REMOTE_REGISTRY_CONFIG_FILE)
        val store = DaemonConnectionConfigStore(configFile) { readBundledConfig() }
        val bootstrapConfig = DaemonConnectionConfig.parseBootstrapLink(buildBootstrapLink(expiresAtUnix = 200), nowUnix = 100)
        store.write(bootstrapConfig)

        registryFile.writeText(
            """
            {
              "schemaVersion": 1,
              "connectionMode": "remote_registry",
              "activeProfile": "",
              "profiles": [],
              "activeAccount": "jason",
              "activeDaemon": "studio",
              "accounts": [
                {
                  "id": "jason",
                  "label": "Jason",
                  "relayUrl": "https://relay.freehand.local/relay/",
                  "authToken": "updated-token"
                }
              ],
              "daemons": [
                {
                  "id": "studio",
                  "accountId": "jason",
                  "label": "Mac Studio",
                  "nodeId": "studio-node",
                  "activeEndpoint": "tailscale-main",
                  "endpoints": [
                    {
                      "id": "tailscale-main",
                      "kind": "tailscale",
                      "host": "100.66.1.83",
                      "port": 4043
                    }
                  ]
                }
              ]
            }
            """.trimIndent(),
        )

        val reloaded = store.load().activeHostConfig()

        assertEquals("100.66.1.83", reloaded.host)
        assertEquals(4043, reloaded.port)
    }

    private fun readBundledConfig(): String {
        val candidates = listOf(
            File("src/main/assets/config/client.json"),
            File("app/src/main/assets/config/client.json"),
            File("apps/freehand-android/app/src/main/assets/config/client.json"),
        )
        return candidates.firstOrNull { it.exists() }?.readText()
            ?: fail("cannot locate bundled client config").let { "" }
    }

    private fun expectConfigError(block: () -> Unit): DaemonConnectionConfigException {
        return try {
            block()
            fail("expected DaemonConnectionConfigException")
            throw AssertionError("unreachable")
        } catch (e: DaemonConnectionConfigException) {
            e
        }
    }

    private fun buildBootstrapLink(expiresAtUnix: Long): String {
        val json = """
            {
              "kind": "freehand.remote-daemon-bootstrap",
              "schemaVersion": 1,
              "exportedAtUnix": 10,
              "expiresAtUnix": $expiresAtUnix,
              "nonce": "nonce-1",
              "account": {
                "id": "jason",
                "label": "Jason",
                "relayUrl": "https://relay.freehand.local/relay/"
              },
              "daemon": {
                "id": "studio",
                "accountId": "jason",
                "label": "Mac Studio",
                "nodeId": "studio-node",
                "activeEndpoint": "relay-web",
                "endpoints": [
                  {
                    "id": "relay-web",
                    "kind": "relay",
                    "webUrl": "https://relay.freehand.local/daemon/studio/web",
                    "relayHostId": "studio-host"
                  }
                ]
              },
              "credential": {
                "kind": "one_time_token",
                "value": "one-time-secret"
              }
            }
        """.trimIndent()
        val encoded = Base64.getUrlEncoder()
            .withoutPadding()
            .encodeToString(json.toByteArray(StandardCharsets.UTF_8))
        return "freehand://daemon/import?payload=$encoded"
    }
}

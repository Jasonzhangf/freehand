package com.freehand.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.File
import java.nio.file.Files

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
}

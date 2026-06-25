package com.freehand.android.data

import android.content.Context
import com.google.gson.Gson
import com.google.gson.JsonObject
import java.io.InputStreamReader

/**
 * Loads client configuration from assets/config/client.json.
 * Falls back to defaults if asset is missing.
 *
 * Config hierarchy:
 *   1. assets/config/client.json (bundled defaults)
 *   2. SharedPreferences overrides (user changes via drawer)
 *   3. HostStore persisted values
 */
data class ClientConfig(
    val daemonHost: String,
    val daemonPort: Int,
    val daemonProfile: String,
    val healthPath: String,
    val subscribePath: String,
    val commandPath: String,
    val queryPath: String,
    val upgradeCheckUrl: String,
    val upgradeDownloadUrl: String,
    val autoUpgradeCheck: Boolean,
    val upgradeIntervalSeconds: Int,
    val autoLanScan: Boolean,
    val scanPort: Int,
    val scanTimeoutMs: Int,
    val scanThreads: Int,
) {
    val daemonBaseUrl: String get() = "http://$daemonHost:$daemonPort"
    val fullSubscribeUrl: String get() = "$daemonBaseUrl$subscribePath"
    val fullCommandUrl: String get() = "$daemonBaseUrl$commandPath"
    val fullQueryUrl: String get() = "$daemonBaseUrl$queryPath"
    val fullHealthUrl: String get() = "$daemonBaseUrl$healthPath"

    fun toHostConfig(): HostConfig = HostConfig(daemonHost, daemonPort)

    companion object {
        private const val CONFIG_PATH = "config/client.json"
        private const val PREFS = "freehand_config"
        private const val KEY_HOST = "daemon_host"
        private const val KEY_PORT = "daemon_port"

        fun load(context: Context): ClientConfig {
            val defaults = loadFromAssets(context)
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val overrideHost = prefs.getString(KEY_HOST, null)
            val overridePort = if (prefs.contains(KEY_PORT)) prefs.getInt(KEY_PORT, defaults.daemonPort) else null

            return defaults.copy(
                daemonHost = overrideHost ?: defaults.daemonHost,
                daemonPort = overridePort ?: defaults.daemonPort,
            )
        }

        fun saveOverride(context: Context, host: String, port: Int) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY_HOST, host)
                .putInt(KEY_PORT, port)
                .apply()
        }

        private fun loadFromAssets(context: Context): ClientConfig {
            return try {
                val stream = context.assets.open(CONFIG_PATH)
                val reader = InputStreamReader(stream)
                val root = Gson().fromJson(reader, JsonObject::class.java)
                reader.close()

                val daemon = root.getAsJsonObject("daemon") ?: JsonObject()
                val upgrade = root.getAsJsonObject("upgrade") ?: JsonObject()
                val discovery = root.getAsJsonObject("discovery") ?: JsonObject()

                ClientConfig(
                    daemonHost = daemon.get("host")?.asString ?: "127.0.0.1",
                    daemonPort = daemon.get("port")?.asInt ?: 4041,
                    daemonProfile = daemon.get("profile")?.asString ?: "tailscale-main",
                    healthPath = daemon.get("healthPath")?.asString ?: "/health",
                    subscribePath = daemon.get("subscribePath")?.asString ?: "/ui/subscribe/turn/latest",
                    commandPath = daemon.get("commandPath")?.asString ?: "/ui/command",
                    queryPath = daemon.get("queryPath")?.asString ?: "/ui/query/latest-active-turn",
                    upgradeCheckUrl = upgrade.get("checkUrl")?.asString ?: "",
                    upgradeDownloadUrl = upgrade.get("downloadUrl")?.asString ?: "",
                    autoUpgradeCheck = upgrade.get("autoCheck")?.asBoolean ?: false,
                    upgradeIntervalSeconds = upgrade.get("intervalSeconds")?.asInt ?: 3600,
                    autoLanScan = discovery.get("autoLanScan")?.asBoolean ?: false,
                    scanPort = discovery.get("scanPort")?.asInt ?: 4041,
                    scanTimeoutMs = discovery.get("scanTimeoutMs")?.asInt ?: 1500,
                    scanThreads = discovery.get("scanThreads")?.asInt ?: 8,
                )
            } catch (_: Exception) {
                ClientConfig(
                    daemonHost = "100.66.1.82",
                    daemonPort = 4041,
                    daemonProfile = "tailscale-main",
                    healthPath = "/health",
                    subscribePath = "/ui/subscribe/turn/latest",
                    commandPath = "/ui/command",
                    queryPath = "/ui/query/latest-active-turn",
                    upgradeCheckUrl = "",
                    upgradeDownloadUrl = "",
                    autoUpgradeCheck = false,
                    upgradeIntervalSeconds = 3600,
                    autoLanScan = false,
                    scanPort = 4041,
                    scanTimeoutMs = 1500,
                    scanThreads = 8,
                )
            }
        }
    }
}

package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonParseException
import com.google.gson.JsonParser
import java.io.File

data class DaemonConnectionConfig(
    val connectionMode: String,
    val activeProfile: String,
    val profiles: List<DaemonConnectionProfile>,
    val relay: DaemonRelayConfig,
) {
    fun activeHostConfig(): HostConfig {
        val profile = profiles.firstOrNull { it.id == activeProfile }
            ?: throw DaemonConnectionConfigException("active profile '$activeProfile' is not defined")
        if (profile.mode != connectionMode) {
            throw DaemonConnectionConfigException(
                "active profile '${profile.id}' mode '${profile.mode}' does not match connectionMode '$connectionMode'",
            )
        }
        if (profile.mode == "relay" || relay.enabled) {
            throw DaemonConnectionConfigException("relay mode is reserved and disabled")
        }
        return profile.toHostConfig()
    }

    companion object {
        const val DEFAULT_CONFIG_FILE = "daemon-connection.json"
        private val gson = Gson()

        fun defaultTailscale(): DaemonConnectionConfig =
            DaemonConnectionConfig(
                connectionMode = "tailscale",
                activeProfile = "tailscale-main",
                profiles = listOf(
                    DaemonConnectionProfile(
                        id = "tailscale-main",
                        mode = "tailscale",
                        host = "100.66.1.82",
                        port = 4041,
                        adpPath = "/adp",
                        healthPath = "/health",
                        commandPath = "/ui/command",
                        queryPath = "/ui/query/latest-active-turn",
                        subscribePath = "/ui/subscribe/turn/latest",
                    ),
                ),
                relay = DaemonRelayConfig(enabled = false, url = "", authRef = ""),
            )

        fun parse(json: String): DaemonConnectionConfig {
            val root = try {
                JsonParser.parseString(json).asJsonObject
            } catch (e: JsonParseException) {
                throw DaemonConnectionConfigException("invalid daemon connection config json: ${e.message}", e)
            } catch (e: IllegalStateException) {
                throw DaemonConnectionConfigException("invalid daemon connection config json: ${e.message}", e)
            }
            val parsed = DaemonConnectionConfig(
                connectionMode = root.requiredString("connectionMode"),
                activeProfile = root.requiredString("activeProfile"),
                profiles = root.requiredArray("profiles").mapIndexed { index, entry ->
                    if (!entry.isJsonObject) {
                        throw DaemonConnectionConfigException("profiles[$index] must be an object")
                    }
                    val profile = entry.asJsonObject
                    DaemonConnectionProfile(
                        id = profile.requiredString("id"),
                        mode = profile.requiredString("mode"),
                        host = profile.requiredString("host"),
                        port = profile.requiredInt("port"),
                        adpPath = profile.optionalString("adpPath", "/adp"),
                        healthPath = profile.optionalString("healthPath", "/health"),
                        commandPath = profile.optionalString("commandPath", "/ui/command"),
                        queryPath = profile.optionalString("queryPath", "/ui/query/latest-active-turn"),
                        subscribePath = profile.optionalString("subscribePath", "/ui/subscribe/turn/latest"),
                    )
                },
                relay = root.optionalObject("relay")?.let {
                    DaemonRelayConfig(
                        enabled = it.optionalBoolean("enabled", false),
                        url = it.optionalString("url", ""),
                        authRef = it.optionalString("authRef", ""),
                    )
                } ?: DaemonRelayConfig(),
            )
            return parsed.normalizedAndValidated()
        }

        fun toJson(config: DaemonConnectionConfig): String =
            gson.toJson(config.normalizedAndValidated())

        private fun DaemonConnectionConfig.normalizedAndValidated(): DaemonConnectionConfig {
            if (connectionMode.isBlank()) {
                throw DaemonConnectionConfigException("connectionMode is required")
            }
            if (connectionMode != "tailscale") {
                throw DaemonConnectionConfigException("unsupported connectionMode '$connectionMode'")
            }
            if (activeProfile.isBlank()) {
                throw DaemonConnectionConfigException("activeProfile is required")
            }
            if (profiles.isEmpty()) {
                throw DaemonConnectionConfigException("at least one daemon profile is required")
            }
            profiles.forEach { it.validate() }
            relay.validate()
            activeHostConfig()
            return this
        }

        private fun com.google.gson.JsonObject.requiredString(field: String): String {
            val value = get(field)
            if (value == null || value.isJsonNull || value.asString.isBlank()) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asString
        }

        private fun com.google.gson.JsonObject.optionalString(field: String, default: String): String {
            val value = get(field)
            return if (value == null || value.isJsonNull) default else value.asString
        }

        private fun com.google.gson.JsonObject.requiredInt(field: String): Int {
            val value = get(field)
            if (value == null || value.isJsonNull) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asInt
        }

        private fun com.google.gson.JsonObject.optionalBoolean(field: String, default: Boolean): Boolean {
            val value = get(field)
            return if (value == null || value.isJsonNull) default else value.asBoolean
        }

        private fun com.google.gson.JsonObject.requiredArray(field: String): JsonArray {
            val value = get(field)
            if (value == null || value.isJsonNull || !value.isJsonArray) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asJsonArray
        }

        private fun com.google.gson.JsonObject.optionalObject(field: String): com.google.gson.JsonObject? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            if (!value.isJsonObject) {
                throw DaemonConnectionConfigException("$field must be an object")
            }
            return value.asJsonObject
        }
    }
}

data class DaemonConnectionProfile(
    val id: String,
    val mode: String,
    val host: String,
    val port: Int,
    val adpPath: String = "/adp",
    val healthPath: String = "/health",
    val commandPath: String = "/ui/command",
    val queryPath: String = "/ui/query/latest-active-turn",
    val subscribePath: String = "/ui/subscribe/turn/latest",
) {
    fun toHostConfig(): HostConfig =
        HostConfig(
            host = host,
            port = port,
            profileId = id,
            mode = mode,
            adpPath = adpPath,
            healthPath = healthPath,
            commandPath = commandPath,
            queryPath = queryPath,
            subscribePath = subscribePath,
        )

    fun validate() {
        if (id.isBlank()) throw DaemonConnectionConfigException("profile id is required")
        if (mode != "tailscale") throw DaemonConnectionConfigException("profile '$id' uses unsupported mode '$mode'")
        if (host.isBlank()) throw DaemonConnectionConfigException("profile '$id' host is required")
        if (port !in 1..65535) throw DaemonConnectionConfigException("profile '$id' port is invalid")
        validatePath(id, "adpPath", adpPath)
        validatePath(id, "healthPath", healthPath)
        validatePath(id, "commandPath", commandPath)
        validatePath(id, "queryPath", queryPath)
        validatePath(id, "subscribePath", subscribePath)
    }

    private fun validatePath(profileId: String, field: String, value: String) {
        if (!value.startsWith("/")) {
            throw DaemonConnectionConfigException("profile '$profileId' $field must start with '/'")
        }
    }
}

data class DaemonRelayConfig(
    val enabled: Boolean = false,
    val url: String = "",
    val authRef: String = "",
) {
    fun validate() {
        if (enabled) {
            throw DaemonConnectionConfigException("relay config is present but relay is disabled in this client")
        }
    }
}

class DaemonConnectionConfigStore(
    private val configFile: File,
    private val bundledConfigReader: () -> String,
) {
    fun load(): DaemonConnectionConfig {
        if (configFile.exists()) {
            return DaemonConnectionConfig.parse(configFile.readText())
        }
        val bundledJson = try {
            bundledConfigReader()
        } catch (e: Exception) {
            throw DaemonConnectionConfigException("bundled daemon connection config cannot be read: ${e.message}", e)
        }
        val bundled = DaemonConnectionConfig.parse(bundledJson)
        write(bundled)
        return bundled
    }

    fun write(config: DaemonConnectionConfig) {
        val normalized = DaemonConnectionConfig.toJson(config)
        try {
            configFile.parentFile?.mkdirs()
            configFile.writeText(normalized)
        } catch (e: Exception) {
            throw DaemonConnectionConfigException("app-owned daemon connection config cannot be written: ${e.message}", e)
        }
    }
}

class DaemonConnectionConfigException(
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause)

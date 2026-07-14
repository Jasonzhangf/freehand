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
) {
    fun activeHostConfig(): HostConfig {
        val profile = profiles.firstOrNull { it.id == activeProfile }
            ?: throw DaemonConnectionConfigException("active profile '$activeProfile' is not defined")
        if (profile.mode != connectionMode) {
            throw DaemonConnectionConfigException(
                "active profile '${profile.id}' mode '${profile.mode}' does not match connectionMode '$connectionMode'",
            )
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
                    ),
                ),
            )

        fun parse(json: String): DaemonConnectionConfig {
            val root = try {
                JsonParser.parseString(json).asJsonObject
            } catch (e: JsonParseException) {
                throw DaemonConnectionConfigException("invalid daemon connection config json: ${e.message}", e)
            } catch (e: IllegalStateException) {
                throw DaemonConnectionConfigException("invalid daemon connection config json: ${e.message}", e)
            }
            root.rejectUnknownFields("config", setOf("connectionMode", "activeProfile", "profiles"))
            val parsed = DaemonConnectionConfig(
                connectionMode = root.requiredString("connectionMode"),
                activeProfile = root.requiredString("activeProfile"),
                profiles = root.requiredArray("profiles").mapIndexed { index, entry ->
                    if (!entry.isJsonObject) {
                        throw DaemonConnectionConfigException("profiles[$index] must be an object")
                    }
                    val profile = entry.asJsonObject
                    profile.rejectUnknownFields(
                        "profiles[$index]",
                        setOf("id", "mode", "host", "port"),
                    )
                    DaemonConnectionProfile(
                        id = profile.requiredString("id"),
                        mode = profile.requiredString("mode"),
                        host = profile.requiredString("host"),
                        port = profile.requiredInt("port"),
                    )
                },
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
            activeHostConfig()
            return this
        }

        private fun com.google.gson.JsonObject.rejectUnknownFields(
            owner: String,
            allowed: Set<String>,
        ) {
            val unknown = keySet().filterNot(allowed::contains).sorted()
            if (unknown.isNotEmpty()) {
                throw DaemonConnectionConfigException(
                    "$owner contains unsupported fields: ${unknown.joinToString(", ")}",
                )
            }
        }

        private fun com.google.gson.JsonObject.requiredString(field: String): String {
            val value = get(field)
            if (value == null || value.isJsonNull || value.asString.isBlank()) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asString
        }

        private fun com.google.gson.JsonObject.requiredInt(field: String): Int {
            val value = get(field)
            if (value == null || value.isJsonNull) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asInt
        }

        private fun com.google.gson.JsonObject.requiredArray(field: String): JsonArray {
            val value = get(field)
            if (value == null || value.isJsonNull || !value.isJsonArray) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asJsonArray
        }
    }
}

data class DaemonConnectionProfile(
    val id: String,
    val mode: String,
    val host: String,
    val port: Int,
) {
    fun toHostConfig(): HostConfig =
        HostConfig(
            host = host,
            port = port,
        )

    fun validate() {
        if (id.isBlank()) throw DaemonConnectionConfigException("profile id is required")
        if (mode != "tailscale") throw DaemonConnectionConfigException("profile '$id' uses unsupported mode '$mode'")
        if (host.isBlank()) throw DaemonConnectionConfigException("profile '$id' host is required")
        if (port !in 1..65535) throw DaemonConnectionConfigException("profile '$id' port is invalid")
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

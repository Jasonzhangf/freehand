package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import com.google.gson.JsonParseException
import com.google.gson.JsonParser
import java.io.File
import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.Base64

data class DaemonConnectionConfig(
    val schemaVersion: Int,
    val connectionMode: String,
    val activeProfile: String,
    val profiles: List<DaemonConnectionProfile>,
    val activeAccount: String?,
    val activeDaemon: String?,
    val accounts: List<DaemonConnectionAccount>,
    val daemons: List<DaemonConnectionDaemon>,
) {
    fun activeHostConfig(): HostConfig {
        if (connectionMode == "remote_registry") {
            return activeRemoteHostConfig()
        }
        val profile = profiles.firstOrNull { it.id == activeProfile }
            ?: throw DaemonConnectionConfigException("active profile '$activeProfile' is not defined")
        if (profile.mode != connectionMode) {
            throw DaemonConnectionConfigException(
                "active profile '${profile.id}' mode '${profile.mode}' does not match connectionMode '$connectionMode'",
            )
        }
        return profile.toHostConfig()
    }

    private fun activeRemoteHostConfig(): HostConfig {
        val daemonId = activeDaemon?.takeIf { it.isNotBlank() }
            ?: throw DaemonConnectionConfigException("activeDaemon is required")
        val daemon = daemons.firstOrNull { it.id == daemonId }
            ?: throw DaemonConnectionConfigException("active daemon '$daemonId' is not defined")
        val accountId = activeAccount?.takeIf { it.isNotBlank() } ?: daemon.accountId
        accounts.firstOrNull { it.id == accountId }
            ?: throw DaemonConnectionConfigException("active account '$accountId' is not defined")
        if (daemon.accountId != accountId) {
            throw DaemonConnectionConfigException(
                "active daemon '${daemon.id}' belongs to account '${daemon.accountId}', not '$accountId'",
            )
        }
        val endpoint = daemon.endpoints.firstOrNull { it.id == daemon.activeEndpoint }
            ?: throw DaemonConnectionConfigException(
                "active endpoint '${daemon.activeEndpoint}' is not defined for daemon '${daemon.id}'",
            )
        return endpoint.toHostConfig(daemon.id)
    }

    fun toLegacyCompatibilityConfig(): DaemonConnectionConfig {
        val hostConfig = activeHostConfig()
        return DaemonConnectionConfig(
            schemaVersion = 1,
            connectionMode = "tailscale",
            activeProfile = "compat-active",
            profiles = listOf(
                DaemonConnectionProfile(
                    id = "compat-active",
                    mode = "tailscale",
                    host = hostConfig.host,
                    port = hostConfig.port,
                ),
            ),
            activeAccount = null,
            activeDaemon = null,
            accounts = emptyList(),
            daemons = emptyList(),
        ).normalizedAndValidated()
    }

    companion object {
        const val DEFAULT_CONFIG_FILE = "daemon-connection.json"
        const val REMOTE_REGISTRY_CONFIG_FILE = "daemon-connection-registry.json"
        private const val BOOTSTRAP_KIND = "freehand.remote-daemon-bootstrap"
        private const val BOOTSTRAP_SCHEMA_VERSION = 1
        private const val BOOTSTRAP_URL_PREFIX = "freehand://daemon/import?payload="
        private const val BOOTSTRAP_WEB_URL_PREFIX = "https://freehand.local/daemon/import?payload="
        private val gson = Gson()

        fun defaultTailscale(): DaemonConnectionConfig =
            DaemonConnectionConfig(
                schemaVersion = 1,
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
                activeAccount = null,
                activeDaemon = null,
                accounts = emptyList(),
                daemons = emptyList(),
            )

        fun parse(json: String): DaemonConnectionConfig {
            val root = parseJsonObject(json, "invalid daemon connection config json")
            root.rejectUnknownFields(
                "config",
                setOf(
                    "schemaVersion",
                    "connectionMode",
                    "activeProfile",
                    "profiles",
                    "activeAccount",
                    "activeDaemon",
                    "accounts",
                    "daemons",
                ),
            )
            val connectionMode = root.requiredString("connectionMode")
            val parsed = if (connectionMode == "remote_registry") {
                parseRemoteRegistryConfig(root)
            } else {
                parseLegacyProfileConfig(root)
            }
            return parsed.normalizedAndValidated()
        }

        fun parseBootstrapLink(input: String, nowUnix: Long = System.currentTimeMillis() / 1000): DaemonConnectionConfig {
            val payload = extractBootstrapPayload(input)
            val decoded = try {
                String(Base64.getUrlDecoder().decode(padBase64Url(payload)), StandardCharsets.UTF_8)
            } catch (e: IllegalArgumentException) {
                throw DaemonConnectionConfigException("remote daemon bootstrap payload is not base64url: ${e.message}", e)
            }
            val root = parseJsonObject(decoded, "remote daemon bootstrap payload is invalid")
            root.rejectUnknownFields(
                "remote daemon bootstrap",
                setOf(
                    "kind",
                    "schemaVersion",
                    "exportedAtUnix",
                    "expiresAtUnix",
                    "nonce",
                    "account",
                    "daemon",
                    "credential",
                ),
            )
            if (root.requiredString("kind") != BOOTSTRAP_KIND) {
                throw DaemonConnectionConfigException("remote daemon bootstrap kind is unsupported")
            }
            if (root.requiredInt("schemaVersion") != BOOTSTRAP_SCHEMA_VERSION) {
                throw DaemonConnectionConfigException("remote daemon bootstrap schemaVersion is unsupported")
            }
            val expiresAtUnix = root.requiredLong("expiresAtUnix")
            if (expiresAtUnix <= nowUnix) {
                throw DaemonConnectionConfigException("remote daemon bootstrap expired at $expiresAtUnix")
            }
            val nonce = root.requiredString("nonce")
            if (nonce.isBlank()) {
                throw DaemonConnectionConfigException("remote daemon bootstrap nonce is required")
            }
            val account = parseAccount(root.requiredObject("account"), "account")
            val daemon = parseRemoteDaemon(root.requiredObject("daemon"), "daemon")
            val credential = root.requiredObject("credential")
            val credentialKind = credential.requiredString("kind")
            if (credentialKind != "one_time_token") {
                throw DaemonConnectionConfigException("remote daemon bootstrap credential kind is unsupported")
            }
            val credentialValue = credential.requiredString("value")
            return DaemonConnectionConfig(
                schemaVersion = 1,
                connectionMode = "remote_registry",
                activeProfile = "",
                profiles = emptyList(),
                activeAccount = account.id,
                activeDaemon = daemon.id,
                accounts = listOf(account.copy(authToken = credentialValue)),
                daemons = listOf(daemon),
            ).normalizedAndValidated()
        }

        fun toJson(config: DaemonConnectionConfig): String =
            gson.toJson(config.normalizedAndValidated())

        private fun parseLegacyProfileConfig(root: JsonObject): DaemonConnectionConfig {
            return DaemonConnectionConfig(
                schemaVersion = root.optionalInt("schemaVersion") ?: 1,
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
                activeAccount = null,
                activeDaemon = null,
                accounts = emptyList(),
                daemons = emptyList(),
            )
        }

        private fun parseRemoteRegistryConfig(root: JsonObject): DaemonConnectionConfig {
            return DaemonConnectionConfig(
                schemaVersion = root.optionalInt("schemaVersion") ?: 1,
                connectionMode = "remote_registry",
                activeProfile = "",
                profiles = emptyList(),
                activeAccount = root.requiredString("activeAccount"),
                activeDaemon = root.requiredString("activeDaemon"),
                accounts = root.requiredArray("accounts").mapIndexed { index, entry ->
                    if (!entry.isJsonObject) {
                        throw DaemonConnectionConfigException("accounts[$index] must be an object")
                    }
                    parseAccount(entry.asJsonObject, "accounts[$index]")
                },
                daemons = root.requiredArray("daemons").mapIndexed { index, entry ->
                    if (!entry.isJsonObject) {
                        throw DaemonConnectionConfigException("daemons[$index] must be an object")
                    }
                    parseRemoteDaemon(entry.asJsonObject, "daemons[$index]")
                },
            )
        }

        private fun parseAccount(root: JsonObject, owner: String): DaemonConnectionAccount {
            root.rejectUnknownFields(owner, setOf("id", "label", "relayUrl", "authToken", "authTokenEnv"))
            return DaemonConnectionAccount(
                id = root.requiredString("id"),
                label = root.optionalString("label"),
                relayUrl = root.optionalString("relayUrl"),
                authToken = root.optionalString("authToken"),
                authTokenEnv = root.optionalString("authTokenEnv"),
            )
        }

        private fun parseRemoteDaemon(root: JsonObject, owner: String): DaemonConnectionDaemon {
            root.rejectUnknownFields(
                owner,
                setOf("id", "accountId", "label", "nodeId", "activeEndpoint", "endpoints"),
            )
            return DaemonConnectionDaemon(
                id = root.requiredString("id"),
                accountId = root.requiredString("accountId"),
                label = root.optionalString("label"),
                nodeId = root.requiredString("nodeId"),
                activeEndpoint = root.requiredString("activeEndpoint"),
                endpoints = root.requiredArray("endpoints").mapIndexed { index, entry ->
                    if (!entry.isJsonObject) {
                        throw DaemonConnectionConfigException("$owner.endpoints[$index] must be an object")
                    }
                    parseEndpoint(entry.asJsonObject, "$owner.endpoints[$index]")
                },
            )
        }

        private fun parseEndpoint(root: JsonObject, owner: String): DaemonEndpoint {
            root.rejectUnknownFields(
                owner,
                setOf("id", "kind", "host", "port", "webUrl", "adpUrl", "relayHostId", "authRequired"),
            )
            return DaemonEndpoint(
                id = root.requiredString("id"),
                kind = root.requiredString("kind"),
                host = root.optionalString("host"),
                port = root.optionalInt("port"),
                webUrl = root.optionalString("webUrl"),
                adpUrl = root.optionalString("adpUrl"),
                relayHostId = root.optionalString("relayHostId"),
                authRequired = root.optionalBoolean("authRequired") ?: true,
            )
        }

        private fun DaemonConnectionConfig.normalizedAndValidated(): DaemonConnectionConfig {
            if (schemaVersion != 1) {
                throw DaemonConnectionConfigException("unsupported schemaVersion '$schemaVersion'")
            }
            if (connectionMode.isBlank()) {
                throw DaemonConnectionConfigException("connectionMode is required")
            }
            if (connectionMode == "remote_registry") {
                validateRemoteRegistry()
                activeHostConfig()
                return this
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

        private fun DaemonConnectionConfig.validateRemoteRegistry() {
            if (activeAccount.isNullOrBlank()) {
                throw DaemonConnectionConfigException("activeAccount is required")
            }
            if (activeDaemon.isNullOrBlank()) {
                throw DaemonConnectionConfigException("activeDaemon is required")
            }
            if (accounts.isEmpty()) {
                throw DaemonConnectionConfigException("at least one account is required")
            }
            if (daemons.isEmpty()) {
                throw DaemonConnectionConfigException("at least one daemon is required")
            }
            val accountById = linkedMapOf<String, DaemonConnectionAccount>()
            accounts.forEach { account ->
                account.validate()
                if (accountById.put(account.id, account) != null) {
                    throw DaemonConnectionConfigException("duplicate account '${account.id}'")
                }
            }
            val daemonIds = mutableSetOf<String>()
            daemons.forEach { daemon ->
                daemon.validate(accountById)
                if (!daemonIds.add(daemon.id)) {
                    throw DaemonConnectionConfigException("duplicate daemon '${daemon.id}'")
                }
            }
        }

        private fun parseJsonObject(json: String, message: String): JsonObject {
            return try {
                JsonParser.parseString(json).asJsonObject
            } catch (e: JsonParseException) {
                throw DaemonConnectionConfigException("$message: ${e.message}", e)
            } catch (e: IllegalStateException) {
                throw DaemonConnectionConfigException("$message: ${e.message}", e)
            }
        }

        private fun extractBootstrapPayload(input: String): String {
            val raw = input.trim()
            if (raw.isBlank()) {
                throw DaemonConnectionConfigException("remote daemon bootstrap payload is empty")
            }
            if (!raw.contains("://") && !raw.contains("?")) {
                return raw
            }
            listOf(BOOTSTRAP_URL_PREFIX, BOOTSTRAP_WEB_URL_PREFIX).forEach { prefix ->
                if (raw.startsWith(prefix)) {
                    return raw.removePrefix(prefix).substringBefore("&").trim().also {
                        if (it.isBlank()) {
                            throw DaemonConnectionConfigException("remote daemon bootstrap payload is empty")
                        }
                    }
                }
            }
            throw DaemonConnectionConfigException("unsupported remote daemon bootstrap URL")
        }

        private fun padBase64Url(value: String): String {
            val remainder = value.length % 4
            return if (remainder == 0) value else value + "=".repeat(4 - remainder)
        }

        private fun JsonObject.rejectUnknownFields(
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

        private fun JsonObject.requiredObject(field: String): JsonObject {
            val value = get(field)
            if (value == null || value.isJsonNull || !value.isJsonObject) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asJsonObject
        }

        private fun JsonObject.requiredString(field: String): String {
            val value = get(field)
            if (value == null || value.isJsonNull || value.asString.isBlank()) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asString
        }

        private fun JsonObject.optionalString(field: String): String? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            return value.asString.trim().takeIf { it.isNotBlank() }
        }

        private fun JsonObject.requiredInt(field: String): Int {
            val value = get(field)
            if (value == null || value.isJsonNull) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asInt
        }

        private fun JsonObject.requiredLong(field: String): Long {
            val value = get(field)
            if (value == null || value.isJsonNull) {
                throw DaemonConnectionConfigException("$field is required")
            }
            return value.asLong
        }

        private fun JsonObject.optionalInt(field: String): Int? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            return value.asInt
        }

        private fun JsonObject.optionalBoolean(field: String): Boolean? {
            val value = get(field)
            if (value == null || value.isJsonNull) return null
            return value.asBoolean
        }

        private fun JsonObject.requiredArray(field: String): JsonArray {
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

data class DaemonConnectionAccount(
    val id: String,
    val label: String?,
    val relayUrl: String?,
    val authToken: String?,
    val authTokenEnv: String?,
) {
    fun validate() {
        if (id.isBlank()) throw DaemonConnectionConfigException("account id is required")
        relayUrl?.let { requireHttpUrl(it, "account '$id' relayUrl") }
    }
}

data class DaemonConnectionDaemon(
    val id: String,
    val accountId: String,
    val label: String?,
    val nodeId: String,
    val activeEndpoint: String,
    val endpoints: List<DaemonEndpoint>,
) {
    fun validate(accountById: Map<String, DaemonConnectionAccount>) {
        if (id.isBlank()) throw DaemonConnectionConfigException("daemon id is required")
        if (accountId.isBlank()) throw DaemonConnectionConfigException("daemon '$id' accountId is required")
        val account = accountById[accountId]
        if (account == null) {
            throw DaemonConnectionConfigException("daemon '$id' references missing account '$accountId'")
        }
        if (nodeId.isBlank()) throw DaemonConnectionConfigException("daemon '$id' nodeId is required")
        if (activeEndpoint.isBlank()) throw DaemonConnectionConfigException("daemon '$id' activeEndpoint is required")
        if (endpoints.isEmpty()) throw DaemonConnectionConfigException("daemon '$id' must declare at least one endpoint")
        val endpointIds = mutableSetOf<String>()
        endpoints.forEach { endpoint ->
            endpoint.validate(id)
            if (endpoint.kind == "relay" && account.relayUrl.isNullOrBlank()) {
                throw DaemonConnectionConfigException("daemon '$id' relay endpoint '${endpoint.id}' requires account '$accountId' relayUrl")
            }
            if (!endpointIds.add(endpoint.id)) {
                throw DaemonConnectionConfigException("daemon '$id' contains duplicate endpoint '${endpoint.id}'")
            }
        }
        if (!endpointIds.contains(activeEndpoint)) {
            throw DaemonConnectionConfigException("daemon '$id' active endpoint '$activeEndpoint' is not defined")
        }
    }
}

data class DaemonEndpoint(
    val id: String,
    val kind: String,
    val host: String?,
    val port: Int?,
    val webUrl: String?,
    val adpUrl: String?,
    val relayHostId: String?,
    val authRequired: Boolean,
) {
    fun validate(daemonId: String) {
        if (id.isBlank()) throw DaemonConnectionConfigException("daemon '$daemonId' endpoint id is required")
        when (kind) {
            "tailscale", "ipv4", "ipv6" -> {
                if (host.isNullOrBlank()) {
                    throw DaemonConnectionConfigException("daemon '$daemonId' direct endpoint '$id' host is required")
                }
                if (port == null || port !in 1..65535) {
                    throw DaemonConnectionConfigException("daemon '$daemonId' direct endpoint '$id' port is invalid")
                }
            }
            "relay" -> {
                if (webUrl.isNullOrBlank()) {
                    throw DaemonConnectionConfigException("daemon '$daemonId' relay endpoint '$id' webUrl is required")
                }
                requireHttpUrl(webUrl, "daemon '$daemonId' relay endpoint '$id' webUrl")
            }
            else -> throw DaemonConnectionConfigException("daemon '$daemonId' endpoint '$id' kind '$kind' is unsupported")
        }
    }

    fun toHostConfig(daemonId: String): HostConfig {
        validate(daemonId)
        if (kind == "relay") {
            val relayWebUrl = webUrl ?: throw DaemonConnectionConfigException("daemon '$daemonId' relay endpoint '$id' webUrl is required")
            val uri = URI(relayWebUrl)
            return HostConfig(
                host = uri.host ?: "relay",
                port = if (uri.port > 0) uri.port else if (uri.scheme == "https") 443 else 80,
                webUrlOverride = relayWebUrl,
            )
        }
        return HostConfig(
            host = host ?: throw DaemonConnectionConfigException("daemon '$daemonId' endpoint '$id' host is required"),
            port = port ?: throw DaemonConnectionConfigException("daemon '$daemonId' endpoint '$id' port is invalid"),
        )
    }
}

class DaemonConnectionConfigStore(
    private val configFile: File,
    private val bundledConfigReader: () -> String,
) {
    private val registryConfigFile: File =
        File(configFile.parentFile ?: configFile.absoluteFile.parentFile ?: File("."), DaemonConnectionConfig.REMOTE_REGISTRY_CONFIG_FILE)

    fun load(): DaemonConnectionConfig {
        if (registryConfigFile.exists()) {
            val config = DaemonConnectionConfig.parse(registryConfigFile.readText())
            writeLegacyCompatibilityConfig(config)
            return config
        }
        if (configFile.exists()) {
            val config = DaemonConnectionConfig.parse(configFile.readText())
            if (config.connectionMode == "remote_registry") {
                write(config)
            }
            return config
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

    fun importBootstrapLink(input: String, nowUnix: Long = System.currentTimeMillis() / 1000): DaemonConnectionConfig {
        val config = DaemonConnectionConfig.parseBootstrapLink(input, nowUnix)
        write(config)
        return config
    }

    fun write(config: DaemonConnectionConfig) {
        if (config.connectionMode == "remote_registry") {
            writeRemoteRegistryConfig(config)
            writeLegacyCompatibilityConfig(config)
            return
        }
        writeLegacyCompatibilityConfig(config)
        clearRemoteRegistryConfig()
    }

    private fun writeRemoteRegistryConfig(config: DaemonConnectionConfig) {
        writeText(registryConfigFile, DaemonConnectionConfig.toJson(config))
    }

    private fun writeLegacyCompatibilityConfig(config: DaemonConnectionConfig) {
        val legacy = if (config.connectionMode == "remote_registry") {
            config.toLegacyCompatibilityConfig()
        } else {
            config
        }
        writeText(configFile, DaemonConnectionConfig.toJson(legacy))
    }

    private fun clearRemoteRegistryConfig() {
        if (registryConfigFile.exists() && !registryConfigFile.delete()) {
            throw DaemonConnectionConfigException("app-owned remote registry config cannot be cleared")
        }
    }

    private fun writeText(file: File, text: String) {
        try {
            file.parentFile?.mkdirs()
            file.writeText(text)
        } catch (e: Exception) {
            throw DaemonConnectionConfigException("app-owned daemon connection config cannot be written: ${e.message}", e)
        }
    }
}

class DaemonConnectionConfigException(
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause)

private fun requireHttpUrl(value: String, owner: String) {
    val uri = try {
        URI(value)
    } catch (e: Exception) {
        throw DaemonConnectionConfigException("$owner must be an http(s) URL with a host", e)
    }
    if ((uri.scheme != "http" && uri.scheme != "https") || uri.host.isNullOrBlank()) {
        throw DaemonConnectionConfigException("$owner must be an http(s) URL with a host")
    }
}

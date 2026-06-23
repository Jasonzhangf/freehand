package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

class ProtocolClient(
    private val httpClient: OkHttpClient,
    private val host: HostConfig,
) {

    private val gson = Gson()
    private val json = "application/json".toMediaType()

    @Throws(IOException::class)
    fun postCommand(payload: String): CommandResponse {
        val req = Request.Builder()
            .url(host.commandUrl)
            .post(payload.toRequestBody(json))
            .build()
        httpClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) {
                return CommandResponse(
                    ok = false,
                    code = "http_${resp.code}",
                    message = body.ifBlank { "command rejected" },
                )
            }
            return try {
                val parsed = gson.fromJson(body, JsonObject::class.java)
                val ok = parsed.get("ok")?.asBoolean ?: true
                val code = parsed.get("code")?.asString.orEmpty()
                val message = parsed.get("message")?.asString.orEmpty()
                CommandResponse(ok = ok, code = code, message = message)
            } catch (e: Exception) {
                CommandResponse(ok = false, code = "bad_response", message = body)
            }
        }
    }

    @Throws(IOException::class)
    fun getLatestTurn(): JsonObject? {
        val req = Request.Builder().url(host.latestTurnUrl).get().build()
        httpClient.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful) return null
            val body = resp.body?.string().orEmpty()
            if (body.isBlank()) return null
            return runCatching { gson.fromJson(body, JsonObject::class.java) }.getOrNull()
        }
    }
}

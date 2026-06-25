package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import java.util.concurrent.Executors

class CommandIngress(
    private val client: ProtocolClient,
    private val onResult: (ok: Boolean, reason: String) -> Unit,
) {

    private val gson = Gson()
    private val executor = Executors.newSingleThreadExecutor()

    fun submit(userText: String) {
        if (userText.isBlank()) return
        val payload = JsonObject().apply {
            add(
                "SubmitUserInput",
                JsonObject().apply {
                    addProperty("text", userText)
                },
            )
        }
        executor.execute {
            try {
                val response = client.postCommand(gson.toJson(payload))
                onResult(response.ok, response.message.ifBlank { response.code })
            } catch (e: Exception) {
                onResult(false, e.message ?: "send_failed")
            }
        }
    }

    fun cancelLatest() {
        val payload = JsonObject().apply {
            add("CancelLatestActiveTurn", JsonObject())
        }
        executor.execute {
            try {
                client.postCommand(gson.toJson(payload))
            } catch (_: Exception) {
            }
        }
    }
}

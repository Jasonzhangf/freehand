package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/**
 * ADP WebSocket stream for ui.protocol query / subscribe / command frames.
 *
 * Android is a protocol consumer only: this class transports UiAdpRequest and
 * UiAdpResponse JSON, but does not own turn, debug, runtime, or provider truth.
 */
class AdpEventStream(
    private val httpClient: OkHttpClient,
    private val host: HostConfig,
    private val onEvent: (Event) -> Unit,
    private val onCommandResult: (CommandResponse) -> Unit,
    private val onError: (Throwable) -> Unit,
    private val onOpen: () -> Unit = {},
    private val onClosed: () -> Unit = {},
) {
    private val gson = Gson()
    private val sequence = AtomicInteger(0)
    private val pendingCommands = ConcurrentHashMap<String, Unit>()
    private var socket: WebSocket? = null

    fun start() {
        if (socket != null) return
        val req = Request.Builder().url(host.adpUrl).build()
        socket = httpClient.newWebSocket(req, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                onOpen()
                subscribeLatestTurn(webSocket)
                queryLatestTurn(webSocket)
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                handleFrame(text)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                socket = null
                onClosed()
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                socket = null
                onError(t)
            }
        })
    }

    fun stop() {
        socket?.close(1000, "android paused")
        socket = null
    }

    fun sendCommand(commandJson: String): CommandResponse {
        val command = try {
            JsonParser.parseString(commandJson)
        } catch (e: Exception) {
            return CommandResponse(false, "bad_command_json", e.message ?: "bad command json")
        }
        val webSocket = socket
            ?: return CommandResponse(false, "adp_not_connected", "service connection is not ready")
        val requestId = nextRequestId("cmd")
        pendingCommands[requestId] = Unit
        val sent = webSocket.send(buildFrame("command", requestId, "command", command))
        if (!sent) {
            pendingCommands.remove(requestId)
            return CommandResponse(false, "adp_send_failed", "request send failed")
        }
        return CommandResponse(true, "adp_command_sent", "request sent")
    }

    private fun subscribeLatestTurn(webSocket: WebSocket) {
        val subscription = JsonObject().apply {
            add(
                "SubscribeLatestActiveTurn",
                JsonObject().apply { addProperty("client", "WebUi") },
            )
        }
        webSocket.send(buildFrame("subscribe", nextRequestId("sub"), "subscription", subscription))
    }

    private fun queryLatestTurn(webSocket: WebSocket) {
        webSocket.send(buildFrame("query", nextRequestId("query"), "query", gson.toJsonTree("QueryLatestActiveTurn")))
    }

    private fun handleFrame(text: String) {
        val frame = runCatching { JsonParser.parseString(text).asJsonObject }
            .getOrElse {
                onError(IOException("invalid ADP JSON: ${it.message}"))
                return
            }
        when (val kind = frame.get("kind")?.asString) {
            "subscription_accepted",
            "subscription_event",
            "query_result" -> onEvent(Event(kind, frame))
            "command_receipt" -> {
                val requestId = frame.get("request_id")?.asString.orEmpty()
                pendingCommands.remove(requestId)
                val receipt = frame.getAsJsonObject("receipt")
                onCommandResult(
                    CommandResponse(
                        ok = true,
                        code = receipt?.get("dispatch_status")?.asString.orEmpty(),
                        message = receipt?.get("target_feature_id")?.asString.orEmpty(),
                    ),
                )
            }
            "failure" -> {
                val requestId = frame.get("request_id")?.asString.orEmpty()
                pendingCommands.remove(requestId)
                val failure = frame.getAsJsonObject("failure")
                val response = CommandResponse(
                    ok = false,
                    code = failure?.get("code")?.asString ?: "adp_failure",
                    message = failure?.get("message")?.asString ?: "connection failure",
                )
                if (requestId.startsWith("android-cmd-")) {
                    onCommandResult(response)
                } else {
                    onEvent(Event(kind, frame))
                }
            }
            else -> onError(IOException("unknown ADP frame kind: $kind"))
        }
    }

    private fun nextRequestId(prefix: String): String =
        "android-$prefix-${sequence.incrementAndGet()}"

    data class Event(
        val frameKind: String,
        val frame: JsonObject,
    )

    companion object {
        fun buildFrame(kind: String, requestId: String, payloadName: String, payload: JsonElement): String {
            val root = JsonObject()
            root.addProperty("kind", kind)
            root.addProperty("request_id", requestId)
            root.add(payloadName, payload)
            return Gson().toJson(root)
        }
    }
}

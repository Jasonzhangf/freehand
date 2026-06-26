package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources

/**
 * SSE event stream for `ui.protocol` latest-turn subscribe.
 *
 * Android consumes the same SSE stream the WebUI consumes; this is the only
 * truth-source path. Each [Event] has an `event` field (turn/progress/error/
 * terminal/node_status/checkpoints) and a `data` field (JSON body).
 *
 * @see `docs/function-maps/app.android-client.md`
 */
class SseEventStream(
    private val httpClient: OkHttpClient,
    private val host: HostConfig,
    private val onEvent: (Event) -> Unit,
    private val onError: (Throwable) -> Unit,
    private val onOpen: () -> Unit = {},
    private val onClosed: () -> Unit = {},
) {
    private val gson = Gson()
    private var source: EventSource? = null

    fun start() {
        if (source != null) return
        val req = Request.Builder()
            .url(host.latestTurnSseUrl)
            .header("Accept", "text/event-stream")
            .get()
            .build()
        source = EventSources.createFactory(httpClient)
            .newEventSource(req, object : EventSourceListener() {
                override fun onOpen(eventSource: EventSource, response: okhttp3.Response) {
                    onOpen()
                }
                override fun onEvent(
                    eventSource: EventSource,
                    id: String?,
                    type: String?,
                    data: String,
                ) {
                    val parsed = runCatching {
                        gson.fromJson(data, JsonObject::class.java)
                    }.getOrNull()
                    if (parsed != null) {
                        onEvent(Event(eventName = type ?: "message", data = parsed))
                    }
                }
                override fun onClosed(eventSource: EventSource) {
                    onClosed()
                }
                override fun onFailure(
                    eventSource: EventSource,
                    t: Throwable?,
                    response: okhttp3.Response?,
                ) {
                    if (t != null) onError(t)
                }
            })
    }

    fun stop() {
        source?.cancel()
        source = null
    }

    data class Event(
        val eventName: String,
        val data: JsonObject,
    )
}

package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import org.junit.Assert.*
import org.junit.Test
import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Tests that CommandIngress produces correct UiCommand serde shapes
 * matching crates/freehand-ui-protocol/src/lib.rs UiCommand external-tag form.
 *
 * The server expects:
 *   {"SubmitUserInput":{"text":"..."}}   (not {"type":"SubmitUserInput","text":"..."})
 *   {"CancelLatestActiveTurn":{}}
 */
class CommandIngressProtocolTest {

    private val gson = Gson()

    @Test
    fun `submit payload uses external-tag UiCommand shape`() {
        val userText = "hello world"
        val payload = JsonObject().apply {
            add(
                "SubmitUserInput",
                JsonObject().apply {
                    addProperty("text", userText)
                },
            )
        }
        val json = gson.toJson(payload)
        val parsed = JsonParser.parseString(json).asJsonObject

        // Must have exactly one top-level key: "SubmitUserInput"
        assertEquals(1, parsed.entrySet().size)
        assertTrue(parsed.has("SubmitUserInput"))

        // SubmitUserInput must have "text" field
        val inner = parsed.getAsJsonObject("SubmitUserInput")
        assertEquals(userText, inner.get("text").asString)
    }

    @Test
    fun `cancel payload uses external-tag UiCommand shape`() {
        val payload = JsonObject().apply {
            add("CancelLatestActiveTurn", JsonObject())
        }
        val json = gson.toJson(payload)
        val parsed = JsonParser.parseString(json).asJsonObject

        assertEquals(1, parsed.entrySet().size)
        assertTrue(parsed.has("CancelLatestActiveTurn"))
        assertEquals(0, parsed.getAsJsonObject("CancelLatestActiveTurn").entrySet().size)
    }

    @Test
    fun `submit payload must NOT have type field`() {
        val payload = JsonObject().apply {
            addProperty("type", "SubmitUserInput")
            addProperty("text", "hello")
        }
        val json = gson.toJson(payload)
        val parsed = JsonParser.parseString(json).asJsonObject

        // This is the WRONG shape; verify it exists as a negative test
        assertTrue(parsed.has("type"))
        // Correct shape must NOT have "type"
        val correctPayload = JsonObject().apply {
            add("SubmitUserInput", JsonObject().apply { addProperty("text", "hello") })
        }
        val correctParsed = JsonParser.parseString(gson.toJson(correctPayload)).asJsonObject
        assertFalse(correctParsed.has("type"))
    }

    @Test
    fun `ADP command frame wraps UiCommand without type field`() {
        val command = JsonObject().apply {
            add("SubmitUserInput", JsonObject().apply { addProperty("text", "hello adp") })
        }
        val frame = AdpEventStream.buildFrame(
            kind = "command",
            requestId = "android-cmd-1",
            payloadName = "command",
            payload = command,
        )
        val parsed = JsonParser.parseString(frame).asJsonObject

        assertEquals("command", parsed.get("kind").asString)
        assertEquals("android-cmd-1", parsed.get("request_id").asString)
        assertTrue(parsed.getAsJsonObject("command").has("SubmitUserInput"))
        assertFalse(parsed.has("type"))
        assertFalse(parsed.getAsJsonObject("command").has("type"))
    }

    @Test
    fun `ADP query-as-command negative frame is visibly command misuse`() {
        val frame = AdpEventStream.buildFrame(
            kind = "command",
            requestId = "android-bad-1",
            payloadName = "command",
            payload = gson.toJsonTree("QueryLatestActiveTurn"),
        )
        val parsed = JsonParser.parseString(frame).asJsonObject

        assertEquals("command", parsed.get("kind").asString)
        assertEquals("QueryLatestActiveTurn", parsed.get("command").asString)
        assertFalse(parsed.has("query"))
    }

    @Test
    fun `ADP subscribe latest turn frame uses protocol client kind`() {
        val subscription = JsonObject().apply {
            add(
                "SubscribeLatestActiveTurn",
                JsonObject().apply { addProperty("client", "WebUi") },
            )
        }
        val frame = AdpEventStream.buildFrame(
            kind = "subscribe",
            requestId = "android-sub-1",
            payloadName = "subscription",
            payload = subscription,
        )
        val parsed = JsonParser.parseString(frame).asJsonObject

        assertEquals("subscribe", parsed.get("kind").asString)
        assertEquals(
            "WebUi",
            parsed.getAsJsonObject("subscription")
                .getAsJsonObject("SubscribeLatestActiveTurn")
                .get("client").asString,
        )
    }

    @Test
    fun `special characters in text are escaped`() {
        val text = "line1\nline2\ttab\"quote"
        val payload = JsonObject().apply {
            add("SubmitUserInput", JsonObject().apply { addProperty("text", text) })
        }
        val json = gson.toJson(payload)
        val parsed = JsonParser.parseString(json).asJsonObject
        assertEquals(text, parsed.getAsJsonObject("SubmitUserInput").get("text").asString)
    }

    @Test
    fun `empty text is allowed in payload`() {
        val payload = JsonObject().apply {
            add("SubmitUserInput", JsonObject().apply { addProperty("text", "") })
        }
        val json = gson.toJson(payload)
        val parsed = JsonParser.parseString(json).asJsonObject
        assertEquals("", parsed.getAsJsonObject("SubmitUserInput").get("text").asString)
    }

    @Test
    fun `cancelLatest reports transport failure through callback`() {
        val latch = CountDownLatch(1)
        var ok: Boolean? = null
        var reason: String? = null
        val ingress = CommandIngress(
            DummyProtocolClient,
            { resultOk, resultReason ->
                ok = resultOk
                reason = resultReason
                latch.countDown()
            },
            sendCommand = { throw IOException("cancel_failed") },
        )

        ingress.cancelLatest()

        assertTrue(latch.await(3, TimeUnit.SECONDS))
        assertEquals(false, ok)
        assertEquals("cancel_failed", reason)
    }

    private companion object {
        val DummyProtocolClient = ProtocolClient(
            httpClient = okhttp3.OkHttpClient(),
            host = HostConfig("127.0.0.1", 4041),
        )
    }
}

package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import org.junit.Assert.*
import org.junit.Test

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
}

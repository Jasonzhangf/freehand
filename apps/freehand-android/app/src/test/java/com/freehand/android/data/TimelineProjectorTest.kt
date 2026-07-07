package com.freehand.android.data

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

class TimelineProjectorTest {

    private lateinit var projector: TimelineProjector

    @Before
    fun setup() {
        projector = TimelineProjector()
    }

    // ── apply() turn event ──────────────────────────────────────────────

    @Test
    fun `turn event sets turn state to running when no terminal_status`() {
        val event = sseEvent("turn", """
            {"turn":{"source_agent_id":"master","source_node_id":"master-node",
             "session_id":"s1","turn_id":"t1","user_text":"hello",
             "reasoning":[],"text":[],"tool_calls":[],"usage":[],
             "terminal_status":null,"terminal_text":null,
             "errors":[],"slave_substream_card":false},
             "public_conversation":[{"kind":"UserText","title":"User","body":"hello","status":"submitted"}]}
        """.trimIndent())
        projector.apply(event)
        assertEquals("running", projector.snapshot()["turn_state"])
        assertEquals("master", projector.snapshot()["agent_id"])
    }

    @Test
    fun `turn event with terminal_status Success maps to done`() {
        val event = sseEvent("turn", """
            {"turn":{"source_agent_id":"master","source_node_id":"master-node",
             "session_id":"s1","turn_id":"t1","user_text":"hello",
             "reasoning":[],"text":["hi"],"tool_calls":[],"usage":[],
             "terminal_status":"Success","terminal_text":"done",
             "errors":[],"slave_substream_card":false},
             "public_conversation":[{"kind":"Terminal","title":"Final","body":"done","status":"completed"}]}
        """.trimIndent())
        projector.apply(event)
        assertEquals("done", projector.snapshot()["turn_state"])
    }

    @Test
    fun `turn event with terminal_status Error maps to error`() {
        val event = sseEvent("turn", """
            {"turn":{"source_agent_id":"master","source_node_id":"master-node",
             "session_id":"s1","turn_id":"t1","user_text":"hello",
             "reasoning":[],"text":[],"tool_calls":[],"usage":[],
             "terminal_status":"Error","terminal_text":"failed",
             "errors":[],"slave_substream_card":false},
             "public_conversation":[]}
        """.trimIndent())
        projector.apply(event)
        assertEquals("error", projector.snapshot()["turn_state"])
    }

    @Test
    fun `turn event preserves latestRawTurnProjection for bridge`() {
        val raw = """{"turn":{"source_agent_id":"master","source_node_id":"master-node",
            "session_id":"s1","turn_id":"t1","user_text":"hello",
            "reasoning":[],"text":[],"tool_calls":[],"usage":[],
            "terminal_status":null,"terminal_text":null,
            "errors":[],"slave_substream_card":false},
            "public_conversation":[{"kind":"UserText","title":"User","body":"hello","status":"submitted"}]}"""
        val event = sseEvent("turn", raw)
        projector.apply(event)
        val json = projector.latestTurnProjectionJson()
        assertNotNull(json)
        assertTrue(json!!.contains("t1"))
        assertTrue(json.contains("public_conversation"))
    }

    @Test
    fun `ADP subscription_event turn updates latest projection for bridge`() {
        val event = adpEvent("subscription_event", """
            {"kind":"subscription_event","request_id":"sub-1",
             "event":{"projection":{"Turn":{"source":{"source_agent_id":"master","source_node_id":"master-node","source_turn_id":"t-adp","stream_kind":"Turn"},
             "session_id":"s1","turn_id":"t-adp","user_text":"hello adp",
             "reasoning":[],"text":[],"tool_calls":[],"tool_activities":[{"tool_call_id":"tool-1","tool_name":"read_file","status":"Waiting","detail":"waiting for tool execution"}],"usage":[],
             "terminal_status":null,"terminal_text":null,
             "errors":[],"slave_substream_card":false}},
             "latest_active_turn_id":"t-adp"}}
        """.trimIndent())

        projector.applyAdp(event)

        assertEquals("running", projector.snapshot()["turn_state"])
        assertEquals("master", projector.snapshot()["agent_id"])
        val json = projector.latestTurnProjectionJson()
        assertNotNull(json)
        assertTrue(json!!.contains("hello adp"))
        assertTrue(json.contains("\"title\":\"read_file\""))
        assertTrue(json.contains("\"body\":\"waiting\""))
        assertFalse(json.contains("Tool call requested"))
    }

    @Test
    fun `ADP failure marks visible error projection`() {
        val event = adpEvent("failure", """
            {"kind":"failure","request_id":"bad-1",
             "failure":{"code":"ingress_command_kind_mismatch","message":"query frame rejected","retryable":false}}
        """.trimIndent())

        projector.applyAdp(event)

        assertEquals("error", projector.snapshot()["turn_state"])
        assertEquals("error", projector.snapshot()["connection"])
        val json = projector.latestTurnProjectionJson()
        assertNotNull(json)
        assertTrue(json!!.contains("query frame rejected"))
        assertTrue(json.contains("\"title\":\"Connection\""))
        assertFalse(json.contains("\"title\":\"ADP\""))
        assertFalse(json.contains("ADP failure"))
    }

    // ── apply() progress event ──────────────────────────────────────────

    @Test
    fun `progress event updates turn_state`() {
        projector.apply(sseEvent("progress", """{"status_text":"thinking"}"""))
        assertEquals("thinking", projector.snapshot()["turn_state"])
    }

    // ── apply() node_status event ───────────────────────────────────────

    @Test
    fun `node_status event populates slaves`() {
        projector.apply(sseEvent("node_status", """
            {"source":{"source_agent_id":"worker","source_node_id":"worker-node","source_turn_id":null,"stream_kind":"NodeStatus"},
             "node_id":"worker-node","healthy":true,"pairing_state":"paired"}
        """.trimIndent()))
        val slaves = projector.snapshot()["slaves"] as Map<*, *>
        assertTrue(slaves.containsKey("worker"))
    }

    @Test
    fun `node_status unhealthy sets blocked`() {
        projector.apply(sseEvent("node_status", """
            {"source":{"source_agent_id":"worker","source_node_id":"worker-node","source_turn_id":null,"stream_kind":"NodeStatus"},
             "node_id":"worker-node","healthy":false,"pairing_state":"lost"}
        """.trimIndent()))
        assertEquals("blocked", projector.snapshot()["turn_state"])
    }

    // ── apply() terminal event ──────────────────────────────────────────

    @Test
    fun `terminal event updates turn terminal status`() {
        // First create a turn
        projector.apply(sseEvent("turn", """
            {"turn":{"source_agent_id":"master","source_node_id":"n","session_id":"s","turn_id":"t1",
             "user_text":"x","reasoning":[],"text":[],"tool_calls":[],"usage":[],
             "terminal_status":null,"terminal_text":null,"errors":[],"slave_substream_card":false},
             "public_conversation":[]}
        """.trimIndent()))
        // Then apply terminal
        projector.apply(sseEvent("terminal", """
            {"turn_id":"t1","status":"done","summary":"finished"}
        """.trimIndent()))
        assertEquals("done", projector.snapshot()["turn_state"])
    }

    // ── apply() error event ─────────────────────────────────────────────

    @Test
    fun `error event marks turn as error`() {
        projector.apply(sseEvent("turn", """
            {"turn":{"source_agent_id":"master","source_node_id":"n","session_id":"s","turn_id":"t1",
             "user_text":"x","reasoning":[],"text":[],"tool_calls":[],"usage":[],
             "terminal_status":null,"terminal_text":null,"errors":[],"slave_substream_card":false},
             "public_conversation":[]}
        """.trimIndent()))
        projector.apply(sseEvent("error", """{"turn_id":"t1","message":"provider timeout"}"""))
        assertEquals("error", projector.snapshot()["turn_state"])
    }

    // ── empty state ─────────────────────────────────────────────────────

    @Test
    fun `snapshot returns idle state when no events received`() {
        val snap = projector.snapshot()
        assertEquals("idle", snap["turn_state"])
        assertEquals("idle", snap["connection"])
    }

    @Test
    fun `latestTurnProjectionJson returns null when no turn received`() {
        assertNull(projector.latestTurnProjectionJson())
    }

    // ── snapshotJson() ──────────────────────────────────────────────────

    @Test
    fun `snapshotJson returns valid JSON`() {
        val json = projector.snapshotJson()
        assertNotNull(json)
        assertTrue(json.contains("turn_state"))
    }

    // ── connection state ────────────────────────────────────────────────

    @Test
    fun `setConnectionState updates snapshot connection field`() {
        projector.setConnectionState("open")
        assertEquals("open", projector.snapshot()["connection"])
        projector.setConnectionState("error")
        assertEquals("error", projector.snapshot()["connection"])
    }

    // ── multi-turn accumulation + allTurnsProjectionJson ───────────────

    @Test
    fun `multiple turn events accumulate and are exposed via allTurnsProjectionJson`() {
        projector.apply(sseEvent("turn", "{\"turn\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\"," +
            "\"session_id\":\"s1\",\"turn_id\":\"t1\",\"user_text\":\"hello\",\"reasoning\":[],\"text\":[]," +
            "\"tool_calls\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}," +
            "\"public_conversation\":[{\"kind\":\"UserText\",\"title\":\"User\",\"body\":\"hello\",\"status\":\"submitted\"}]}"))

        projector.apply(sseEvent("turn", "{\"turn\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\"," +
            "\"session_id\":\"s1\",\"turn_id\":\"t2\",\"user_text\":\"second message\",\"reasoning\":[],\"text\":[\"hi there\"]," +
            "\"tool_calls\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}," +
            "\"public_conversation\":[{\"kind\":\"UserText\",\"title\":\"User\",\"body\":\"second message\",\"status\":\"submitted\"}," +
            "{\"kind\":\"AssistantText\",\"title\":\"Assistant\",\"body\":\"hi there\",\"status\":\"streaming\"}]}"))

        val snap = projector.snapshot()
        assertEquals(2, (snap["turns"] as List<*>).size)

        val allJson = projector.allTurnsProjectionJson()
        assertTrue(allJson.contains("t1"))
        assertTrue(allJson.contains("t2"))
        assertTrue(allJson.contains("hello"))
        assertTrue(allJson.contains("second message"))
        val wrapped = JsonParser.parseString(allJson).asJsonObject
        assertTrue(wrapped.has("all_turns"))
        assertEquals(2, wrapped.getAsJsonArray("all_turns").size())
    }

    @Test
    fun `allTurnsProjectionJson returns empty all_turns when no turns received`() {
        val wrapped = JsonParser.parseString(projector.allTurnsProjectionJson()).asJsonObject
        assertTrue(wrapped.has("all_turns"))
        assertEquals(0, wrapped.getAsJsonArray("all_turns").size())
    }

    @Test
    fun `clearAccumulatedTurns removes only accumulated render state`() {
        projector.setConnectionState("open")
        projector.setCurrentAgent("master", "Master")
        projector.apply(sseEvent("turn", "{\"turn\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\"," +
            "\"session_id\":\"s\",\"turn_id\":\"t1\",\"user_text\":\"x\",\"reasoning\":[],\"text\":[]," +
            "\"tool_calls\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}," +
            "\"public_conversation\":[{\"kind\":\"UserText\",\"title\":\"User\",\"body\":\"x\",\"status\":\"submitted\"}]}"))

        projector.clearAccumulatedTurns()

        val wrapped = JsonParser.parseString(projector.allTurnsProjectionJson()).asJsonObject
        assertEquals(0, wrapped.getAsJsonArray("all_turns").size())
        assertEquals("open", projector.snapshot()["connection"])
        assertEquals("master", projector.snapshot()["agent_id"])
        assertEquals("Master", projector.snapshot()["agent_name"])
        assertEquals(0, (projector.snapshot()["turns"] as List<*>).size)
    }

    @Test
    fun `terminal event on existing turn updates its all-turns projection`() {
        projector.apply(sseEvent("turn", "{\"turn\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\"," +
            "\"session_id\":\"s\",\"turn_id\":\"t1\",\"user_text\":\"x\",\"reasoning\":[],\"text\":[]," +
            "\"tool_calls\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}," +
            "\"public_conversation\":[{\"kind\":\"UserText\",\"title\":\"User\",\"body\":\"x\",\"status\":\"submitted\"}]}"))

        projector.apply(sseEvent("terminal", "{\"turn_id\":\"t1\",\"status\":\"done\",\"summary\":\"finished\"}"))

        val allJson = projector.allTurnsProjectionJson()
        assertTrue(allJson.contains("\"terminal_status\":\"done\""))
    }

    @Test
    fun `error event on existing turn updates its all-turns projection`() {
        projector.apply(sseEvent("turn", "{\"turn\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\"," +
            "\"session_id\":\"s\",\"turn_id\":\"t1\",\"user_text\":\"x\",\"reasoning\":[],\"text\":[]," +
            "\"tool_calls\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}," +
            "\"public_conversation\":[{\"kind\":\"UserText\",\"title\":\"User\",\"body\":\"x\",\"status\":\"submitted\"}]}"))

        projector.apply(sseEvent("error", "{\"turn_id\":\"t1\",\"message\":\"provider timeout\"}"))

        val allJson = projector.allTurnsProjectionJson()
        assertTrue(allJson.contains("provider timeout"))
    }

    @Test
    fun `ADP subscription_event accumulates turns across multiple events`() {
        val turn1 = "{\"kind\":\"subscription_event\",\"request_id\":\"sub-1\"," +
            "\"event\":{\"projection\":{\"Turn\":{\"source\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\",\"source_turn_id\":\"t-adp-1\",\"stream_kind\":\"Turn\"}," +
            "\"session_id\":\"s\",\"turn_id\":\"t-adp-1\",\"user_text\":\"first\",\"reasoning\":[],\"text\":[]," +
            "\"tool_calls\":[],\"tool_activities\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}}," +
            "\"latest_active_turn_id\":\"t-adp-1\"}}"

        val turn2 = "{\"kind\":\"subscription_event\",\"request_id\":\"sub-2\"," +
            "\"event\":{\"projection\":{\"Turn\":{\"source\":{\"source_agent_id\":\"master\",\"source_node_id\":\"n\",\"source_turn_id\":\"t-adp-2\",\"stream_kind\":\"Turn\"}," +
            "\"session_id\":\"s\",\"turn_id\":\"t-adp-2\",\"user_text\":\"second\",\"reasoning\":[],\"text\":[\"response\"]," +
            "\"tool_calls\":[],\"tool_activities\":[],\"usage\":[],\"terminal_status\":null,\"terminal_text\":null," +
            "\"errors\":[],\"slave_substream_card\":false}}," +
            "\"latest_active_turn_id\":\"t-adp-2\"}}"

        projector.applyAdp(adpEvent("subscription_event", turn1))
        projector.applyAdp(adpEvent("subscription_event", turn2))

        val snap = projector.snapshot()
        assertEquals(2, (snap["turns"] as List<*>).size)

        val allJson = projector.allTurnsProjectionJson()
        assertTrue(allJson.contains("t-adp-1"))
        assertTrue(allJson.contains("t-adp-2"))
    }

    // ── helpers ─────────────────────────────────────────────────────────

    private fun sseEvent(eventName: String, data: String): SseEventStream.Event {
        return SseEventStream.Event(
            eventName = eventName,
            data = JsonParser.parseString(data).asJsonObject,
        )
    }

    private fun adpEvent(frameKind: String, data: String): AdpEventStream.Event {
        return AdpEventStream.Event(
            frameKind = frameKind,
            frame = JsonParser.parseString(data).asJsonObject,
        )
    }
}

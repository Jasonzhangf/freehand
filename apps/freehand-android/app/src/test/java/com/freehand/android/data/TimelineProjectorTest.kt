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

    // ── fallbackTurnsJson ───────────────────────────────────────────────

    @Test
    fun `fallbackTurnsJson returns turns array`() {
        val json = projector.fallbackTurnsJson()
        assertTrue(json.contains("turns"))
    }

    // ── helpers ─────────────────────────────────────────────────────────

    private fun sseEvent(eventName: String, data: String): SseEventStream.Event {
        return SseEventStream.Event(
            eventName = eventName,
            data = JsonParser.parseString(data).asJsonObject,
        )
    }
}

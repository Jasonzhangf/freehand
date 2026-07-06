package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Transforms `ui.protocol` ADP/SSE events into a UI-safe turn timeline.
 * This is the ONLY truth source for Android UI state.
 *
 * Android does NOT own session truth; it only renders what ui.protocol projects.
 *
 * Public projection shape (matches `UiPublicTurnProjection` from
 * `crates/freehand-ui-protocol`):
 *   { turn: { source_agent_id, source_node_id, session_id, turn_id,
 *            user_text, reasoning[], text[], tool_calls[], usage[],
 *            terminal_status, terminal_text },
 *     public_conversation: [ { kind, title, body, status } ] }
 *
 * @see `docs/function-maps/ui.protocol.md`
 * @see `docs/function-maps/app.android-client.md`
 */
class TimelineProjector {

    private val turnOrder = CopyOnWriteArrayList<String>()
    private val turns = LinkedHashMap<String, TurnCard>()
    private val slaves = LinkedHashMap<String, SlaveState>()
    private var currentAgentId: String = ""
    private var currentAgentName: String = ""
    private var connectionState: String = "idle"
    private var turnState: String = "idle"
    private val gson = Gson()

    /**
     * Safe string extraction from Gson JsonElement.
     * Gson returns JsonNull (not Kotlin null) for JSON null values;
     * calling .asString on JsonNull throws UnsupportedOperationException.
     */
    private fun JsonElement?.asStringSafe(): String? {
        if (this == null || this.isJsonNull || !this.isJsonPrimitive) return null
        return try { this.asString } catch (_: Exception) { null }
    }

    private fun JsonElement?.asBooleanSafe(): Boolean? {
        if (this == null || this.isJsonNull || !this.isJsonPrimitive) return null
        return try { this.asBoolean } catch (_: Exception) { null }
    }
    // Latest raw UiPublicTurnProjection body from the daemon SSE `turn` event.
    // When present it carries the canonical { turn, public_conversation } shape
    // that bridge.html renders directly, so the bridge receives the same wire
    // JSON the daemon published without any re-serialisation round-trip.
    private var latestRawTurnProjection: JsonObject? = null
    // Per-turn accumulated public projections for multi-turn bridge rendering.
    // Each entry is the canonical { turn, public_conversation } shape rebuilt
    // from projector state whenever applyTurnProjection / applyError / applyTerminal mutate a turn.
    private val rawProjectionsByTurn: LinkedHashMap<String, JsonObject> = LinkedHashMap()

    /** Stable iteration order of turns. */
    val orderedTurns: List<TurnCard> get() = turnOrder.mapNotNull { turns[it] }
    val orderedSlaves: List<Pair<String, SlaveState>> get() = slaves.toList()
    val activeAgentId: String get() = currentAgentId
    val activeAgentName: String get() = currentAgentName

    /**
     * Apply a JSON object from the SSE compatibility stream.
     * The event name from SSE is the `event` field of [SseEventStream.Event];
     * the data is a JSON object matching `UiSubscriptionEvent` shape.
     */
    fun apply(event: SseEventStream.Event) {
        when (event.eventName) {
            "turn" -> applyTurnEnvelope(event.data)
            "progress" -> applyProgress(event.data)
            "node_status" -> applyNodeStatus(event.data)
            "error" -> applyError(event.data)
            "terminal" -> applyTerminal(event.data)
            "checkpoints" -> /* observation only on Android */ Unit
            "debug" -> /* observation only on Android */ Unit
        }
    }

    /**
     * Apply a UiAdpResponse frame from the daemon `/adp` WebSocket.
     */
    fun applyAdp(event: AdpEventStream.Event) {
        when (event.frameKind) {
            "subscription_accepted" -> {
                connectionState = "open"
                turnState = "waiting"
            }
            "query_result" -> applyAdpQueryResult(event.frame.getAsJsonObject("result"))
            "subscription_event" -> {
                val subscriptionEvent = event.frame.objectField("event") ?: return
                applyAdpProjection(subscriptionEvent.objectField("projection"))
            }
            "failure" -> {
                val failure = event.frame.getAsJsonObject("failure")
                turnState = "error"
                connectionState = "error"
                latestRawTurnProjection = JsonObject().apply {
                    add("turn", JsonObject())
                    add("public_conversation", JsonArray().apply {
                        add(JsonObject().apply {
                            addProperty("kind", "Error")
                            addProperty("title", "Connection")
                            addProperty("body", failure?.get("message").asStringSafe() ?: "connection failure")
                            addProperty("status", "failed")
                        })
                    })
                }
            }
        }
    }

    private fun applyAdpQueryResult(result: JsonObject?) {
        val turn = result?.objectField("Turn")
        if (turn != null) {
            if (turn.entrySet().isEmpty()) return
            applyTurnProjection(turn)
            return
        }
        val debug = result?.objectField("Debug")
        if (debug != null) {
            turnState = debug.get("status_text").asStringSafe() ?: turnState
        }
    }

    private fun applyAdpProjection(projection: JsonObject?) {
        val turn = projection?.objectField("Turn")
        if (turn != null) {
            // Canonical ADP Turn projection carries full public_conversation; capture it per-turn.
            if (projection.has("latest_active_turn_id")) {
                val turnId = turn.get("turn_id")?.asStringSafe()
                    ?: turn.getAsJsonObject("source")?.get("source_turn_id")?.asStringSafe()
                if (turnId != null) {
                    rawProjectionsByTurn[turnId] = JsonObject().apply {
                        add("turn", turn.deepCopy())
                        add("public_conversation", buildPublicConversationFromTurn(turn))
                    }
                }
            } else {
                // Legacy ADP shape without latest_active_turn_id wrapper; derive from Turn payload.
                val turnId = turn.get("turn_id")?.asStringSafe()
                if (turnId != null) {
                    rawProjectionsByTurn[turnId] = JsonObject().apply {
                        add("turn", turn.deepCopy())
                        add("public_conversation", buildPublicConversationFromTurn(turn))
                    }
                }
            }
            applyTurnProjection(turn)
            return
        }
        val progress = projection?.objectField("Progress")
        if (progress != null) {
            applyProgress(progress)
            return
        }
        val nodeStatus = projection?.objectField("NodeStatus")
        if (nodeStatus != null) {
            applyNodeStatus(nodeStatus)
            return
        }
        val debug = projection?.objectField("Debug")
        if (debug != null) {
            turnState = debug.get("status_text").asStringSafe() ?: turnState
        }
    }

    private fun applyTurnEnvelope(data: JsonObject) {
        // The SSE `turn` event body is a UiSubscriptionEvent whose projection field
        // is a UiProjection::Turn(UiTurnProjection). The wire format serialises to
        // { turn: UiTurnProjection, public_conversation: Vec<UiConversationItem> }.
        // When the daemon sends this canonical shape we keep it verbatim so the
        // bridge receives the same wire JSON the daemon published, preserving the
        // exact public_conversation ordering without re-serialisation artifacts.
        if (data.has("turn") && data.has("public_conversation")) {
            latestRawTurnProjection = data.deepCopy()
            val turnJson = data.getAsJsonObject("turn")
            val turnId = turnJson?.get("turn_id")?.asStringSafe()
            if (turnId != null) {
                rawProjectionsByTurn[turnId] = data.deepCopy()
            }
        }
        val turnJson = if (data.has("turn") && data.get("turn").isJsonObject) {
            data.getAsJsonObject("turn")
        } else {
            data
        }
        applyTurnProjection(turnJson, data.takeIf { it.has("turn") && it.has("public_conversation") })
    }

    private fun applyTurnProjection(turnJson: JsonObject, canonicalPublicProjection: JsonObject? = null) {
        val turn = parseTurnProjection(turnJson) ?: return
        latestRawTurnProjection = canonicalPublicProjection?.deepCopy() ?: publicProjectionFromTurn(turnJson)
        // Non-canonical ADP Turn shape (no top-level public_conversation): rebuild per-turn.
        if (canonicalPublicProjection == null) {
            rawProjectionsByTurn[turn.turnId] = latestRawTurnProjection!!.deepCopy()
        }
        turns[turn.turnId] = turn
        if (!turnOrder.contains(turn.turnId)) turnOrder.add(turn.turnId)
        currentAgentId = turn.sourceAgentId
        turnState = turn.terminalStatus?.asStateString() ?: "running"
    }

    private fun applyProgress(data: JsonObject) {
        val statusText = data.get("status_text").asStringSafe()
        if (!statusText.isNullOrBlank()) {
            turnState = statusText
            // Progress events target the latest active turn; surface as a system card
            // on any tracked turn projection so streaming state is visible.
            rebuildRawProjectionForLatest(statusText)
        }
    }

    private fun applyNodeStatus(data: JsonObject) {
        val source = data.getAsJsonObject("source") ?: return
        val nodeId = data.get("node_id").asStringSafe() ?: return
        val pairingState = data.get("pairing_state").asStringSafe() ?: "unknown"
        val sourceAgent = source.get("source_agent_id").asStringSafe() ?: return
        slaves[sourceAgent] = SlaveState(nodeId = nodeId, pairingState = pairingState)
        if (data.get("healthy").asBooleanSafe() == false) {
            turnState = "blocked"
        }
    }

    private fun applyError(data: JsonObject) {
        val turnId = data.get("turn_id").asStringSafe() ?: return
        val message = data.get("message").asStringSafe() ?: "error"
        val prev = turns[turnId] ?: return
        turns[turnId] = prev.copy(
            terminalStatus = "error",
            terminalText = message,
            toolCalls = prev.toolCalls + "ERR: $message",
        )
        rebuildRawProjection(turnId)
        turnState = "error"
    }

    private fun applyTerminal(data: JsonObject) {
        val turnId = data.get("turn_id").asStringSafe() ?: return
        val status = data.get("status").asStringSafe() ?: "done"
        val summary = data.get("summary").asStringSafe() ?: ""
        val prev = turns[turnId] ?: return
        turns[turnId] = prev.copy(
            terminalStatus = status,
            terminalText = summary.ifBlank { prev.terminalText },
        )
        rebuildRawProjection(turnId)
        turnState = status
    }

    fun setConnectionState(state: String) {
        connectionState = state
    }

    fun setCurrentAgent(agentId: String, agentName: String) {
        currentAgentId = agentId
        currentAgentName = agentName.ifBlank { agentId }
    }

    /** Clear all accumulated turn state. Used before a fresh ADP connection so stale
     *  turns from a previous session do not mix with live subscription events. */
    fun clearAccumulatedTurns() {
        turnOrder.clear()
        turns.clear()
        rawProjectionsByTurn.clear()
        slaves.clear()
        latestRawTurnProjection = null
        // Preserve connection and agent identity; they are set explicitly by caller.
    }

    private fun parseTurnProjection(json: JsonObject): TurnCard? {
        val source = json.getAsJsonObject("source")
        val turnId = json.get("turn_id").asStringSafe() ?: return null
        return TurnCard(
            sourceAgentId = source?.get("source_agent_id").asStringSafe()
                ?: json.get("source_agent_id").asStringSafe().orEmpty(),
            sourceNodeId = source?.get("source_node_id").asStringSafe()
                ?: json.get("source_node_id").asStringSafe().orEmpty(),
            sessionId = json.get("session_id").asStringSafe().orEmpty(),
            turnId = turnId,
            userText = json.get("user_text").asStringSafe().orEmpty(),
            reasoning = json.getAsJsonArrayOrEmpty("reasoning").map { it.asString },
            text = json.getAsJsonArrayOrEmpty("text").map { it.asString },
            toolCalls = json.getAsJsonArrayOrEmpty("tool_calls").map { it.asString },
            usage = json.getAsJsonArrayOrEmpty("usage").map { it.asString },
            terminalStatus = json.get("terminal_status").asStringSafe(),
            terminalText = json.get("terminal_text").asStringSafe(),
        )
    }

    private fun publicProjectionFromTurn(turnJson: JsonObject): JsonObject {
        val publicConversation = JsonArray()
        turnJson.get("user_text").asStringSafe()?.takeIf { it.isNotBlank() }?.let { text ->
            publicConversation.add(conversationItem("UserText", "User", text, "submitted"))
        }
        turnJson.getAsJsonArrayOrEmpty("text").mapNotNull { it.asStringSafe() }.forEach { text ->
            if (text.isNotBlank()) {
                publicConversation.add(conversationItem("AssistantText", "Assistant", text, "streaming"))
            }
        }
        turnJson.getAsJsonArrayOrEmpty("tool_activities").forEach { element ->
            val tool = element.asJsonObject
            val status = tool.get("status").asStringSafe()?.lowercase() ?: "waiting"
            val toolName = tool.get("tool_name").asStringSafe() ?: "tool"
            publicConversation.add(conversationItem("ToolSummary", toolName, status, status).apply {
                tool.get("tool_call_id").asStringSafe()?.let { addProperty("tool_call_id", it) }
            })
        }
        turnJson.get("terminal_text").asStringSafe()?.takeIf { it.isNotBlank() }?.let { text ->
            publicConversation.add(
                conversationItem(
                    "Terminal",
                    "Final",
                    text,
                    when (turnJson.get("terminal_status").asStringSafe()?.lowercase()) {
                        "failed" -> "failed"
                        "cancelled" -> "cancelled"
                        "blocked" -> "blocked"
                        "interrupted" -> "interrupted"
                        else -> "completed"
                    },
                ),
            )
        }
        turnJson.getAsJsonArrayOrEmpty("errors").mapNotNull { it.asStringSafe() }.forEach { error ->
            publicConversation.add(conversationItem("Error", "Error", error, "failed"))
        }
        return JsonObject().apply {
            add("turn", turnJson.deepCopy())
            add("public_conversation", publicConversation)
        }
    }

    private fun conversationItem(kind: String, title: String, body: String, status: String): JsonObject =
        JsonObject().apply {
            addProperty("kind", kind)
            addProperty("title", title)
            addProperty("body", body)
            addProperty("status", status)
        }

    private fun JsonObject.getAsJsonArrayOrEmpty(name: String): JsonArray =
        this.getAsJsonArray(name) ?: JsonArray()

    private fun JsonObject.objectField(name: String): JsonObject? {
        val value = get(name) ?: return null
        if (value.isJsonNull || !value.isJsonObject) return null
        return value.asJsonObject
    }


    /** Build a canonical public_conversation JsonArray from a raw Turn payload (no public_conversation). */
    private fun buildPublicConversationFromTurn(turnJson: JsonObject): JsonArray {
        val pc = JsonArray()
        turnJson.get("user_text").asStringSafe()?.takeIf { it.isNotBlank() }?.let { text ->
            pc.add(conversationItem("UserText", "User", text, "submitted"))
        }
        turnJson.getAsJsonArrayOrEmpty("text").mapNotNull { it.asStringSafe() }.forEach { text ->
            if (text.isNotBlank()) pc.add(conversationItem("AssistantText", "Assistant", text, "streaming"))
        }
        turnJson.getAsJsonArrayOrEmpty("tool_activities").forEach { element ->
            val tool = element.asJsonObject
            val status = tool.get("status").asStringSafe()?.lowercase() ?: "waiting"
            val toolName = tool.get("tool_name").asStringSafe() ?: "tool"
            pc.add(conversationItem("ToolSummary", toolName, status, status).apply {
                tool.get("tool_call_id").asStringSafe()?.let { addProperty("tool_call_id", it) }
            })
        }
        turnJson.get("terminal_text").asStringSafe()?.takeIf { it.isNotBlank() }?.let { text ->
            pc.add(conversationItem("Terminal", "Final", text, terminalStatusClass(turnJson.get("terminal_status").asStringSafe())))
        }
        turnJson.getAsJsonArrayOrEmpty("errors").mapNotNull { it.asStringSafe() }.forEach { error ->
            pc.add(conversationItem("Error", "Error", error, "failed"))
        }
        return pc
    }

    /** Rebuild per-turn raw projection from stored TurnCard state after mutation. */
    private fun rebuildRawProjection(turnId: String) {
        val card = turns[turnId] ?: return
        // Start from existing projection if available, otherwise build fresh.
        val base = rawProjectionsByTurn[turnId]?.deepCopy()
            ?: JsonObject().apply { add("turn", JsonObject()); add("public_conversation", JsonArray()) }

        val turnNode = base.getAsJsonObject("turn") ?: JsonObject().also { base.add("turn", it) }
        if (card.userText.isNotBlank()) turnNode.addProperty("user_text", card.userText)
        if (card.terminalStatus != null) turnNode.addProperty("terminal_status", card.terminalStatus)
        if (card.terminalText != null) turnNode.addProperty("terminal_text", card.terminalText)

        val pc = base.getAsJsonArray("public_conversation") ?: JsonArray().also { base.add("public_conversation", it) }
        // Replace existing Error/Terminal items for this turn, keep User/Assistant/Tool intact.
        val filtered = JsonArray()
        for (i in 0 until pc.size()) {
            val item = pc[i].asJsonObject
            when (item.get("kind")?.asString) {
                "Error", "Terminal" -> { /* drop stale terminal/error; will re-add below */ }
                else -> filtered.add(item.deepCopy())
            }
        }

        if (card.terminalStatus != null && card.terminalText?.isNotBlank() == true) {
            filtered.add(conversationItem("Terminal", "Final", card.terminalText, terminalStatusClass(card.terminalStatus)))
        } else if (card.terminalStatus == "error" || card.toolCalls.any { it.startsWith("ERR:") }) {
            val errMsg = card.toolCalls.firstOrNull { it.startsWith("ERR:") }?.removePrefix("ERR: ") ?: card.terminalText ?: "error"
            filtered.add(conversationItem("Error", "Error", errMsg, "failed"))
        }

        base.remove("public_conversation")
        base.add("public_conversation", filtered)
        rawProjectionsByTurn[turnId] = base
    }

    /** Append a progress system card to the latest tracked turn's raw projection. */
    private fun rebuildRawProjectionForLatest(statusText: String) {
        val latestTurnId = turnOrder.lastOrNull() ?: return
        val base = rawProjectionsByTurn[latestTurnId]?.deepCopy()
            ?: JsonObject().apply { add("turn", JsonObject()); add("public_conversation", JsonArray()) }

        val pc = base.getAsJsonArray("public_conversation") ?: JsonArray().also { base.add("public_conversation", it) }
        // Replace any prior System/Progress item to avoid duplicates.
        val filtered = JsonArray()
        for (i in 0 until pc.size()) {
            val item = pc[i].asJsonObject
            if (item.get("kind")?.asString != "System") filtered.add(item.deepCopy())
        }
        filtered.add(conversationItem("System", "Status", statusText, "running"))

        base.remove("public_conversation")
        base.add("public_conversation", filtered)
        rawProjectionsByTurn[latestTurnId] = base
    }

    private fun terminalStatusClass(status: String?): String = when (status?.lowercase()) {
        "failed" -> "failed"
        "cancelled" -> "cancelled"
        "blocked" -> "blocked"
        "interrupted" -> "interrupted"
        else -> "completed"
    }

    /** Emit all accumulated turns as a JSON array of UiPublicTurnProjection shapes for the bridge. */
    fun allTurnsProjectionJson(): String? {
        if (rawProjectionsByTurn.isEmpty()) return null
        val arr = JsonArray()
        for ((_, proj) in rawProjectionsByTurn) {
            arr.add(proj.deepCopy())
        }
        return JsonObject().apply { add("all_turns", arr) }.toString()
    }

    fun snapshot(): Map<String, Any?> {
        val orderedTurnsMap: List<Map<String, Any?>> = orderedTurns.map { card ->
            mapOf(
                "id" to card.turnId,
                "session_id" to card.sessionId,
                "source_agent_id" to card.sourceAgentId,
                "source_node_id" to card.sourceNodeId,
                "user_text" to card.userText,
                "reasoning" to card.reasoning,
                "text" to card.text,
                "tool_calls" to card.toolCalls,
                "usage" to card.usage,
                "terminal_status" to card.terminalStatus,
                "terminal_text" to card.terminalText,
            )
        }
        val slavesMap: Map<String, Any?> = slaves.mapValues { (_, v) -> mapOf(
            "node_id" to v.nodeId,
            "pairing_state" to v.pairingState,
        ) }
        // Prefer the canonical daemon wire shape (turn + public_conversation);
        // fall back to the legacy flat `turns` list only when no public projection
        // has been received yet. When both are present we emit both so any
        // historical consumer of the flat list still works.
        val latestTurn = latestRawTurnProjection?.deepCopy()
        return mapOf(
            "agent_id" to currentAgentId,
            "agent_name" to currentAgentName,
            "connection" to connectionState,
            "turn_state" to turnState,
            "slaves" to slavesMap,
            "latest_turn" to latestTurn,
            "turns" to orderedTurnsMap,
        )
    }

    fun snapshotJson(): String = gson.toJson(snapshot())

    fun latestTurnProjectionJson(): String? = latestRawTurnProjection?.toString()

    fun fallbackTurnsJson(): String = gson.toJson(
        mapOf(
            "turns" to orderedTurns.map { card ->
                mapOf(
                    "id" to card.turnId,
                    "session_id" to card.sessionId,
                    "source_agent_id" to card.sourceAgentId,
                    "source_node_id" to card.sourceNodeId,
                    "user_text" to card.userText,
                    "reasoning" to card.reasoning,
                    "text" to card.text,
                    "tool_calls" to card.toolCalls,
                    "usage" to card.usage,
                    "terminal_status" to card.terminalStatus,
                    "terminal_text" to card.terminalText,
                )
            }
        )
    )
}

data class TurnCard(
    val sourceAgentId: String,
    val sourceNodeId: String,
    val sessionId: String,
    val turnId: String,
    val userText: String,
    val reasoning: List<String>,
    val text: List<String>,
    val toolCalls: List<String>,
    val usage: List<String>,
    val terminalStatus: String?,
    val terminalText: String?,
)

data class SlaveState(
    val nodeId: String,
    val pairingState: String,
)

private fun String.asStateString(): String = when (this) {
    "Success", "success", "Done", "done" -> "done"
    "Error", "error", "Failed", "failed" -> "error"
    "Blocked", "blocked" -> "blocked"
    "Cancelled", "cancelled" -> "cancelled"
    else -> "running"
}

package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Transforms `ui.protocol` SSE events into a UI-safe turn timeline.
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

    /** Stable iteration order of turns. */
    val orderedTurns: List<TurnCard> get() = turnOrder.mapNotNull { turns[it] }
    val orderedSlaves: List<Pair<String, SlaveState>> get() = slaves.toList()
    val activeAgentId: String get() = currentAgentId
    val activeAgentName: String get() = currentAgentName

    /**
     * Apply a JSON object from the SSE event stream.
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

    private fun applyTurnEnvelope(data: JsonObject) {
        // The SSE `turn` event body is a UiSubscriptionEvent whose projection field
        // is a UiProjection::Turn(UiTurnProjection). The wire format serialises to
        // { turn: UiTurnProjection, public_conversation: Vec<UiConversationItem> }.
        // When the daemon sends this canonical shape we keep it verbatim so the
        // bridge receives the same wire JSON the daemon published, preserving the
        // exact public_conversation ordering without re-serialisation artifacts.
        if (data.has("turn") && data.has("public_conversation")) {
            latestRawTurnProjection = data.deepCopy()
        }
        val turnJson = if (data.has("turn") && data.get("turn").isJsonObject) {
            data.getAsJsonObject("turn")
        } else {
            data
        }
        val turn = parseTurnProjection(turnJson) ?: return
        turns[turn.turnId] = turn
        if (!turnOrder.contains(turn.turnId)) turnOrder.add(turn.turnId)
        currentAgentId = turn.sourceAgentId
        turnState = turn.terminalStatus?.asStateString() ?: "running"
    }

    private fun applyProgress(data: JsonObject) {
        val statusText = data.get("status_text").asStringSafe()
        if (!statusText.isNullOrBlank()) turnState = statusText
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
        turnState = status
    }

    fun setConnectionState(state: String) {
        connectionState = state
    }

    fun setCurrentAgent(agentId: String, agentName: String) {
        currentAgentId = agentId
        currentAgentName = agentName.ifBlank { agentId }
    }

    private fun parseTurnProjection(json: JsonObject): TurnCard? {
        val turnId = json.get("turn_id").asStringSafe() ?: return null
        return TurnCard(
            sourceAgentId = json.get("source_agent_id").asStringSafe().orEmpty(),
            sourceNodeId = json.get("source_node_id").asStringSafe().orEmpty(),
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

    private fun JsonObject.getAsJsonArrayOrEmpty(name: String): JsonArray =
        this.getAsJsonArray(name) ?: JsonArray()

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

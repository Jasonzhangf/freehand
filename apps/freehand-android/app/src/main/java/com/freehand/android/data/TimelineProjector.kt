package com.freehand.android.data

import com.google.gson.Gson
import com.google.gson.JsonObject
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Transforms `ui.protocol` projection events into a UI-safe turn timeline.
 * This is the ONLY truth source for Android UI state.
 *
 * Android does NOT own session truth; it only renders what ui.protocol projects.
 *
 * @see `docs/function-maps/ui.protocol.md`
 * @see `docs/design/android-client-v1-execution.md`
 */
class TimelineProjector {

    private val turns = CopyOnWriteArrayList<TurnCard>()
    private var currentAgent: String = ""
    private var connectionState: String = "idle"
    private var turnState: String = "idle"
    private val gson = Gson()

    /**
     * Apply a turn projection event from ui.protocol SSE / query.
     * Only accepts events from ui.protocol; no local invented state.
     */
    fun applyTurnEvent(event: JsonObject) {
        val type = event.get("type")?.asString ?: return
        when (type) {
            "turn" -> applyTurnProjection(event)
            "progress" -> applyProgress(event)
            "error" -> applyError(event)
            "terminal" -> applyTerminal(event)
        }
    }

    private fun applyTurnProjection(event: JsonObject) {
        val turnId = event.get("turn_id")?.asString ?: return
        val userText = event.get("user_text")?.asString ?: ""
        val assistantText = event.get("assistant_text")?.asString ?: ""
        val status = event.get("status")?.asString ?: "done"

        val existing = turns.indexOfFirst { it.turnId == turnId }
        val card = TurnCard(
            turnId = turnId,
            userText = userText,
            assistantText = assistantText,
            status = status,
        )
        if (existing >= 0) {
            turns[existing] = card
        } else {
            turns.add(card)
        }
    }

    private fun applyProgress(event: JsonObject) {
        val status = event.get("status")?.asString ?: "running"
        turnState = status
    }

    private fun applyError(event: JsonObject) {
        val turnId = event.get("turn_id")?.asString ?: return
        val message = event.get("message")?.asString ?: "error"
        val idx = turns.indexOfFirst { it.turnId == turnId }
        if (idx >= 0) {
            val prev = turns[idx]
            turns[idx] = prev.copy(status = "error", assistantText = message)
        }
    }

    private fun applyTerminal(event: JsonObject) {
        val turnId = event.get("turn_id")?.asString ?: return
        val status = event.get("status")?.asString ?: "done"
        val summary = event.get("summary")?.asString ?: ""
        val idx = turns.indexOfFirst { it.turnId == turnId }
        if (idx >= 0) {
            val prev = turns[idx]
            turns[idx] = prev.copy(status = status, assistantText = summary)
        }
        turnState = status
    }

    fun setConnectionState(state: String) {
        connectionState = state
    }

    fun setCurrentAgent(name: String) {
        currentAgent = name
    }

    fun snapshot(): Map<String, Any?> = mapOf(
        "agent" to currentAgent,
        "connection" to connectionState,
        "turn_state" to turnState,
        "turns" to turns.map { mapOf(
            "id" to it.turnId,
            "user" to it.userText,
            "assistant" to it.assistantText,
            "status" to it.status,
        ) },
    )

    fun snapshotJson(): String = gson.toJson(snapshot())
}

data class TurnCard(
    val turnId: String,
    val userText: String,
    val assistantText: String,
    val status: String,
)

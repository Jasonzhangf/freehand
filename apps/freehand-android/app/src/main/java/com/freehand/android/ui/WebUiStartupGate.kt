package com.freehand.android.ui

import com.google.gson.JsonParser

/** Accepts only the canonical daemon-hosted Android WebUI shell probe. */
object WebUiStartupGate {
    data class Verdict(
        val ready: Boolean,
        val status: String,
    )

    fun isCanonicalProbe(rawProbe: String?): Boolean {
        return evaluate(rawProbe).ready
    }

    fun evaluate(rawProbe: String?): Verdict {
        if (rawProbe.isNullOrBlank() || rawProbe == "null") {
            return Verdict(false, "Waiting for canonical WebUI shell")
        }
        return runCatching {
            var value = JsonParser.parseString(rawProbe)
            if (value.isJsonPrimitive && value.asJsonPrimitive.isString) {
                value = JsonParser.parseString(value.asString)
            }
            val probe = value.asJsonObject
            when {
                probe.get("webuiShell")?.asBoolean != true ->
                    Verdict(false, "Waiting for canonical WebUI shell")

                probe.get("layoutClient")?.asString != "android-webview" ->
                    Verdict(false, "Waiting for Android WebUI client")

                probe.get("webuiCssApplied")?.asBoolean != true ->
                    Verdict(false, "Waiting for WebUI stylesheet")

                probe.get("webuiJsReady")?.asBoolean != true ->
                    Verdict(false, "Waiting for WebUI JavaScript")

                else -> Verdict(true, "Workspace ready")
            }
        }.getOrDefault(Verdict(false, "Waiting for canonical WebUI shell"))
    }
}

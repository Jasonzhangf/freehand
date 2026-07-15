package com.freehand.android.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WebUiStartupGateTest {
    @Test
    fun `canonical Android WebUI probe is ready`() {
        assertTrue(
            WebUiStartupGate.isCanonicalProbe(
                """{"webuiShell":true,"layoutClient":"android-webview","layoutShape":"tall_phone","webuiCssApplied":true,"webuiJsReady":true}""",
            ),
        )
    }

    @Test
    fun `JSON encoded canonical probe is ready`() {
        assertTrue(
            WebUiStartupGate.isCanonicalProbe(
                "\"{\\\"webuiShell\\\":true,\\\"layoutClient\\\":\\\"android-webview\\\",\\\"webuiCssApplied\\\":true,\\\"webuiJsReady\\\":true}\"",
            ),
        )
    }

    @Test
    fun `page finish without canonical shell remains not ready`() {
        assertFalse(WebUiStartupGate.isCanonicalProbe(null))
        assertFalse(WebUiStartupGate.isCanonicalProbe("not-json"))
        assertFalse(
            WebUiStartupGate.isCanonicalProbe(
                """{"webuiShell":false,"layoutClient":"android-webview"}""",
            ),
        )
        assertFalse(
            WebUiStartupGate.isCanonicalProbe(
                """{"webuiShell":true,"layoutClient":"desktop","webuiCssApplied":true,"webuiJsReady":true}""",
            ),
        )
    }

    @Test
    fun `canonical shell without assets remains not ready`() {
        assertEquals(
            "Waiting for WebUI stylesheet",
            WebUiStartupGate.evaluate(
                """{"webuiShell":true,"layoutClient":"android-webview","webuiCssApplied":false,"webuiJsReady":true}""",
            ).status,
        )
        assertEquals(
            "Waiting for WebUI JavaScript",
            WebUiStartupGate.evaluate(
                """{"webuiShell":true,"layoutClient":"android-webview","webuiCssApplied":true,"webuiJsReady":false}""",
            ).status,
        )
        assertFalse(
            WebUiStartupGate.isCanonicalProbe(
                """{"webuiShell":true,"layoutClient":"android-webview","webuiCssApplied":false,"webuiJsReady":true}""",
            ),
        )
        assertFalse(
            WebUiStartupGate.isCanonicalProbe(
                """{"webuiShell":true,"layoutClient":"android-webview","webuiCssApplied":true,"webuiJsReady":false}""",
            ),
        )
    }
}

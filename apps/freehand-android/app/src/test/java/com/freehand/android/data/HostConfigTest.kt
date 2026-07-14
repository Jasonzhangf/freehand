package com.freehand.android.data

import org.junit.Assert.assertEquals
import org.junit.Test

class HostConfigTest {
    @Test
    fun `baseUrl constructs canonical daemon origin`() {
        assertEquals(
            "http://100.66.1.82:4041",
            HostConfig("100.66.1.82", 4041).baseUrl,
        )
    }

    @Test
    fun `different host and port produce their exact daemon origin`() {
        assertEquals(
            "http://freehand-tailnet:4042",
            HostConfig("freehand-tailnet", 4042).baseUrl,
        )
    }
}

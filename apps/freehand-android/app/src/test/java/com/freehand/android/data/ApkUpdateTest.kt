package com.freehand.android.data

import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files

class ApkUpdateTest {
    @Test
    fun `manifest parses release metadata`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 2,
              "versionName": "0.2.0",
              "apkUrl": "https://example.invalid/freehand.apk",
              "releaseNotes": "test release",
              "required": true
            }
            """.trimIndent(),
        )

        assertEquals(2L, manifest.versionCode)
        assertEquals("0.2.0", manifest.versionName)
        assertTrue(manifest.required)
        assertTrue(manifest.isNewerThan(1))
        assertFalse(manifest.isNewerThan(2))
    }

    @Test
    fun `check resolves daemon relative apk url`() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody(
            """
            {
              "versionCode": 2,
              "versionName": "0.2.0",
              "apkUrl": "/android/freehand-android.apk",
              "releaseNotes": "",
              "required": false
            }
            """.trimIndent(),
        ))
        server.start()
        try {
            val client = ApkUpdateClient(OkHttpClient(), currentVersionCode = 1)
            val result = client.check(server.url("/android/update.json").toString())

            assertTrue(result.updateAvailable)
            assertEquals(
                server.url("/android/freehand-android.apk").toString(),
                result.manifest?.apkUrl,
            )
        } finally {
            server.shutdown()
        }
    }

    @Test
    fun `download writes non empty apk file`() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("fake-apk"))
        server.start()
        try {
            val dir = Files.createTempDirectory("freehand-apk-update").toFile()
            val target = File(dir, "freehand.apk")
            val manifest = ApkUpdateManifest(
                versionCode = 2,
                versionName = "0.2.0",
                apkUrl = server.url("/freehand.apk").toString(),
                releaseNotes = "",
                required = false,
            )

            val apk = ApkUpdateClient(OkHttpClient(), currentVersionCode = 1)
                .download(manifest, target)

            assertTrue(apk.exists())
            assertEquals("fake-apk", apk.readText())
        } finally {
            server.shutdown()
        }
    }

    @Test
    fun `check treats missing manifest as no update`() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(404))
        server.start()
        try {
            val client = ApkUpdateClient(OkHttpClient(), currentVersionCode = 1)
            val result = client.check(server.url("/android/update.json").toString())

            assertFalse(result.updateAvailable)
            assertNull(result.manifest)
        } finally {
            server.shutdown()
        }
    }
}

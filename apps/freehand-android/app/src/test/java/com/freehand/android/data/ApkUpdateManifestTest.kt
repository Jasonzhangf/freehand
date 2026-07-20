package com.freehand.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ApkUpdateManifestTest {
    @Test
    fun `manifest with higher version creates install plan`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 2,
              "versionName": "0.2.0",
              "apkUrl": "/android/freehand-android.apk",
              "releaseNotes": "new build",
              "required": true
            }
            """.trimIndent(),
        )

        val plan = manifest.updatePlan(
            currentVersionCode = 1,
            hostConfig = HostConfig("100.66.1.82", 4042),
        )

        assertEquals(2L, plan?.versionCode)
        assertEquals("0.2.0", plan?.versionName)
        assertEquals("http://100.66.1.82:4042/android/freehand-android.apk", plan?.apkUrl)
        assertTrue(plan?.required == true)
    }

    @Test
    fun `manifest at current version creates no plan`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 1,
              "apkUrl": "/android/freehand-android.apk"
            }
            """.trimIndent(),
        )

        assertNull(
            manifest.updatePlan(
                currentVersionCode = 1,
                hostConfig = HostConfig("100.66.1.82", 4042),
            ),
        )
    }

    @Test
    fun `relay manifest relative apk url stays under relay daemon namespace`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 3,
              "apkUrl": "/android/freehand-android.apk"
            }
            """.trimIndent(),
        )

        val plan = manifest.updatePlan(
            currentVersionCode = 1,
            hostConfig = HostConfig(
                host = "100.66.1.82",
                port = 44042,
                webUrlOverride = "http://100.66.1.82:44042/relay/daemon/studio-host/",
            ),
        )

        assertEquals(
            "http://100.66.1.82:44042/relay/daemon/studio-host/android/freehand-android.apk",
            plan?.apkUrl,
        )
    }

    @Test
    fun `manifest rejects unknown fields`() {
        val error = try {
            ApkUpdateManifest.parse("""{"versionCode":2,"apkUrl":"/a.apk","debug":true}""")
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("unsupported fields"))
        assertTrue(error?.message.orEmpty().contains("debug"))
    }

    @Test
    fun `manifest rejects non-positive version code`() {
        val error = try {
            ApkUpdateManifest.parse("""{"versionCode":0,"apkUrl":"/a.apk"}""")
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("versionCode must be positive"))
    }

    @Test
    fun `manifest rejects non-http absolute apk url`() {
        val error = try {
            ApkUpdateManifest.parse("""{"versionCode":2,"apkUrl":"file:///tmp/freehand.apk"}""")
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("apkUrl absolute URL scheme must be http(s)"))
    }
}

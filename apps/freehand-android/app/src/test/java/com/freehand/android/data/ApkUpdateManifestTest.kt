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
              "sha256": "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
              "size": 45000000,
              "signerSha256": "ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda",
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
        assertEquals(
            "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
            plan?.sha256,
        )
        assertEquals(45000000L, plan?.size)
    }

    @Test
    fun `manifest at current version creates no plan`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 1,
              "apkUrl": "/android/freehand-android.apk",
              "sha256": "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
              "size": 45000000,
              "signerSha256": "ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"
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
              "apkUrl": "/android/freehand-android.apk",
              "sha256": "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
              "size": 45000000,
              "signerSha256": "ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"
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
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda","debug":true}""",
            )
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
            ApkUpdateManifest.parse(
                """{"versionCode":0,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("versionCode must be positive"))
    }

    @Test
    fun `manifest rejects non-http absolute apk url`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"file:///tmp/freehand.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("apkUrl absolute URL scheme must be http(s)"))
    }

    @Test
    fun `manifest admits sha256 and size fields`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 5,
              "versionName": "0.3.0",
              "apkUrl": "/android/freehand-android.apk",
              "sha256": "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
              "size": 45000000,
              "signerSha256": "ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda",
              "required": false,
              "releaseNotes": "dual-path upgrade support"
            }
            """.trimIndent(),
        )

        val plan = manifest.updatePlan(
            currentVersionCode = 1,
            hostConfig = HostConfig("100.66.1.82", 4042),
        )

        assertEquals(5L, plan?.versionCode)
        assertEquals("0.3.0", plan?.versionName)
        assertEquals(
            "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
            plan?.sha256,
        )
        assertEquals(45000000L, plan?.size)
    }

    @Test
    fun `manifest sha256 upper case is normalized to lower case`() {
        val manifest = ApkUpdateManifest.parse(
            """
            {
              "versionCode": 5,
              "apkUrl": "/android/freehand-android.apk",
              "sha256": "979906F579118625C1F57E6DB1EEF8F055475ED884ABB93934E180D0B8F14611",
              "size": 45000000,
              "signerSha256": "ECD63A2C2070970735CC079B0BB090427CA0B59200DA0EBC07C80B50A1DFFFDA"
            }
            """.trimIndent(),
        )

        val plan = manifest.updatePlan(
            currentVersionCode = 1,
            hostConfig = HostConfig("100.66.1.82", 4042),
        )

        // must be normalized to lowercase
        assertEquals(
            "979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611",
            plan?.sha256,
        )
    }

    @Test
    fun `manifest rejects invalid sha256`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"not-a-valid-hex","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("sha256 must be a 64-char lowercase hex digest"))
    }

    @Test
    fun `manifest rejects sha256 wrong length`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f146","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("sha256 must be a 64-char lowercase hex digest"))
    }

    @Test
    fun `manifest rejects non-positive size`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":0,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("size must be positive"))
    }

    @Test
    fun `higher version update requires sha256 and size`() {
        val missingSha256 = try {
            ApkUpdateManifest.parse("""{"versionCode":2,"apkUrl":"/a.apk","size":1,"signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""").updatePlan(
                currentVersionCode = 1,
                hostConfig = HostConfig("100.66.1.82", 4042),
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }
        val missingSize = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","signerSha256":"ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda"}""",
            ).updatePlan(
                currentVersionCode = 1,
                hostConfig = HostConfig("100.66.1.82", 4042),
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(missingSha256?.message.orEmpty().contains("sha256 is required for a higher-version APK"))
        assertTrue(missingSize?.message.orEmpty().contains("size is required for a higher-version APK"))
    }

    @Test
    fun `higher version update requires signer sha256`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":1}""",
            ).updatePlan(
                currentVersionCode = 1,
                hostConfig = HostConfig("100.66.1.82", 4042),
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("signerSha256 is required for a higher-version APK"))
    }

    @Test
    fun `manifest rejects invalid signer sha256`() {
        val error = try {
            ApkUpdateManifest.parse(
                """{"versionCode":2,"apkUrl":"/a.apk","sha256":"979906f579118625c1f57e6db1eef8f055475ed884abb93934e180d0b8f14611","size":1,"signerSha256":"invalid"}""",
            )
            null
        } catch (caught: ApkUpdateManifestException) {
            caught
        }

        assertTrue(error?.message.orEmpty().contains("signerSha256 must be a 64-char lowercase hex digest"))
    }

    @Test
    fun `current version manifest does not require integrity metadata`() {
        val manifest = ApkUpdateManifest.parse(
            """{"versionCode":1,"apkUrl":"/a.apk"}""",
        )

        assertNull(
            manifest.updatePlan(
                currentVersionCode = 1,
                hostConfig = HostConfig("100.66.1.82", 4042),
            ),
        )
    }
}

package com.freehand.android.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ApkUpdateStatusTest {
    @Test
    fun `status helpers expose stable phases for WebUI bridge`() {
        assertEquals("checking", ApkUpdateStatus.checking("http://daemon/android/update.json").phase)

        val current = ApkUpdateStatus.current(7)
        assertEquals("current", current.phase)
        assertEquals(7L, current.versionCode)
        assertNull(current.apkUrl)

        val available = ApkUpdateStatus.available(
            versionCode = 8,
            versionName = "0.8.0",
            apkUrl = "http://daemon/android/freehand-android.apk",
        )
        assertEquals("available", available.phase)
        assertEquals(8L, available.versionCode)
        assertEquals("0.8.0", available.versionName)
        assertEquals("http://daemon/android/freehand-android.apk", available.apkUrl)

        assertEquals("downloading", ApkUpdateStatus.downloading(8, available.apkUrl!!).phase)
        assertEquals("downloaded", ApkUpdateStatus.downloaded(8, 1024).phase)
        assertEquals("installer_started", ApkUpdateStatus.installerStarted(8, "0.8.0").phase)
        assertEquals("already_checking", ApkUpdateStatus.alreadyChecking().phase)
        assertEquals("failed", ApkUpdateStatus.failed(IllegalStateException("boom")).phase)
    }
}

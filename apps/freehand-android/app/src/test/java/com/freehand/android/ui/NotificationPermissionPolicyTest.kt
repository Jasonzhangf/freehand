package com.freehand.android.ui

import android.Manifest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class NotificationPermissionPolicyTest {
    @Test
    fun `notification permission is Android 13 plus only`() {
        assertNull(NotificationPermissionPolicy.runtimePermissionForSdk(32))
        assertEquals(
            Manifest.permission.POST_NOTIFICATIONS,
            NotificationPermissionPolicy.runtimePermissionForSdk(33),
        )
        assertEquals(
            Manifest.permission.POST_NOTIFICATIONS,
            NotificationPermissionPolicy.runtimePermissionForSdk(34),
        )
    }

    @Test
    fun `startup prompt is once per package install while notification permission is missing`() {
        assertFalse(
            NotificationPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = -1,
                currentInstallMarker = 1_000,
                permissionMissing = false,
            ),
        )
        assertTrue(
            NotificationPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = -1,
                currentInstallMarker = 1_000,
                permissionMissing = true,
            ),
        )
        assertFalse(
            NotificationPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = 1_000,
                currentInstallMarker = 1_000,
                permissionMissing = true,
            ),
        )
        assertTrue(
            NotificationPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = 1_000,
                currentInstallMarker = 2_000,
                permissionMissing = true,
            ),
        )
    }
}

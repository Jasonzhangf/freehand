package com.freehand.android.ui

import android.Manifest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class FileAccessPermissionPolicyTest {
    @Test
    fun `runtime permission list follows Android storage model`() {
        assertEquals(emptyList<String>(), FileAccessPermissionPolicy.runtimePermissionsForSdk(22))
        assertEquals(
            listOf(
                Manifest.permission.READ_EXTERNAL_STORAGE,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            ),
            FileAccessPermissionPolicy.runtimePermissionsForSdk(28),
        )
        assertEquals(
            listOf(Manifest.permission.READ_EXTERNAL_STORAGE),
            FileAccessPermissionPolicy.runtimePermissionsForSdk(32),
        )
        assertEquals(
            listOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VIDEO,
                Manifest.permission.READ_MEDIA_AUDIO,
                Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED,
            ),
            FileAccessPermissionPolicy.runtimePermissionsForSdk(34),
        )
    }

    @Test
    fun `all files settings is Android 11 plus only`() {
        assertFalse(FileAccessPermissionPolicy.allFilesSettingsAvailableForSdk(29))
        assertTrue(FileAccessPermissionPolicy.allFilesSettingsAvailableForSdk(30))
        assertTrue(FileAccessPermissionPolicy.allFilesSettingsAvailableForSdk(34))
    }

    @Test
    fun `startup prompt is once per package install while access is missing`() {
        assertFalse(
            FileAccessPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = -1,
                currentInstallMarker = 1_000,
                missingRuntimePermissionCount = 0,
                needsAllFilesAccess = false,
            ),
        )
        assertTrue(
            FileAccessPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = -1,
                currentInstallMarker = 1_000,
                missingRuntimePermissionCount = 2,
                needsAllFilesAccess = true,
            ),
        )
        assertFalse(
            FileAccessPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = 1_000,
                currentInstallMarker = 1_000,
                missingRuntimePermissionCount = 2,
                needsAllFilesAccess = true,
            ),
        )
        assertTrue(
            FileAccessPermissionPolicy.shouldPromptForInstall(
                promptedInstallMarker = 1_000,
                currentInstallMarker = 2_000,
                missingRuntimePermissionCount = 0,
                needsAllFilesAccess = true,
            ),
        )
    }
}

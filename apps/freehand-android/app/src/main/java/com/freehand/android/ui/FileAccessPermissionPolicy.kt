package com.freehand.android.ui

import android.Manifest

object FileAccessPermissionPolicy {
    const val PREFS_NAME = "freehand-file-access"
    const val PROMPTED_INSTALL_MARKER_KEY = "prompted_install_marker"

    fun runtimePermissionsForSdk(sdkInt: Int): List<String> = when {
        sdkInt >= 34 -> listOf(
            Manifest.permission.READ_MEDIA_IMAGES,
            Manifest.permission.READ_MEDIA_VIDEO,
            Manifest.permission.READ_MEDIA_AUDIO,
            Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED,
        )
        sdkInt >= 33 -> listOf(
            Manifest.permission.READ_MEDIA_IMAGES,
            Manifest.permission.READ_MEDIA_VIDEO,
            Manifest.permission.READ_MEDIA_AUDIO,
        )
        sdkInt >= 29 -> listOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        sdkInt >= 23 -> listOf(
            Manifest.permission.READ_EXTERNAL_STORAGE,
            Manifest.permission.WRITE_EXTERNAL_STORAGE,
        )
        else -> emptyList()
    }

    fun allFilesSettingsAvailableForSdk(sdkInt: Int): Boolean = sdkInt >= 30

    fun shouldPromptForInstall(
        promptedInstallMarker: Long,
        currentInstallMarker: Long,
        missingRuntimePermissionCount: Int,
        needsAllFilesAccess: Boolean,
    ): Boolean {
        if (missingRuntimePermissionCount <= 0 && !needsAllFilesAccess) {
            return false
        }
        return promptedInstallMarker != currentInstallMarker
    }
}

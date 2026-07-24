package com.freehand.android.ui

import android.Manifest

object NotificationPermissionPolicy {
    const val PREFS_NAME = "freehand-notifications"
    const val PROMPTED_INSTALL_MARKER_KEY = "prompted_install_marker"
    const val NOTIFIED_TURNS_KEY = "notified_turns"

    fun runtimePermissionForSdk(sdkInt: Int): String? =
        if (sdkInt >= 33) Manifest.permission.POST_NOTIFICATIONS else null

    fun shouldPromptForInstall(
        promptedInstallMarker: Long,
        currentInstallMarker: Long,
        permissionMissing: Boolean,
    ): Boolean {
        if (!permissionMissing) return false
        return promptedInstallMarker != currentInstallMarker
    }
}

package com.freehand.android.data

import android.content.Context
import java.io.File

/**
 * Android Context adapter for the protocol-only daemon connection config.
 *
 * The authoritative mobile daemon endpoint is an app-owned JSON file. The
 * bundled asset is only the first-run bootstrap input.
 */
object ClientConfig {
    private const val CONFIG_PATH = "config/client.json"

    fun store(context: Context): DaemonConnectionConfigStore =
        DaemonConnectionConfigStore(
            configFile = File(context.filesDir, DaemonConnectionConfig.DEFAULT_CONFIG_FILE),
            bundledConfigReader = {
                context.assets.open(CONFIG_PATH).bufferedReader().use { it.readText() }
            },
        )

    fun load(context: Context): DaemonConnectionConfig = store(context).load()
}

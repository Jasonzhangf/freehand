package com.freehand.android.data

import android.content.Context
import androidx.core.content.edit

class HostStore(context: Context) {

    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun load(): HostConfig {
        val host = prefs.getString(KEY_HOST, null) ?: DEFAULT_HOST
        val port = prefs.getInt(KEY_PORT, DEFAULT_PORT)
        return HostConfig(host, port)
    }

    fun save(host: HostConfig) {
        prefs.edit {
            putString(KEY_HOST, host.host)
            putInt(KEY_PORT, host.port)
        }
    }

    companion object {
        private const val PREFS = "freehand_host"
        private const val KEY_HOST = "host"
        private const val KEY_PORT = "port"
        const val DEFAULT_HOST = "100.66.1.82"
        const val DEFAULT_PORT = 4041
    }
}

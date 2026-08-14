package com.freehand.android.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidApkUpdaterGenerationTest {
    @Test
    fun `new updater generation invalidates a prior in-flight check`() {
        val first = AndroidApkUpdater.nextGeneration()
        val second = AndroidApkUpdater.nextGeneration()

        assertFalse(AndroidApkUpdater.isGenerationCurrent(first))
        assertTrue(AndroidApkUpdater.isGenerationCurrent(second))
    }

    @Test
    fun `same generation remains current`() {
        val current = AndroidApkUpdater.nextGeneration()
        assertTrue(AndroidApkUpdater.isGenerationCurrent(current))
    }

    @Test
    fun `runIfCurrent runs installer action only for the current generation`() {
        val current = AndroidApkUpdater.nextGeneration()
        var ran = false
        AndroidApkUpdater.runIfCurrent(current) { ran = true }
        assertTrue(ran)
    }

    @Test
    fun `runIfCurrent skips installer action for a stale generation`() {
        val stale = AndroidApkUpdater.nextGeneration()
        AndroidApkUpdater.nextGeneration()
        var ran = false
        AndroidApkUpdater.runIfCurrent(stale) { ran = true }
        assertFalse(ran)
    }
}

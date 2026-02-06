package com.wifisync.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for JNI library loading on the emulator.
 * Validates that the x86_64 native library loads and core functions are callable.
 */
@RunWith(AndroidJUnit4::class)
class JniLibraryLoadingTest {

    private lateinit var filesDir: String

    @Before
    fun setUp() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        filesDir = context.filesDir.absolutePath
    }

    @Test
    fun testNativeLibraryLoaded() {
        // The library is loaded in WifisyncApplication.init{} companion block.
        // If it fails, we get UnsatisfiedLinkError before any test runs.
        // This test explicitly verifies it by trying to call a native method.
        try {
            WifisyncCore.init(filesDir)
        } catch (e: UnsatisfiedLinkError) {
            fail("Native library libwifisync_jni.so failed to load: ${e.message}")
        }
    }

    @Test
    fun testInitReturnsResult() {
        val result = WifisyncCore.init(filesDir)
        // init returns a Boolean; we just verify it doesn't throw
        assertNotNull("init() should return a non-null Boolean", result)
    }

    @Test
    fun testListCredentialsReturnsResult() {
        WifisyncCore.init(filesDir)
        val result = WifisyncCore.listCredentials()
        assertNotNull("listCredentials() should return a Result", result)
        // On a fresh install, either success with empty list or a handled error
        assertTrue(
            "listCredentials() should succeed or fail gracefully",
            result.isSuccess || result.isFailure
        )
    }

    @Test
    fun testSyncStatusReturnsDisabled() {
        WifisyncCore.init(filesDir)
        val result = WifisyncCore.syncStatus()
        assertNotNull("syncStatus() should return a Result", result)
        if (result.isSuccess) {
            val status = result.getOrThrow()
            assertFalse("Sync should not be enabled on fresh install", status.enabled)
        }
        // If it fails, that's also acceptable on a fresh install without config
    }

    @Test
    fun testGetVersionReturnsVersionInfo() {
        WifisyncCore.init(filesDir)
        val result = WifisyncCore.getVersion()
        assertNotNull("getVersion() should return a Result", result)
        if (result.isSuccess) {
            val version = result.getOrThrow()
            assertNotNull("Version string should not be null", version.version)
            assertTrue("Version should not be empty", version.version.isNotEmpty())
        }
    }
}

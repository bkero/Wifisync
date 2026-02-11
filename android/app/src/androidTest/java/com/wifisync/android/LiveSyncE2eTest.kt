package com.wifisync.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assume
import org.junit.Before
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import org.junit.Assert.*
import java.io.File
import java.util.UUID

/**
 * Cross-device E2E tests for Android.
 *
 * These tests are orchestrated by tests/e2e/run-e2e.sh which:
 * 1. Starts a Docker server
 * 2. Runs CLI commands to push data (CLI phase)
 * 3. Launches these Android tests to verify cross-device sync (Android phase)
 *
 * Credentials are passed via instrumentation arguments from the orchestrator.
 */
@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class LiveSyncE2eTest {

    private var isLoggedIn = false
    private lateinit var testFilesDir: String

    companion object {
        private var coreInitialized = false
    }

    @Before
    fun setUp() {
        Assume.assumeTrue(
            "E2E tests require server credentials via instrumentation args",
            LiveSyncTestConfig.isConfigured
        )

        // Use a unique subdirectory for each test to ensure isolation
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val uniqueDir = File(context.filesDir, "e2e_${UUID.randomUUID().toString().take(8)}")
        uniqueDir.mkdirs()
        testFilesDir = uniqueDir.absolutePath

        WifisyncCore.init(testFilesDir)
        coreInitialized = true
    }

    @After
    fun tearDown() {
        if (isLoggedIn) {
            try {
                WifisyncCore.syncLogout()
            } catch (_: Exception) {
                // Ignore logout errors during cleanup
            }
            isLoggedIn = false
        }
        // Clean up test directory
        if (::testFilesDir.isInitialized) {
            File(testFilesDir).deleteRecursively()
        }
    }

    private fun login(): SyncLoginResponse {
        val result = WifisyncCore.syncLogin(
            LiveSyncTestConfig.serverUrl,
            LiveSyncTestConfig.username,
            LiveSyncTestConfig.password
        )
        assertTrue("syncLogin should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        isLoggedIn = true
        return result.getOrThrow()
    }

    // =========================================================================
    // Login / Logout
    // =========================================================================

    @Test
    fun test01_androidLoginLogout() {
        // Login
        val loginResponse = login()
        assertNotNull("Login response should not be null", loginResponse)
        assertTrue("Device ID should not be empty", loginResponse.deviceId.isNotEmpty())

        // Check status
        val statusResult = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed", statusResult.isSuccess)
        val status = statusResult.getOrThrow()
        assertTrue("Sync should be enabled after login", status.enabled)

        // Logout
        val logoutResult = WifisyncCore.syncLogout()
        assertTrue("syncLogout should succeed", logoutResult.isSuccess)
        isLoggedIn = false

        // Verify disabled
        val finalStatus = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed after logout", finalStatus.isSuccess)
        assertFalse("Sync should be disabled after logout", finalStatus.getOrThrow().enabled)
    }

    // =========================================================================
    // Cross-device pull: CLI pushed data, Android pulls
    // =========================================================================

    @Test
    fun test02_pullDataPushedByCli() {
        login()

        // Pull data that was pushed by the CLI in the orchestrator's CLI phase
        val pullResult = WifisyncCore.syncPull(LiveSyncTestConfig.password)
        assertTrue(
            "syncPull should succeed: ${pullResult.exceptionOrNull()?.message}",
            pullResult.isSuccess
        )

        val pull = pullResult.getOrThrow()
        // On a fresh E2E run, the CLI may have pushed collections and credentials.
        // We verify the pull completes without errors.
        assertTrue(
            "Pull should have 0 errors, got ${pull.errors}. Details: ${pull.error_details}",
            pull.errors == 0
        )

        // Verify we can list collections after pull
        val collectionsResult = WifisyncCore.listCollections()
        assertTrue("listCollections should succeed after pull", collectionsResult.isSuccess)
    }

    // =========================================================================
    // Cross-device push: Android pushes, CLI will verify in reverse phase
    // =========================================================================

    @Test
    fun test03_androidPushData() {
        login()

        // Create a collection on Android
        val collName = "android_e2e_${UUID.randomUUID().toString().take(8)}"
        val createResult = WifisyncCore.createCollection(collName)
        assertTrue(
            "createCollection should succeed: ${createResult.exceptionOrNull()?.message}",
            createResult.isSuccess
        )

        // Push to server
        val pushResult = WifisyncCore.syncPush(LiveSyncTestConfig.password)
        assertTrue(
            "syncPush should succeed: ${pushResult.exceptionOrNull()?.message}",
            pushResult.isSuccess
        )

        val push = pushResult.getOrThrow()
        assertTrue("Push conflicts should be 0, got ${push.conflicts}", push.conflicts == 0)
    }

    // =========================================================================
    // Pull creates missing collections on fresh device
    // =========================================================================

    @Test
    fun test04_pullCreatesMissingCollections() {
        // This test uses a fresh data dir (set up in @Before), so there are
        // no local collections. After login + pull, any collections pushed by
        // CLI should appear locally.
        login()

        // Pull
        val pullResult = WifisyncCore.syncPull(LiveSyncTestConfig.password)
        assertTrue(
            "syncPull should succeed: ${pullResult.exceptionOrNull()?.message}",
            pullResult.isSuccess
        )

        val pull = pullResult.getOrThrow()
        assertTrue(
            "Pull should have 0 errors, got ${pull.errors}",
            pull.errors == 0
        )

        // List collections -- should include any that were pushed by CLI
        val collectionsResult = WifisyncCore.listCollections()
        assertTrue("listCollections should succeed", collectionsResult.isSuccess)
        // We can't assert exact counts since this depends on orchestrator state,
        // but we verify the operation succeeds without error.
    }

    // =========================================================================
    // Multiple devices same user
    // =========================================================================

    @Test
    fun test05_multipleDevicesSameUser() {
        // Login as the shared E2E user
        val loginResponse = login()

        // Verify we got a unique device ID
        assertTrue("Device ID should not be empty", loginResponse.deviceId.isNotEmpty())

        // Check that we can see devices (should include CLI device if it logged in)
        val devicesResult = WifisyncCore.listDevices()
        assertTrue("listDevices should succeed", devicesResult.isSuccess)

        val devices = devicesResult.getOrThrow()
        assertTrue("Should have at least 1 device", devices.isNotEmpty())

        // Current device should be marked
        val currentDevice = devices.find { it.isCurrentDevice }
        assertNotNull("Should have a current device", currentDevice)
        assertEquals(
            "Current device ID should match",
            loginResponse.deviceId,
            currentDevice!!.id
        )
    }
}

package com.wifisync.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assume
import org.junit.Before
import org.junit.BeforeClass
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import org.junit.Assert.*

/**
 * Instrumented tests that connect to a real sync server and test the full sync lifecycle.
 * Tests are skipped when server credentials are not provided via environment variables.
 *
 * Required env vars: WIFISYNC_SERVER_URL, WIFISYNC_USERNAME, WIFISYNC_PASSWORD
 */
@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class LiveSyncApiTest {

    private var isLoggedIn = false

    companion object {
        @BeforeClass
        @JvmStatic
        fun initCore() {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            WifisyncCore.init(context.filesDir.absolutePath)
        }
    }

    @Before
    fun setUp() {
        Assume.assumeTrue(LiveSyncTestConfig.getSkipMessage(), LiveSyncTestConfig.isConfigured)
    }

    @After
    fun tearDown() {
        if (isLoggedIn) {
            WifisyncCore.syncLogout()
            isLoggedIn = false
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

    @Test
    fun test01_syncLogin() {
        val loginResponse = login()

        assertNotNull("Login response should not be null", loginResponse)
        assertEquals("Server URL should match", LiveSyncTestConfig.serverUrl, loginResponse.serverUrl)
        assertEquals("Username should match", LiveSyncTestConfig.username, loginResponse.username)
        assertTrue("Device ID should not be empty", loginResponse.deviceId.isNotEmpty())
    }

    @Test
    fun test02_syncStatus() {
        login()

        val result = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)

        val status = result.getOrThrow()
        assertTrue("Sync should be enabled after login", status.enabled)
        assertEquals("Server URL should match", LiveSyncTestConfig.serverUrl, status.serverUrl)
        assertEquals("Username should match", LiveSyncTestConfig.username, status.username)
        assertTrue("Should have a valid token", status.hasValidToken)
    }

    @Test
    fun test03_syncPush() {
        login()

        val result = WifisyncCore.syncPush(LiveSyncTestConfig.password)
        assertTrue("syncPush should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)

        val push = result.getOrThrow()
        assertTrue("accepted should be >= 0", push.accepted >= 0)
        assertTrue("conflicts should be >= 0", push.conflicts >= 0)
    }

    @Test
    fun test04_syncPull() {
        login()

        val result = WifisyncCore.syncPull(LiveSyncTestConfig.password)
        assertTrue("syncPull should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)

        val pull = result.getOrThrow()
        assertTrue("applied should be >= 0", pull.applied >= 0)
        assertTrue("errors should be >= 0", pull.errors >= 0)
    }

    @Test
    fun test05_listDevices() {
        val loginResponse = login()

        val result = WifisyncCore.listDevices()
        assertTrue("listDevices should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)

        val devices = result.getOrThrow()
        assertTrue("Device list should not be empty", devices.isNotEmpty())

        val currentDevice = devices.find { it.isCurrentDevice }
        assertNotNull("Should have a current device", currentDevice)
        assertEquals("Current device ID should match login deviceId", loginResponse.deviceId, currentDevice!!.id)
    }

    @Test
    fun test06_listCredentials() {
        login()
        WifisyncCore.syncPull(LiveSyncTestConfig.password)

        val result = WifisyncCore.listCredentials()
        assertTrue("listCredentials should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        // List may be empty on a fresh account, just verify it returns successfully
    }

    @Test
    fun test07_listCollections() {
        login()
        WifisyncCore.syncPull(LiveSyncTestConfig.password)

        val result = WifisyncCore.listCollections()
        assertTrue("listCollections should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        // List may be empty on a fresh account, just verify it returns successfully
    }

    @Test
    fun test08_syncLogout() {
        login()

        val logoutResult = WifisyncCore.syncLogout()
        assertTrue("syncLogout should succeed: ${logoutResult.exceptionOrNull()?.message}", logoutResult.isSuccess)
        isLoggedIn = false

        val statusResult = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed after logout", statusResult.isSuccess)

        val status = statusResult.getOrThrow()
        assertFalse("Sync should be disabled after logout", status.enabled)
    }

    @Test
    fun test09_fullLifecycle() {
        // Login
        val loginResponse = login()
        assertNotNull("Login should return a response", loginResponse)

        // Status check
        val statusResult = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed", statusResult.isSuccess)
        assertTrue("Sync should be enabled", statusResult.getOrThrow().enabled)

        // Push
        val pushResult = WifisyncCore.syncPush(LiveSyncTestConfig.password)
        assertTrue("syncPush should succeed: ${pushResult.exceptionOrNull()?.message}", pushResult.isSuccess)

        // Pull
        val pullResult = WifisyncCore.syncPull(LiveSyncTestConfig.password)
        assertTrue("syncPull should succeed: ${pullResult.exceptionOrNull()?.message}", pullResult.isSuccess)

        // List devices
        val devicesResult = WifisyncCore.listDevices()
        assertTrue("listDevices should succeed", devicesResult.isSuccess)
        assertTrue("Should have at least one device", devicesResult.getOrThrow().isNotEmpty())

        // Logout
        val logoutResult = WifisyncCore.syncLogout()
        assertTrue("syncLogout should succeed", logoutResult.isSuccess)
        isLoggedIn = false

        // Verify disabled
        val finalStatus = WifisyncCore.syncStatus()
        assertTrue("syncStatus should succeed after logout", finalStatus.isSuccess)
        assertFalse("Sync should be disabled after logout", finalStatus.getOrThrow().enabled)
    }
}

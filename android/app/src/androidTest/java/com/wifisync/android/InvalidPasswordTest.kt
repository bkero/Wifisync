package com.wifisync.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assume
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.Assert.*
import java.io.File
import java.util.UUID

/**
 * E2E tests for invalid password scenarios on Android.
 *
 * Verifies that wrong passwords produce clear error messages instead of
 * crashes or silent corruption.
 */
@RunWith(AndroidJUnit4::class)
class InvalidPasswordTest {

    private var isLoggedIn = false
    private lateinit var testFilesDir: String

    @Before
    fun setUp() {
        Assume.assumeTrue(
            "Password tests require server credentials via instrumentation args",
            LiveSyncTestConfig.isConfigured
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val uniqueDir = File(context.filesDir, "e2e_pwd_${UUID.randomUUID().toString().take(8)}")
        uniqueDir.mkdirs()
        testFilesDir = uniqueDir.absolutePath

        WifisyncCore.init(testFilesDir)
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
        if (::testFilesDir.isInitialized) {
            File(testFilesDir).deleteRecursively()
        }
    }

    @Test
    fun testWrongPasswordPush() {
        // Login with correct password
        val loginResult = WifisyncCore.syncLogin(
            LiveSyncTestConfig.serverUrl,
            LiveSyncTestConfig.username,
            LiveSyncTestConfig.password
        )
        assertTrue("Login should succeed", loginResult.isSuccess)
        isLoggedIn = true

        // Attempt push with wrong password
        val pushResult = WifisyncCore.syncPush("completely_wrong_password_12345")
        assertTrue(
            "Push with wrong password should fail",
            pushResult.isFailure
        )

        val error = pushResult.exceptionOrNull()
        assertNotNull("Should have an error", error)
        // Verify error message mentions password
        val errorMsg = error!!.message ?: ""
        assertTrue(
            "Error should mention password issue, got: $errorMsg",
            errorMsg.lowercase().contains("password") ||
                errorMsg.lowercase().contains("auth") ||
                errorMsg.lowercase().contains("proof") ||
                errorMsg.lowercase().contains("mismatch")
        )
    }

    @Test
    fun testWrongPasswordPull() {
        // Login with correct password
        val loginResult = WifisyncCore.syncLogin(
            LiveSyncTestConfig.serverUrl,
            LiveSyncTestConfig.username,
            LiveSyncTestConfig.password
        )
        assertTrue("Login should succeed", loginResult.isSuccess)
        isLoggedIn = true

        // Attempt pull with wrong password
        val pullResult = WifisyncCore.syncPull("completely_wrong_password_12345")
        assertTrue(
            "Pull with wrong password should fail",
            pullResult.isFailure
        )

        val error = pullResult.exceptionOrNull()
        assertNotNull("Should have an error", error)
        val errorMsg = error!!.message ?: ""
        assertTrue(
            "Error should mention password issue, got: $errorMsg",
            errorMsg.lowercase().contains("password") ||
                errorMsg.lowercase().contains("auth") ||
                errorMsg.lowercase().contains("proof") ||
                errorMsg.lowercase().contains("mismatch")
        )
    }

    @Test
    fun testWrongPasswordLoginFails() {
        // The shared E2E user was registered with LiveSyncTestConfig.password.
        // Trying to login with a different password should fail.
        val loginResult = WifisyncCore.syncLogin(
            LiveSyncTestConfig.serverUrl,
            LiveSyncTestConfig.username,
            "this_is_definitely_the_wrong_password"
        )

        assertTrue(
            "Login with wrong password should fail",
            loginResult.isFailure
        )
    }
}

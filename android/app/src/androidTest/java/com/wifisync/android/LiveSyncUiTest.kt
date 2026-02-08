package com.wifisync.android

import android.Manifest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assume
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * UI integration tests for the sync feature via Compose.
 * Tests login, push, pull, and logout through the actual UI.
 *
 * Requires a running sync server with credentials provided via environment variables:
 * WIFISYNC_SERVER_URL, WIFISYNC_USERNAME, WIFISYNC_PASSWORD
 */
@RunWith(AndroidJUnit4::class)
class LiveSyncUiTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    private var isLoggedIn = false

    @Before
    fun setUp() {
        Assume.assumeTrue(LiveSyncTestConfig.getSkipMessage(), LiveSyncTestConfig.isConfigured)

        // Initialize core for API-level cleanup in @After
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        WifisyncCore.init(context.filesDir.absolutePath)
    }

    @After
    fun tearDown() {
        if (isLoggedIn) {
            WifisyncCore.syncLogout()
            isLoggedIn = false
        }
    }

    private fun navigateToSync() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()
    }

    private fun waitForSyncScreen() {
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Sync Status").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                try {
                    composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    false
                }
            }
        }
    }

    private fun apiLogin() {
        val result = WifisyncCore.syncLogin(
            LiveSyncTestConfig.serverUrl,
            LiveSyncTestConfig.username,
            LiveSyncTestConfig.password
        )
        if (result.isSuccess) {
            isLoggedIn = true
        }
    }

    @Test
    fun testLoginViaUi() {
        navigateToSync()

        // Wait for "Login to Server" button
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }

        // Open login dialog
        composeTestRule.onNodeWithText("Login to Server").performClick()
        composeTestRule.waitForIdle()

        // Fill in login fields
        composeTestRule.onNodeWithText("Server URL").performTextInput(LiveSyncTestConfig.serverUrl)
        composeTestRule.onNodeWithText("Username").performTextInput(LiveSyncTestConfig.username)
        composeTestRule.onNodeWithText("Master Password").performTextInput(LiveSyncTestConfig.password)

        // Click Login
        composeTestRule.onNodeWithText("Login").performClick()

        // Wait for "Logout" button to appear (indicates successful login)
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Logout").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }

        isLoggedIn = true
        composeTestRule.onNodeWithText("Logout").assertIsDisplayed()
    }

    @Test
    fun testSyncStatusAfterLogin() {
        apiLogin()
        navigateToSync()

        // Wait for status card to load with server info
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Sync Status").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }

        composeTestRule.onNodeWithText("Sync Status").assertIsDisplayed()
        composeTestRule.onNodeWithText(LiveSyncTestConfig.serverUrl).assertIsDisplayed()
        composeTestRule.onNodeWithText(LiveSyncTestConfig.username).assertIsDisplayed()
    }

    @Test
    fun testPushViaUi() {
        apiLogin()
        navigateToSync()
        waitForSyncScreen()

        // Click Push button
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Push").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
        composeTestRule.onNodeWithText("Push").performClick()
        composeTestRule.waitForIdle()

        // Fill password dialog
        composeTestRule.onNodeWithText("Push Changes").assertIsDisplayed()
        composeTestRule.onNodeWithText("Master Password").performTextInput(LiveSyncTestConfig.password)
        composeTestRule.onNodeWithText("Confirm").performClick()

        // Wait for push to complete (loading spinner disappears, result shown)
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Push").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
    }

    @Test
    fun testPullViaUi() {
        apiLogin()
        navigateToSync()
        waitForSyncScreen()

        // Click Pull button
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Pull").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
        composeTestRule.onNodeWithText("Pull").performClick()
        composeTestRule.waitForIdle()

        // Fill password dialog
        composeTestRule.onNodeWithText("Pull Changes").assertIsDisplayed()
        composeTestRule.onNodeWithText("Master Password").performTextInput(LiveSyncTestConfig.password)
        composeTestRule.onNodeWithText("Confirm").performClick()

        // Wait for pull to complete
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Pull").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
    }

    @Test
    fun testLogoutViaUi() {
        apiLogin()
        navigateToSync()

        // Wait for Logout button (indicates logged-in state)
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Logout").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }

        // Click Logout
        composeTestRule.onNodeWithText("Logout").performClick()

        // Wait for "Login to Server" to reappear (indicates logged-out state)
        composeTestRule.waitUntil(timeoutMillis = 15000) {
            try {
                composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }

        isLoggedIn = false
        composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
    }
}

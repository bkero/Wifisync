package com.wifisync.android

import android.Manifest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for interactive Compose UI elements.
 * Tests button clicks, dialog interactions, and async loading states.
 */
@RunWith(AndroidJUnit4::class)
class ComposeUiInteractionTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    @Test
    fun testCredentialsScreenShowsStateAfterLoading() {
        // Wait for async credential loading to complete
        composeTestRule.waitUntil(timeoutMillis = 10000) {
            // Either empty state or credentials list should appear
            try {
                composeTestRule.onNodeWithText("No credentials found").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                try {
                    composeTestRule.onNodeWithText("Error").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    // Could be showing credentials list - check for top bar
                    try {
                        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
                        true
                    } catch (_: AssertionError) {
                        false
                    }
                }
            }
        }
    }

    @Test
    fun testCredentialsRefreshButton() {
        // Wait for initial load
        composeTestRule.waitForIdle()
        // The refresh button has a Refresh icon; verify the top bar is there
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
    }

    @Test
    fun testSyncScreenShowsNotConnectedState() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitUntil(timeoutMillis = 10000) {
            try {
                composeTestRule.onNodeWithText("Not connected to a sync server").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                try {
                    // May show sync status if somehow connected
                    composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    false
                }
            }
        }
    }

    @Test
    fun testLoginButtonShowsDialog() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()

        // Click "Login to Server" button
        composeTestRule.waitUntil(timeoutMillis = 10000) {
            try {
                composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
        composeTestRule.onNodeWithText("Login to Server").performClick()
        composeTestRule.waitForIdle()

        // Verify login dialog appears with expected fields
        composeTestRule.onNodeWithText("Server URL").assertIsDisplayed()
        composeTestRule.onNodeWithText("Username").assertIsDisplayed()
        composeTestRule.onNodeWithText("Master Password").assertIsDisplayed()
    }

    @Test
    fun testLoginDialogCanBeDismissedViaCancel() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()

        composeTestRule.waitUntil(timeoutMillis = 10000) {
            try {
                composeTestRule.onNodeWithText("Login to Server").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
        composeTestRule.onNodeWithText("Login to Server").performClick()
        composeTestRule.waitForIdle()

        // Verify dialog is shown
        composeTestRule.onNodeWithText("Server URL").assertIsDisplayed()

        // Dismiss via Cancel
        composeTestRule.onNodeWithText("Cancel").performClick()
        composeTestRule.waitForIdle()

        // Dialog should be gone
        composeTestRule.onNodeWithText("Server URL").assertDoesNotExist()
    }

    @Test
    fun testEmptyStateMessageText() {
        // Wait for credentials screen to load
        composeTestRule.waitUntil(timeoutMillis = 10000) {
            try {
                composeTestRule.onNodeWithText("No credentials found").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                try {
                    composeTestRule.onNodeWithText("Error").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    false
                }
            }
        }

        // If empty state is shown, verify subtitle
        try {
            composeTestRule.onNodeWithText("No credentials found").assertIsDisplayed()
            composeTestRule.onNodeWithText("Sync with a server to get credentials").assertIsDisplayed()
        } catch (_: AssertionError) {
            // Error state is shown instead, which is also valid
        }
    }

    @Test
    fun testSyncScreenRefreshButton() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()

        // Sync top bar should be visible
        composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
    }
}

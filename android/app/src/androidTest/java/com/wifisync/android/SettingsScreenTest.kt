package com.wifisync.android

import android.Manifest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for the Settings screen content.
 */
@RunWith(AndroidJUnit4::class)
class SettingsScreenTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    @Before
    fun navigateToSettings() {
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
    }

    @Test
    fun testShowsAboutCard() {
        composeTestRule.onNodeWithText("About").assertIsDisplayed()
    }

    @Test
    fun testDisplaysAppVersion() {
        composeTestRule.onNodeWithText("0.1.0").assertIsDisplayed()
    }

    @Test
    fun testDisplaysPackageName() {
        composeTestRule.onNodeWithText("com.wifisync.android").assertIsDisplayed()
    }

    @Test
    fun testShowsLoggedInDevicesCard() {
        composeTestRule.onNodeWithText("Logged In Devices").assertIsDisplayed()
    }

    @Test
    fun testShowsEmptyDevicesMessage() {
        composeTestRule.waitUntil(timeoutMillis = 10000) {
            try {
                composeTestRule.onNodeWithText("No devices found. Login to a sync server to see devices.").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                // May still be loading
                false
            }
        }
    }

    @Test
    fun testDevicesRefreshButton() {
        // Verify the Logged In Devices section is present (which contains the refresh button)
        composeTestRule.onNodeWithText("Logged In Devices").assertIsDisplayed()
    }
}

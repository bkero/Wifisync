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
 * Instrumented tests for tab navigation interactions.
 * Verifies that clicking tabs navigates to the correct screens.
 */
@RunWith(AndroidJUnit4::class)
class NavigationInteractionTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    @Test
    fun testClickNetworksTab_showsWifiNetworksTopBar() {
        // Networks is the default tab, but click it explicitly
        composeTestRule.onNodeWithText("Networks").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
    }

    @Test
    fun testClickSyncTab_showsSyncScreen() {
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()
        // Sync screen top bar says "Sync"
        // and shows status content
        composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
    }

    @Test
    fun testClickSettingsTab_showsAboutCard() {
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("About").assertIsDisplayed()
    }

    @Test
    fun testNavigateBackToNetworksFromSettings() {
        // Go to Settings
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("About").assertIsDisplayed()

        // Go back to Networks
        composeTestRule.onNodeWithText("Networks").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
    }

    @Test
    fun testSequentialTabNavigation() {
        // Networks -> Sync -> Settings -> Networks
        composeTestRule.onNodeWithText("Networks").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()

        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("About").assertIsDisplayed()

        composeTestRule.onNodeWithText("Networks").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
    }

    @Test
    fun testNavigationBarVisibleAcrossTransitions() {
        // Verify navigation bar items remain visible on every screen

        // Networks screen
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()

        // Sync screen
        composeTestRule.onNodeWithText("Sync").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()

        // Settings screen
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Sync").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
    }
}

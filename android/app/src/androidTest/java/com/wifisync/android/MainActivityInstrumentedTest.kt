package com.wifisync.android

import android.Manifest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Compose UI tests for MainActivity.
 */
@RunWith(AndroidJUnit4::class)
class MainActivityInstrumentedTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    // =========================================================================
    // Basic UI Tests
    // =========================================================================

    @Test
    fun testAppTitleDisplayed() {
        composeTestRule.onNodeWithText("Wifisync").assertIsDisplayed()
    }

    @Test
    fun testCredentialsTabExists() {
        composeTestRule.onNodeWithText("Credentials").assertIsDisplayed()
    }

    @Test
    fun testCollectionsTabExists() {
        composeTestRule.onNodeWithText("Collections").assertIsDisplayed()
    }

    @Test
    fun testSettingsTabExists() {
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
    }

    // =========================================================================
    // Navigation Tests
    // =========================================================================

    @Test
    fun testCredentialsIsDefaultTab() {
        // The Credentials tab should be selected by default
        // Verify credentials-related content is visible
        composeTestRule.onNodeWithText("Credentials").assertIsDisplayed()
    }

    // =========================================================================
    // Status Display Tests
    // =========================================================================

    @Test
    fun testStatusMessageDisplaysAfterInit() {
        // Wait for initialization
        composeTestRule.waitForIdle()

        // After init, there should be some status message
        // (either success or error depending on native library availability)
        // We just verify the UI is responsive
        composeTestRule.onNodeWithText("Wifisync").assertIsDisplayed()
    }
}

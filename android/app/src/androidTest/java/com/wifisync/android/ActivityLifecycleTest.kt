package com.wifisync.android

import android.Manifest
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.lifecycle.Lifecycle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for activity lifecycle scenarios.
 * Validates that the app survives configuration changes and lifecycle transitions.
 */
@RunWith(AndroidJUnit4::class)
class ActivityLifecycleTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    @Test
    fun testActivityRecreationPreservesUi() {
        // Verify initial state
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()

        // Recreate activity (simulates configuration change like rotation)
        composeTestRule.activityRule.scenario.recreate()
        composeTestRule.waitForIdle()

        // UI should still be functional after recreation
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
    }

    @Test
    fun testRecreationOnSettingsTab() {
        // Navigate to Settings
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("About").assertIsDisplayed()

        // Recreate activity
        composeTestRule.activityRule.scenario.recreate()
        composeTestRule.waitForIdle()

        // Navigation should still work
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
    }

    @Test
    fun testPauseAndResume() {
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()

        // Move to STARTED (paused)
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.STARTED)

        // Resume
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        composeTestRule.waitForIdle()

        // UI should still be functional
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
    }

    @Test
    fun testStopAndRestart() {
        composeTestRule.onNodeWithText("WiFi Networks").assertIsDisplayed()

        // Move to CREATED (stopped)
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.CREATED)

        // Restart
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        composeTestRule.waitForIdle()

        // UI should still be functional
        composeTestRule.onNodeWithText("Networks").assertIsDisplayed()
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
    }
}

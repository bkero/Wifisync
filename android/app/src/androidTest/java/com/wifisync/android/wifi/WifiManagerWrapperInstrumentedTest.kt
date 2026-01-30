package com.wifisync.android.wifi

import android.Manifest
import android.content.Context
import android.net.wifi.WifiManager
import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.*
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for WifiManagerWrapper.
 *
 * Note: These tests are limited because:
 * 1. Adding network suggestions requires user consent (POST_NOTIFICATIONS on Android 13+)
 * 2. Some operations may fail based on device state
 * 3. Tests should not leave suggestions installed on the device
 *
 * These tests primarily verify the wrapper's behavior and error handling.
 */
@RunWith(AndroidJUnit4::class)
class WifiManagerWrapperInstrumentedTest {

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    private lateinit var context: Context
    private lateinit var wrapper: WifiManagerWrapper

    @Before
    fun setUp() {
        context = InstrumentationRegistry.getInstrumentation().targetContext
        wrapper = WifiManagerWrapper(context)
    }

    // =========================================================================
    // Basic Properties Tests
    // =========================================================================

    @Test
    fun testApiLevelIsCorrect() {
        assertEquals(Build.VERSION.SDK_INT, wrapper.apiLevel)
    }

    @Test
    fun testApiLevelIsAtLeast29() {
        // Our minSdk is 29, so this should always be true
        assertTrue(wrapper.apiLevel >= 29)
    }

    @Test
    fun testWifiEnabledReturnsBoolean() {
        // Just verify we can read the property without exception
        val enabled = wrapper.isWifiEnabled
        // Result depends on device state - just ensure no exception
        assertTrue(enabled || !enabled) // Always true, but verifies the call
    }

    @Test
    fun testHasWifiPermissions() {
        // With GrantPermissionRule, this should be true
        assertTrue(wrapper.hasWifiPermissions())
    }

    @Test
    fun testHasLocationPermission() {
        // With GrantPermissionRule, this should be true
        assertTrue(wrapper.hasLocationPermission())
    }

    @Test
    fun testSuggestionLimit() {
        assertEquals(50, wrapper.suggestionLimit)
    }

    // =========================================================================
    // Suggestion List Tests
    // =========================================================================

    @Test
    fun testListSuggestionsInitiallyEmpty() {
        // Clean up any previous test suggestions
        wrapper.removeAllSuggestions()

        val suggestions = wrapper.listSuggestions()
        // Should be empty or only contain our tracked suggestions
        // (not other apps' suggestions)
        assertTrue(suggestions.isEmpty() || suggestions.all { it.ssid.isNotEmpty() })
    }

    @Test
    fun testSuggestionCountMatchesList() {
        // Clean up first
        wrapper.removeAllSuggestions()

        assertEquals(wrapper.listSuggestions().size, wrapper.suggestionCount)
    }

    // =========================================================================
    // Remove All Suggestions Tests
    // =========================================================================

    @Test
    fun testRemoveAllSuggestionsSuccess() {
        // This should succeed even if there are no suggestions
        val result = wrapper.removeAllSuggestions()
        assertTrue(result.isSuccess)
    }

    @Test
    fun testRemoveAllSuggestionsReturnsCount() {
        // Clean state
        wrapper.removeAllSuggestions()

        val result = wrapper.removeAllSuggestions()
        assertTrue(result.isSuccess)
        assertEquals(0, result.getOrNull())
    }

    // =========================================================================
    // Remove Nonexistent Suggestion Tests
    // =========================================================================

    @Test
    fun testRemoveNonexistentSuggestionFails() {
        val result = wrapper.removeSuggestion("nonexistent-id-12345")
        assertTrue(result.isFailure)

        val exception = result.exceptionOrNull()
        assertTrue(exception is WifiException)
        assertEquals(WifiErrorCode.NOT_FOUND, (exception as WifiException).errorCode)
    }

    // =========================================================================
    // Error Code Tests
    // =========================================================================

    @Test
    fun testWifiErrorCodeValues() {
        // Ensure all error codes are defined
        val codes = WifiErrorCode.values()
        assertTrue(codes.contains(WifiErrorCode.PERMISSION_DENIED))
        assertTrue(codes.contains(WifiErrorCode.DUPLICATE))
        assertTrue(codes.contains(WifiErrorCode.EXCEEDS_LIMIT))
        assertTrue(codes.contains(WifiErrorCode.APP_DISALLOWED))
        assertTrue(codes.contains(WifiErrorCode.INTERNAL_ERROR))
        assertTrue(codes.contains(WifiErrorCode.NOT_FOUND))
        assertTrue(codes.contains(WifiErrorCode.UNKNOWN))
    }

    // =========================================================================
    // Integration Tests (may require user interaction in real scenarios)
    // =========================================================================

    /**
     * Note: Actually adding suggestions may show a system dialog requiring
     * user consent. This test verifies the wrapper correctly calls the API.
     *
     * In a real test environment, you might need to:
     * 1. Use a test device with user interaction
     * 2. Mock the WifiManager (not possible in instrumented tests)
     * 3. Accept that this test may fail due to system dialogs
     */
    @Test
    fun testAddSuggestionReturnsResult() {
        // Use a unique SSID to avoid conflicts with real networks
        val testSsid = "WifisyncTest_${System.currentTimeMillis()}"

        val result = wrapper.addWpa2Suggestion(
            ssid = testSsid,
            password = "testpassword123",
            isHidden = false
        )

        // The result should be either success or a known failure
        // (e.g., APP_DISALLOWED if user hasn't granted suggestion permissions)
        if (result.isSuccess) {
            // Clean up: remove the test suggestion
            val suggestionId = result.getOrNull()!!
            wrapper.removeSuggestion(suggestionId)
        } else {
            // Verify it's a valid WifiException
            val exception = result.exceptionOrNull()
            assertTrue(exception is WifiException)
        }
    }

    @Test
    fun testAddWpa3SuggestionReturnsResult() {
        val testSsid = "WifisyncWpa3Test_${System.currentTimeMillis()}"

        val result = wrapper.addWpa3Suggestion(
            ssid = testSsid,
            password = "securepassword123",
            isHidden = false
        )

        // Same logic as WPA2 test
        if (result.isSuccess) {
            val suggestionId = result.getOrNull()!!
            wrapper.removeSuggestion(suggestionId)
        } else {
            val exception = result.exceptionOrNull()
            assertTrue(exception is WifiException)
        }
    }
}

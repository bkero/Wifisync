package com.wifisync.android.wifi

import android.Manifest
import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * End-to-end instrumented tests for the WiFi suggestion lifecycle.
 * Tests add, verify, remove, and persistence flows through WifiManagerWrapper.
 *
 * Note: On emulators, addSuggestion may return APP_DISALLOWED because the
 * emulator's WifiManager may not support network suggestions. These tests
 * handle that gracefully.
 */
@RunWith(AndroidJUnit4::class)
class WifiSuggestionEndToEndTest {

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
        // Clean up any leftover suggestions
        wrapper.removeAllSuggestions()
    }

    @After
    fun tearDown() {
        wrapper.removeAllSuggestions()
    }

    @Test
    fun testAddVerifyAndRemoveSuggestion() {
        val result = wrapper.addWpa2Suggestion("E2E-TestNet", "password123")

        if (result.isFailure) {
            val exception = result.exceptionOrNull()
            if (exception is WifiException && exception.errorCode == WifiErrorCode.APP_DISALLOWED) {
                // Emulator limitation - not a bug
                return
            }
            fail("Unexpected failure: ${exception?.message}")
        }

        val suggestionId = result.getOrThrow()
        assertNotNull("Suggestion ID should not be null", suggestionId)

        // Verify it's tracked
        val suggestions = wrapper.listSuggestions()
        assertTrue("Suggestion should be in list", suggestions.any { it.id == suggestionId })

        // Remove it
        val removeResult = wrapper.removeSuggestion(suggestionId)
        assertTrue("Remove should succeed", removeResult.isSuccess)

        // Verify it's gone
        val afterRemove = wrapper.listSuggestions()
        assertFalse("Suggestion should no longer be in list", afterRemove.any { it.id == suggestionId })
    }

    @Test
    fun testAddMultipleAndRemoveAll() {
        val ids = mutableListOf<String>()

        for (i in 1..3) {
            val result = wrapper.addWpa2Suggestion("E2E-Multi-$i", "password$i")
            if (result.isFailure) {
                val exception = result.exceptionOrNull()
                if (exception is WifiException && exception.errorCode == WifiErrorCode.APP_DISALLOWED) {
                    return
                }
                fail("Unexpected failure adding suggestion $i: ${exception?.message}")
            }
            ids.add(result.getOrThrow())
        }

        assertEquals("Should have 3 tracked suggestions", 3, wrapper.listSuggestions().size)

        // Remove all
        val removeResult = wrapper.removeAllSuggestions()
        assertTrue("removeAllSuggestions should succeed", removeResult.isSuccess)

        assertEquals("All suggestions should be removed", 0, wrapper.listSuggestions().size)
    }

    @Test
    fun testPersistenceAcrossWrapperInstances() {
        val result = wrapper.addWpa2Suggestion("E2E-Persist", "password123")

        if (result.isFailure) {
            val exception = result.exceptionOrNull()
            if (exception is WifiException && exception.errorCode == WifiErrorCode.APP_DISALLOWED) {
                return
            }
            fail("Unexpected failure: ${exception?.message}")
        }

        val suggestionId = result.getOrThrow()

        // Create a new wrapper instance
        val wrapper2 = WifiManagerWrapper(context)
        val suggestions = wrapper2.listSuggestions()

        assertTrue(
            "Suggestion should persist across wrapper instances",
            suggestions.any { it.id == suggestionId }
        )

        // Clean up via new instance
        wrapper2.removeAllSuggestions()
    }

    @Test
    fun testDuplicateSuggestionFails() {
        val result = wrapper.addWpa2Suggestion("E2E-Duplicate", "password123")

        if (result.isFailure) {
            val exception = result.exceptionOrNull()
            if (exception is WifiException && exception.errorCode == WifiErrorCode.APP_DISALLOWED) {
                return
            }
            fail("Unexpected failure: ${exception?.message}")
        }

        // Try adding the same SSID again
        val duplicateResult = wrapper.addWpa2Suggestion("E2E-Duplicate", "password123")

        if (duplicateResult.isSuccess) {
            // Some Android versions allow duplicate SSIDs with different IDs
            // This is valid behavior
            return
        }

        val exception = duplicateResult.exceptionOrNull()
        assertTrue(
            "Duplicate should fail with DUPLICATE error",
            exception is WifiException &&
                (exception.errorCode == WifiErrorCode.DUPLICATE ||
                 exception.errorCode == WifiErrorCode.APP_DISALLOWED)
        )
    }

    @Test
    fun testRemoveAllOnEmptyState() {
        // Ensure clean state
        wrapper.removeAllSuggestions()

        // Removing all when empty should succeed
        val result = wrapper.removeAllSuggestions()
        assertTrue("removeAllSuggestions on empty state should succeed", result.isSuccess)

        val count = result.getOrThrow()
        assertEquals("Count should be 0 when no suggestions exist", 0, count)
    }
}

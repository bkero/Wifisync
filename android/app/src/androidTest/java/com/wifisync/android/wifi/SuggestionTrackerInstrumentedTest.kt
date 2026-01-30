package com.wifisync.android.wifi

import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for SuggestionTracker.
 * These tests run on an actual Android device or emulator.
 */
@RunWith(AndroidJUnit4::class)
class SuggestionTrackerInstrumentedTest {

    private lateinit var context: Context
    private lateinit var tracker: SuggestionTracker

    @Before
    fun setUp() {
        context = InstrumentationRegistry.getInstrumentation().targetContext
        tracker = SuggestionTracker(context)
        // Clear any existing data before each test
        tracker.clearAll()
    }

    @After
    fun tearDown() {
        // Clean up after each test
        tracker.clearAll()
    }

    @Test
    fun testTrackSuggestion() {
        val record = SuggestionRecord(
            id = "test-123",
            ssid = "TestNetwork",
            securityType = "WPA2",
            isHidden = false,
            installedAt = System.currentTimeMillis()
        )

        tracker.trackSuggestion(record)

        val retrieved = tracker.getSuggestion("test-123")
        assertNotNull(retrieved)
        assertEquals("TestNetwork", retrieved?.ssid)
        assertEquals("WPA2", retrieved?.securityType)
    }

    @Test
    fun testGetSuggestionBySsid() {
        val record = SuggestionRecord(
            id = "ssid-lookup-test",
            ssid = "UniqueSSID",
            securityType = "WPA3",
            isHidden = true,
            installedAt = System.currentTimeMillis()
        )

        tracker.trackSuggestion(record)

        val retrieved = tracker.getSuggestionBySsid("UniqueSSID")
        assertNotNull(retrieved)
        assertEquals("ssid-lookup-test", retrieved?.id)
        assertTrue(retrieved?.isHidden == true)
    }

    @Test
    fun testGetSuggestionNotFound() {
        val retrieved = tracker.getSuggestion("nonexistent-id")
        assertNull(retrieved)
    }

    @Test
    fun testRemoveSuggestion() {
        val record = SuggestionRecord(
            id = "to-remove",
            ssid = "RemoveMe",
            securityType = "WPA2",
            isHidden = false,
            installedAt = System.currentTimeMillis()
        )

        tracker.trackSuggestion(record)
        assertNotNull(tracker.getSuggestion("to-remove"))

        val removed = tracker.removeSuggestion("to-remove")
        assertTrue(removed)
        assertNull(tracker.getSuggestion("to-remove"))
    }

    @Test
    fun testRemoveNonexistentSuggestion() {
        val removed = tracker.removeSuggestion("does-not-exist")
        assertFalse(removed)
    }

    @Test
    fun testGetAllSuggestions() {
        val records = listOf(
            SuggestionRecord("id-1", "Network1", "WPA2", false, 1000L),
            SuggestionRecord("id-2", "Network2", "WPA3", true, 2000L),
            SuggestionRecord("id-3", "Network3", "WPA2", false, 3000L)
        )

        records.forEach { tracker.trackSuggestion(it) }

        val all = tracker.getAllSuggestions()
        assertEquals(3, all.size)
    }

    @Test
    fun testGetAllSuggestionsEmpty() {
        val all = tracker.getAllSuggestions()
        assertTrue(all.isEmpty())
    }

    @Test
    fun testClearAll() {
        val records = listOf(
            SuggestionRecord("id-1", "Network1", "WPA2", false, 1000L),
            SuggestionRecord("id-2", "Network2", "WPA3", true, 2000L)
        )

        records.forEach { tracker.trackSuggestion(it) }
        assertEquals(2, tracker.getAllSuggestions().size)

        tracker.clearAll()
        assertTrue(tracker.getAllSuggestions().isEmpty())
    }

    @Test
    fun testMarkAsRemovedByUser() {
        val record = SuggestionRecord(
            id = "user-removed",
            ssid = "UserRemovedNetwork",
            securityType = "WPA2",
            isHidden = false,
            installedAt = System.currentTimeMillis()
        )

        tracker.trackSuggestion(record)
        assertNotNull(tracker.getSuggestion("user-removed"))

        tracker.markAsRemovedByUser("user-removed")
        assertNull(tracker.getSuggestion("user-removed"))
    }

    @Test
    fun testPersistenceAcrossInstances() {
        val record = SuggestionRecord(
            id = "persistent",
            ssid = "PersistentNetwork",
            securityType = "WPA2",
            isHidden = false,
            installedAt = System.currentTimeMillis()
        )

        tracker.trackSuggestion(record)

        // Create a new tracker instance (simulating app restart)
        val newTracker = SuggestionTracker(context)

        val retrieved = newTracker.getSuggestion("persistent")
        assertNotNull(retrieved)
        assertEquals("PersistentNetwork", retrieved?.ssid)
    }

    @Test
    fun testMultipleSuggestionsWithSameNetwork() {
        // This tests that we can distinguish between different suggestion IDs
        // for potentially the same network (e.g., after removal and re-addition)
        val record1 = SuggestionRecord(
            id = "first-attempt",
            ssid = "SharedSSID",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 1000L
        )

        val record2 = SuggestionRecord(
            id = "second-attempt",
            ssid = "SharedSSID",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 2000L
        )

        tracker.trackSuggestion(record1)
        tracker.trackSuggestion(record2)

        assertEquals(2, tracker.getAllSuggestions().size)

        // getSuggestionBySsid returns first match
        val bySSid = tracker.getSuggestionBySsid("SharedSSID")
        assertNotNull(bySSid)

        // But we can get each by ID
        assertNotNull(tracker.getSuggestion("first-attempt"))
        assertNotNull(tracker.getSuggestion("second-attempt"))
    }
}

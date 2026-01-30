package com.wifisync.android.wifi

import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/**
 * Unit tests for SuggestionRecord data class.
 *
 * Note: SuggestionTracker requires Android Context and is tested
 * in instrumented tests. These tests cover the data model.
 */
@RunWith(JUnit4::class)
class SuggestionRecordTest {

    @Test
    fun `test SuggestionRecord creation`() {
        val record = SuggestionRecord(
            id = "test-id-123",
            ssid = "TestNetwork",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 1706548800000L
        )

        assertEquals("test-id-123", record.id)
        assertEquals("TestNetwork", record.ssid)
        assertEquals("WPA2", record.securityType)
        assertFalse(record.isHidden)
        assertEquals(1706548800000L, record.installedAt)
    }

    @Test
    fun `test SuggestionRecord WPA3`() {
        val record = SuggestionRecord(
            id = "wpa3-network",
            ssid = "SecureNetwork",
            securityType = "WPA3",
            isHidden = true,
            installedAt = System.currentTimeMillis()
        )

        assertEquals("WPA3", record.securityType)
        assertTrue(record.isHidden)
    }

    @Test
    fun `test SuggestionRecord equality`() {
        val record1 = SuggestionRecord(
            id = "same-id",
            ssid = "Network",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 1000L
        )

        val record2 = SuggestionRecord(
            id = "same-id",
            ssid = "Network",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 1000L
        )

        assertEquals(record1, record2)
        assertEquals(record1.hashCode(), record2.hashCode())
    }

    @Test
    fun `test SuggestionRecord copy`() {
        val original = SuggestionRecord(
            id = "original",
            ssid = "Network",
            securityType = "WPA2",
            isHidden = false,
            installedAt = 1000L
        )

        val copy = original.copy(ssid = "UpdatedNetwork")

        assertEquals("original", copy.id)
        assertEquals("UpdatedNetwork", copy.ssid)
        assertEquals("WPA2", copy.securityType)
    }
}

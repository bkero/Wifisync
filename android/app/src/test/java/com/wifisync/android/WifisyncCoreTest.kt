package com.wifisync.android

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/**
 * Unit tests for WifisyncCore data classes.
 *
 * Note: The actual native methods require the Rust library and are tested
 * in instrumented tests. These tests cover the Kotlin data models.
 */
@RunWith(JUnit4::class)
class WifisyncCoreTest {

    // =========================================================================
    // CredentialSummary Tests
    // =========================================================================

    @Test
    fun `test CredentialSummary creation`() {
        val credential = CredentialSummary(
            id = "cred-123",
            ssid = "HomeNetwork",
            securityType = "WPA2-PSK",
            hidden = false,
            managed = true,
            tags = listOf("home", "trusted")
        )

        assertEquals("cred-123", credential.id)
        assertEquals("HomeNetwork", credential.ssid)
        assertEquals("WPA2-PSK", credential.securityType)
        assertFalse(credential.hidden)
        assertTrue(credential.managed)
        assertEquals(2, credential.tags.size)
        assertTrue(credential.tags.contains("home"))
        assertTrue(credential.tags.contains("trusted"))
    }

    @Test
    fun `test CredentialSummary with empty tags`() {
        val credential = CredentialSummary(
            id = "cred-456",
            ssid = "WorkNetwork",
            securityType = "WPA3-SAE",
            hidden = true,
            managed = false,
            tags = emptyList()
        )

        assertTrue(credential.tags.isEmpty())
        assertTrue(credential.hidden)
        assertFalse(credential.managed)
    }

    @Test
    fun `test CredentialSummary equality`() {
        val cred1 = CredentialSummary("id", "ssid", "WPA2-PSK", false, true, listOf("tag"))
        val cred2 = CredentialSummary("id", "ssid", "WPA2-PSK", false, true, listOf("tag"))

        assertEquals(cred1, cred2)
    }

    // =========================================================================
    // CredentialDetail Tests
    // =========================================================================

    @Test
    fun `test CredentialDetail with password`() {
        val detail = CredentialDetail(
            id = "detail-123",
            ssid = "SecureNetwork",
            securityType = "WPA2-PSK",
            password = "secret123",
            hidden = false,
            managed = true,
            tags = listOf("work")
        )

        assertEquals("detail-123", detail.id)
        assertEquals("SecureNetwork", detail.ssid)
        assertEquals("secret123", detail.password)
        assertFalse(detail.hidden)
        assertTrue(detail.managed)
    }

    @Test
    fun `test CredentialDetail without password`() {
        val detail = CredentialDetail(
            id = "detail-456",
            ssid = "OpenNetwork",
            securityType = "Open",
            password = null,
            hidden = false,
            managed = false,
            tags = emptyList()
        )

        assertNull(detail.password)
        assertFalse(detail.managed)
    }

    @Test
    fun `test CredentialDetail hidden network`() {
        val detail = CredentialDetail(
            id = "hidden-net",
            ssid = "HiddenSSID",
            securityType = "WPA2-PSK",
            password = "hiddenpass",
            hidden = true,
            managed = true,
            tags = listOf("hidden")
        )

        assertTrue(detail.hidden)
    }

    // =========================================================================
    // CollectionSummary Tests
    // =========================================================================

    @Test
    fun `test CollectionSummary creation`() {
        val collection = CollectionSummary(
            id = "coll-123",
            name = "Work Networks",
            credentialCount = 5,
            isShared = false
        )

        assertEquals("coll-123", collection.id)
        assertEquals("Work Networks", collection.name)
        assertEquals(5, collection.credentialCount)
        assertFalse(collection.isShared)
    }

    @Test
    fun `test CollectionSummary shared`() {
        val collection = CollectionSummary(
            id = "shared-coll",
            name = "Family Networks",
            credentialCount = 3,
            isShared = true
        )

        assertTrue(collection.isShared)
    }

    @Test
    fun `test CollectionSummary empty`() {
        val collection = CollectionSummary(
            id = "empty-coll",
            name = "New Collection",
            credentialCount = 0,
            isShared = false
        )

        assertEquals(0, collection.credentialCount)
    }

    // =========================================================================
    // ImportSummary Tests
    // =========================================================================

    @Test
    fun `test ImportSummary creation`() {
        val summary = ImportSummary(
            name = "imported-collection",
            count = 10
        )

        assertEquals("imported-collection", summary.name)
        assertEquals(10, summary.count)
    }

    @Test
    fun `test ImportSummary empty import`() {
        val summary = ImportSummary(
            name = "empty-import",
            count = 0
        )

        assertEquals(0, summary.count)
    }

    // =========================================================================
    // ExportSummary Tests
    // =========================================================================

    @Test
    fun `test ExportSummary encrypted`() {
        val summary = ExportSummary(
            name = "my-collection",
            count = 15,
            path = "/storage/emulated/0/Documents/wifisync.enc",
            encrypted = true
        )

        assertEquals("my-collection", summary.name)
        assertEquals(15, summary.count)
        assertTrue(summary.encrypted)
        assertTrue(summary.path.endsWith(".enc"))
    }

    @Test
    fun `test ExportSummary unencrypted`() {
        val summary = ExportSummary(
            name = "public-networks",
            count = 8,
            path = "/storage/emulated/0/Documents/wifisync.json",
            encrypted = false
        )

        assertFalse(summary.encrypted)
        assertTrue(summary.path.endsWith(".json"))
    }

    // =========================================================================
    // ApiResponse Tests
    // =========================================================================

    @Test
    fun `test ApiResponse success with data`() {
        val response = ApiResponse(
            success = true,
            data = "test data",
            error = null,
            message = null
        )

        assertTrue(response.success)
        assertEquals("test data", response.data)
        assertNull(response.error)
    }

    @Test
    fun `test ApiResponse failure with error`() {
        val response = ApiResponse<String>(
            success = false,
            data = null,
            error = "Something went wrong",
            message = null
        )

        assertFalse(response.success)
        assertNull(response.data)
        assertEquals("Something went wrong", response.error)
    }

    @Test
    fun `test ApiResponse success with message`() {
        val response = ApiResponse<Unit>(
            success = true,
            data = null,
            error = null,
            message = "Operation completed"
        )

        assertTrue(response.success)
        assertEquals("Operation completed", response.message)
    }

    // =========================================================================
    // WifisyncException Tests
    // =========================================================================

    @Test
    fun `test WifisyncException creation`() {
        val exception = WifisyncException("Test error message")

        assertEquals("Test error message", exception.message)
    }

    @Test
    fun `test WifisyncException is throwable`() {
        val exception = WifisyncException("Throwable error")

        try {
            throw exception
        } catch (e: WifisyncException) {
            assertEquals("Throwable error", e.message)
        }
    }

    @Test
    fun `test WifisyncException extends Exception`() {
        val exception = WifisyncException("Test")

        assertTrue(exception is Exception)
    }
}

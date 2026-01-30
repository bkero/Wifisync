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
            tags = listOf("home", "trusted")
        )

        assertEquals("cred-123", credential.id)
        assertEquals("HomeNetwork", credential.ssid)
        assertEquals("WPA2-PSK", credential.securityType)
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
            tags = emptyList()
        )

        assertTrue(credential.tags.isEmpty())
    }

    @Test
    fun `test CredentialSummary equality`() {
        val cred1 = CredentialSummary("id", "ssid", "WPA2-PSK", listOf("tag"))
        val cred2 = CredentialSummary("id", "ssid", "WPA2-PSK", listOf("tag"))

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
            source = "NetworkManager",
            tags = listOf("work"),
            createdAt = "2024-01-29T12:00:00Z",
            modifiedAt = "2024-01-29T12:00:00Z"
        )

        assertEquals("detail-123", detail.id)
        assertEquals("SecureNetwork", detail.ssid)
        assertEquals("secret123", detail.password)
        assertFalse(detail.hidden)
        assertEquals("NetworkManager", detail.source)
    }

    @Test
    fun `test CredentialDetail without password`() {
        val detail = CredentialDetail(
            id = "detail-456",
            ssid = "OpenNetwork",
            securityType = "Open",
            password = null,
            hidden = false,
            source = "Manual",
            tags = emptyList(),
            createdAt = "2024-01-29T12:00:00Z",
            modifiedAt = null
        )

        assertNull(detail.password)
        assertNull(detail.modifiedAt)
    }

    @Test
    fun `test CredentialDetail hidden network`() {
        val detail = CredentialDetail(
            id = "hidden-net",
            ssid = "HiddenSSID",
            securityType = "WPA2-PSK",
            password = "hiddenpass",
            hidden = true,
            source = "Android",
            tags = listOf("hidden"),
            createdAt = "2024-01-29T12:00:00Z",
            modifiedAt = null
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
            isShared = false,
            createdAt = "2024-01-29T12:00:00Z"
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
            isShared = true,
            createdAt = "2024-01-29T12:00:00Z"
        )

        assertTrue(collection.isShared)
    }

    @Test
    fun `test CollectionSummary empty`() {
        val collection = CollectionSummary(
            id = "empty-coll",
            name = "New Collection",
            credentialCount = 0,
            isShared = false,
            createdAt = "2024-01-29T12:00:00Z"
        )

        assertEquals(0, collection.credentialCount)
    }

    // =========================================================================
    // ImportSummary Tests
    // =========================================================================

    @Test
    fun `test ImportSummary success`() {
        val summary = ImportSummary(
            imported = 10,
            skipped = 2,
            errors = emptyList()
        )

        assertEquals(10, summary.imported)
        assertEquals(2, summary.skipped)
        assertTrue(summary.errors.isEmpty())
    }

    @Test
    fun `test ImportSummary with errors`() {
        val summary = ImportSummary(
            imported = 5,
            skipped = 0,
            errors = listOf("Invalid security type", "Missing SSID")
        )

        assertEquals(2, summary.errors.size)
        assertTrue(summary.errors.contains("Invalid security type"))
    }

    // =========================================================================
    // ExportSummary Tests
    // =========================================================================

    @Test
    fun `test ExportSummary encrypted`() {
        val summary = ExportSummary(
            exported = 15,
            path = "/storage/emulated/0/Documents/wifisync.enc",
            encrypted = true
        )

        assertEquals(15, summary.exported)
        assertTrue(summary.encrypted)
        assertTrue(summary.path.endsWith(".enc"))
    }

    @Test
    fun `test ExportSummary unencrypted`() {
        val summary = ExportSummary(
            exported = 8,
            path = "/storage/emulated/0/Documents/wifisync.json",
            encrypted = false
        )

        assertFalse(summary.encrypted)
        assertTrue(summary.path.endsWith(".json"))
    }
}

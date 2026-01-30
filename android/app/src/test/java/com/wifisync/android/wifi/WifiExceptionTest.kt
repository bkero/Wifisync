package com.wifisync.android.wifi

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/**
 * Unit tests for WifiException and WifiErrorCode.
 */
@RunWith(JUnit4::class)
class WifiExceptionTest {

    @Test
    fun `test WifiException with PERMISSION_DENIED`() {
        val exception = WifiException("WiFi permissions not granted", WifiErrorCode.PERMISSION_DENIED)

        assertEquals("WiFi permissions not granted", exception.message)
        assertEquals(WifiErrorCode.PERMISSION_DENIED, exception.errorCode)
    }

    @Test
    fun `test WifiException with DUPLICATE`() {
        val exception = WifiException("Network already suggested", WifiErrorCode.DUPLICATE)

        assertEquals("Network already suggested", exception.message)
        assertEquals(WifiErrorCode.DUPLICATE, exception.errorCode)
    }

    @Test
    fun `test WifiException with EXCEEDS_LIMIT`() {
        val exception = WifiException("Maximum suggestions reached", WifiErrorCode.EXCEEDS_LIMIT)

        assertEquals(WifiErrorCode.EXCEEDS_LIMIT, exception.errorCode)
    }

    @Test
    fun `test WifiException with APP_DISALLOWED`() {
        val exception = WifiException("App is not allowed", WifiErrorCode.APP_DISALLOWED)

        assertEquals(WifiErrorCode.APP_DISALLOWED, exception.errorCode)
    }

    @Test
    fun `test WifiException with INTERNAL_ERROR`() {
        val exception = WifiException("Internal WiFi error", WifiErrorCode.INTERNAL_ERROR)

        assertEquals(WifiErrorCode.INTERNAL_ERROR, exception.errorCode)
    }

    @Test
    fun `test WifiException with NOT_FOUND`() {
        val exception = WifiException("Suggestion not found", WifiErrorCode.NOT_FOUND)

        assertEquals(WifiErrorCode.NOT_FOUND, exception.errorCode)
    }

    @Test
    fun `test WifiException with UNKNOWN`() {
        val exception = WifiException("Unknown error: 99", WifiErrorCode.UNKNOWN)

        assertEquals(WifiErrorCode.UNKNOWN, exception.errorCode)
    }

    @Test
    fun `test WifiErrorCode values`() {
        val codes = WifiErrorCode.values()

        assertEquals(7, codes.size)
        assertTrue(codes.contains(WifiErrorCode.PERMISSION_DENIED))
        assertTrue(codes.contains(WifiErrorCode.DUPLICATE))
        assertTrue(codes.contains(WifiErrorCode.EXCEEDS_LIMIT))
        assertTrue(codes.contains(WifiErrorCode.APP_DISALLOWED))
        assertTrue(codes.contains(WifiErrorCode.INTERNAL_ERROR))
        assertTrue(codes.contains(WifiErrorCode.NOT_FOUND))
        assertTrue(codes.contains(WifiErrorCode.UNKNOWN))
    }

    @Test
    fun `test WifiException is throwable`() {
        val exception = WifiException("Test error", WifiErrorCode.INTERNAL_ERROR)

        try {
            throw exception
        } catch (e: WifiException) {
            assertEquals("Test error", e.message)
            assertEquals(WifiErrorCode.INTERNAL_ERROR, e.errorCode)
        }
    }
}

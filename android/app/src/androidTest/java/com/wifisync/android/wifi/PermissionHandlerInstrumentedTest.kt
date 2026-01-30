package com.wifisync.android.wifi

import android.Manifest
import android.app.Activity
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import com.wifisync.android.MainActivity
import org.junit.Assert.*
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for PermissionHandler.
 */
@RunWith(AndroidJUnit4::class)
class PermissionHandlerInstrumentedTest {

    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @get:Rule
    val permissionRule: GrantPermissionRule = GrantPermissionRule.grant(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    private lateinit var permissionHandler: PermissionHandler

    @Before
    fun setUp() {
        activityRule.scenario.onActivity { activity ->
            permissionHandler = PermissionHandler(activity)
        }
    }

    // =========================================================================
    // Permission Arrays Tests
    // =========================================================================

    @Test
    fun testWifiPermissionsArray() {
        activityRule.scenario.onActivity {
            assertEquals(2, permissionHandler.wifiPermissions.size)
            assertTrue(permissionHandler.wifiPermissions.contains(Manifest.permission.ACCESS_WIFI_STATE))
            assertTrue(permissionHandler.wifiPermissions.contains(Manifest.permission.CHANGE_WIFI_STATE))
        }
    }

    @Test
    fun testLocationPermissionsArray() {
        activityRule.scenario.onActivity {
            assertEquals(1, permissionHandler.locationPermissions.size)
            assertTrue(permissionHandler.locationPermissions.contains(Manifest.permission.ACCESS_FINE_LOCATION))
        }
    }

    @Test
    fun testAllPermissionsArray() {
        activityRule.scenario.onActivity {
            assertEquals(3, permissionHandler.allPermissions.size)
            assertTrue(permissionHandler.allPermissions.contains(Manifest.permission.ACCESS_WIFI_STATE))
            assertTrue(permissionHandler.allPermissions.contains(Manifest.permission.CHANGE_WIFI_STATE))
            assertTrue(permissionHandler.allPermissions.contains(Manifest.permission.ACCESS_FINE_LOCATION))
        }
    }

    // =========================================================================
    // Permission Check Tests
    // =========================================================================

    @Test
    fun testHasWifiPermissions() {
        activityRule.scenario.onActivity {
            // With GrantPermissionRule, this should be true
            assertTrue(permissionHandler.hasWifiPermissions())
        }
    }

    @Test
    fun testHasLocationPermission() {
        activityRule.scenario.onActivity {
            // With GrantPermissionRule, this should be true
            assertTrue(permissionHandler.hasLocationPermission())
        }
    }

    @Test
    fun testHasAllPermissions() {
        activityRule.scenario.onActivity {
            // With GrantPermissionRule, this should be true
            assertTrue(permissionHandler.hasAllPermissions())
        }
    }

    // =========================================================================
    // Missing Permissions Tests
    // =========================================================================

    @Test
    fun testGetMissingPermissionsWhenAllGranted() {
        activityRule.scenario.onActivity {
            val missing = permissionHandler.getMissingPermissions(permissionHandler.allPermissions)
            assertTrue(missing.isEmpty())
        }
    }

    // =========================================================================
    // Permission Description Tests
    // =========================================================================

    @Test
    fun testGetPermissionDescriptionWifiState() {
        activityRule.scenario.onActivity {
            val description = permissionHandler.getPermissionDescription(
                Manifest.permission.ACCESS_WIFI_STATE
            )
            assertEquals("Read WiFi network information", description)
        }
    }

    @Test
    fun testGetPermissionDescriptionChangeWifiState() {
        activityRule.scenario.onActivity {
            val description = permissionHandler.getPermissionDescription(
                Manifest.permission.CHANGE_WIFI_STATE
            )
            assertEquals("Add and remove WiFi network suggestions", description)
        }
    }

    @Test
    fun testGetPermissionDescriptionFineLocation() {
        activityRule.scenario.onActivity {
            val description = permissionHandler.getPermissionDescription(
                Manifest.permission.ACCESS_FINE_LOCATION
            )
            assertTrue(description.contains("WiFi scanning"))
        }
    }

    @Test
    fun testGetPermissionDescriptionCoarseLocation() {
        activityRule.scenario.onActivity {
            val description = permissionHandler.getPermissionDescription(
                Manifest.permission.ACCESS_COARSE_LOCATION
            )
            assertTrue(description.contains("location"))
        }
    }

    @Test
    fun testGetPermissionDescriptionUnknown() {
        activityRule.scenario.onActivity {
            val unknownPermission = "android.permission.UNKNOWN_PERMISSION"
            val description = permissionHandler.getPermissionDescription(unknownPermission)
            // Unknown permissions return the permission string itself
            assertEquals(unknownPermission, description)
        }
    }

    // =========================================================================
    // Rationale Tests
    // =========================================================================

    @Test
    fun testShouldShowRationaleWhenGranted() {
        activityRule.scenario.onActivity {
            // When permission is already granted, shouldShowRationale returns false
            val shouldShow = permissionHandler.shouldShowRationale(
                Manifest.permission.ACCESS_WIFI_STATE
            )
            assertFalse(shouldShow)
        }
    }

    // =========================================================================
    // PermissionResult Tests
    // =========================================================================

    @Test
    fun testPermissionResultGranted() {
        val result: PermissionResult = PermissionResult.Granted
        assertTrue(result is PermissionResult.Granted)
    }

    @Test
    fun testPermissionResultDenied() {
        val result = PermissionResult.Denied(
            listOf(Manifest.permission.ACCESS_FINE_LOCATION)
        )
        assertTrue(result is PermissionResult.Denied)
        assertEquals(1, result.deniedPermissions.size)
        assertTrue(result.deniedPermissions.contains(Manifest.permission.ACCESS_FINE_LOCATION))
    }

    @Test
    fun testPermissionResultPermanentlyDenied() {
        val result = PermissionResult.PermanentlyDenied(
            Manifest.permission.ACCESS_FINE_LOCATION
        )
        assertTrue(result is PermissionResult.PermanentlyDenied)
        assertEquals(Manifest.permission.ACCESS_FINE_LOCATION, result.permission)
    }

    @Test
    fun testPermissionResultSealed() {
        // Verify all sealed class variants can be matched
        fun handleResult(result: PermissionResult): String {
            return when (result) {
                is PermissionResult.Granted -> "granted"
                is PermissionResult.Denied -> "denied"
                is PermissionResult.PermanentlyDenied -> "permanently_denied"
            }
        }

        assertEquals("granted", handleResult(PermissionResult.Granted))
        assertEquals("denied", handleResult(PermissionResult.Denied(emptyList())))
        assertEquals("permanently_denied", handleResult(PermissionResult.PermanentlyDenied("test")))
    }
}

package com.wifisync.android.wifi

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.result.ActivityResultLauncher
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

/**
 * Handles runtime permission requests for WiFi operations.
 *
 * Android requires the following permissions for WiFi operations:
 * - ACCESS_WIFI_STATE: Read WiFi state
 * - CHANGE_WIFI_STATE: Modify WiFi settings (add suggestions)
 * - ACCESS_FINE_LOCATION: Required for WiFi scanning (Android 10+)
 */
class PermissionHandler(private val activity: Activity) {

    /**
     * Required permissions for basic WiFi operations.
     */
    val wifiPermissions = arrayOf(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE
    )

    /**
     * Required permissions for WiFi scanning.
     */
    val locationPermissions = arrayOf(
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    /**
     * All permissions needed for full functionality.
     */
    val allPermissions = wifiPermissions + locationPermissions

    /**
     * Check if all WiFi permissions are granted.
     */
    fun hasWifiPermissions(): Boolean {
        return wifiPermissions.all { permission ->
            ContextCompat.checkSelfPermission(activity, permission) == PackageManager.PERMISSION_GRANTED
        }
    }

    /**
     * Check if location permission is granted.
     */
    fun hasLocationPermission(): Boolean {
        return locationPermissions.all { permission ->
            ContextCompat.checkSelfPermission(activity, permission) == PackageManager.PERMISSION_GRANTED
        }
    }

    /**
     * Check if all permissions are granted.
     */
    fun hasAllPermissions(): Boolean {
        return hasWifiPermissions() && hasLocationPermission()
    }

    /**
     * Check if we should show rationale for a permission.
     */
    fun shouldShowRationale(permission: String): Boolean {
        return ActivityCompat.shouldShowRequestPermissionRationale(activity, permission)
    }

    /**
     * Check if permission was permanently denied ("Don't ask again").
     *
     * This is determined by:
     * - Permission not granted
     * - shouldShowRequestPermissionRationale returns false
     * - The permission was previously requested
     */
    fun isPermanentlyDenied(permission: String): Boolean {
        val notGranted = ContextCompat.checkSelfPermission(activity, permission) != PackageManager.PERMISSION_GRANTED
        val noRationale = !ActivityCompat.shouldShowRequestPermissionRationale(activity, permission)
        val wasRequested = getPermissionRequestedFlag(permission)

        return notGranted && noRationale && wasRequested
    }

    /**
     * Mark that we've requested a permission (used for permanent denial detection).
     */
    fun markPermissionRequested(permission: String) {
        setPermissionRequestedFlag(permission, true)
    }

    /**
     * Open the app settings screen where user can manually enable permissions.
     */
    fun openAppSettings() {
        val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
            data = Uri.fromParts("package", activity.packageName, null)
        }
        activity.startActivity(intent)
    }

    /**
     * Get the list of permissions that need to be requested.
     */
    fun getMissingPermissions(permissions: Array<String>): Array<String> {
        return permissions.filter { permission ->
            ContextCompat.checkSelfPermission(activity, permission) != PackageManager.PERMISSION_GRANTED
        }.toTypedArray()
    }

    /**
     * Get a user-friendly description for a permission.
     */
    fun getPermissionDescription(permission: String): String {
        return when (permission) {
            Manifest.permission.ACCESS_WIFI_STATE ->
                "Read WiFi network information"
            Manifest.permission.CHANGE_WIFI_STATE ->
                "Add and remove WiFi network suggestions"
            Manifest.permission.ACCESS_FINE_LOCATION ->
                "Scan for nearby WiFi networks (required by Android for WiFi scanning)"
            Manifest.permission.ACCESS_COARSE_LOCATION ->
                "Approximate location for WiFi scanning"
            else -> permission
        }
    }

    private fun getPermissionRequestedFlag(permission: String): Boolean {
        val prefs = activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        return prefs.getBoolean("requested_$permission", false)
    }

    private fun setPermissionRequestedFlag(permission: String, value: Boolean) {
        val prefs = activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.edit().putBoolean("requested_$permission", value).apply()
    }

    companion object {
        private const val PREFS_NAME = "wifisync_permissions"
    }
}

/**
 * Result of a permission request.
 */
sealed class PermissionResult {
    /** All requested permissions were granted. */
    object Granted : PermissionResult()

    /** Some or all permissions were denied but can be requested again. */
    data class Denied(val deniedPermissions: List<String>) : PermissionResult()

    /** Permission was permanently denied ("Don't ask again" selected). */
    data class PermanentlyDenied(val permission: String) : PermissionResult()
}

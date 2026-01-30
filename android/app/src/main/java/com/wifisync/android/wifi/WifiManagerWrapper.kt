package com.wifisync.android.wifi

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSuggestion
import android.os.Build
import androidx.core.content.ContextCompat
import java.util.UUID

/**
 * Wrapper for Android WifiManager that manages WifiNetworkSuggestion lifecycle.
 *
 * This class provides a high-level interface for:
 * - Creating and managing network suggestions
 * - Tracking installed suggestions
 * - Handling permission requirements
 */
class WifiManagerWrapper(private val context: Context) {

    private val wifiManager: WifiManager =
        context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager

    private val suggestionTracker = SuggestionTracker(context)

    /**
     * Check if WiFi is enabled.
     */
    val isWifiEnabled: Boolean
        get() = wifiManager.isWifiEnabled

    /**
     * Get the current API level.
     */
    val apiLevel: Int
        get() = Build.VERSION.SDK_INT

    /**
     * Check if we have the required WiFi permissions.
     */
    fun hasWifiPermissions(): Boolean {
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.ACCESS_WIFI_STATE
        ) == PackageManager.PERMISSION_GRANTED &&
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.CHANGE_WIFI_STATE
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Check if we have location permission (required for WiFi scanning).
     */
    fun hasLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Add a WPA2 network suggestion.
     *
     * @param ssid The network SSID.
     * @param password The network password.
     * @param isHidden Whether the network is hidden.
     * @return Result with the suggestion ID or an error.
     */
    fun addWpa2Suggestion(
        ssid: String,
        password: String,
        isHidden: Boolean = false
    ): Result<String> {
        return addSuggestion(
            ssid = ssid,
            password = password,
            isHidden = isHidden,
            isWpa3 = false
        )
    }

    /**
     * Add a WPA3 network suggestion.
     *
     * @param ssid The network SSID.
     * @param password The network password.
     * @param isHidden Whether the network is hidden.
     * @return Result with the suggestion ID or an error.
     */
    fun addWpa3Suggestion(
        ssid: String,
        password: String,
        isHidden: Boolean = false
    ): Result<String> {
        return addSuggestion(
            ssid = ssid,
            password = password,
            isHidden = isHidden,
            isWpa3 = true
        )
    }

    /**
     * Add a network suggestion.
     */
    private fun addSuggestion(
        ssid: String,
        password: String,
        isHidden: Boolean,
        isWpa3: Boolean
    ): Result<String> {
        if (!hasWifiPermissions()) {
            return Result.failure(
                WifiException("WiFi permissions not granted", WifiErrorCode.PERMISSION_DENIED)
            )
        }

        val suggestionBuilder = WifiNetworkSuggestion.Builder()
            .setSsid(ssid)
            .setIsHiddenSsid(isHidden)

        // Set the appropriate passphrase method based on security type
        if (isWpa3) {
            suggestionBuilder.setWpa3Passphrase(password)
        } else {
            suggestionBuilder.setWpa2Passphrase(password)
        }

        val suggestion = suggestionBuilder.build()
        val suggestions = listOf(suggestion)

        val status = wifiManager.addNetworkSuggestions(suggestions)

        return when (status) {
            WifiManager.STATUS_NETWORK_SUGGESTIONS_SUCCESS -> {
                val suggestionId = UUID.randomUUID().toString()
                suggestionTracker.trackSuggestion(
                    SuggestionRecord(
                        id = suggestionId,
                        ssid = ssid,
                        securityType = if (isWpa3) "WPA3" else "WPA2",
                        isHidden = isHidden,
                        installedAt = System.currentTimeMillis()
                    )
                )
                Result.success(suggestionId)
            }
            WifiManager.STATUS_NETWORK_SUGGESTIONS_ERROR_ADD_DUPLICATE -> {
                Result.failure(
                    WifiException("Network already suggested", WifiErrorCode.DUPLICATE)
                )
            }
            WifiManager.STATUS_NETWORK_SUGGESTIONS_ERROR_ADD_EXCEEDS_MAX_PER_APP -> {
                Result.failure(
                    WifiException(
                        "Maximum number of suggestions reached (limit: ~50)",
                        WifiErrorCode.EXCEEDS_LIMIT
                    )
                )
            }
            WifiManager.STATUS_NETWORK_SUGGESTIONS_ERROR_APP_DISALLOWED -> {
                Result.failure(
                    WifiException(
                        "App is not allowed to add suggestions. User may have disabled this.",
                        WifiErrorCode.APP_DISALLOWED
                    )
                )
            }
            WifiManager.STATUS_NETWORK_SUGGESTIONS_ERROR_INTERNAL -> {
                Result.failure(
                    WifiException("Internal WiFi error", WifiErrorCode.INTERNAL_ERROR)
                )
            }
            else -> {
                Result.failure(
                    WifiException("Unknown error: $status", WifiErrorCode.UNKNOWN)
                )
            }
        }
    }

    /**
     * Remove a network suggestion by ID.
     *
     * @param suggestionId The suggestion ID returned from add*Suggestion().
     * @return Result indicating success or an error.
     */
    fun removeSuggestion(suggestionId: String): Result<Unit> {
        val record = suggestionTracker.getSuggestion(suggestionId)
            ?: return Result.failure(
                WifiException("Suggestion not found", WifiErrorCode.NOT_FOUND)
            )

        // Build a matching suggestion to remove
        val suggestion = WifiNetworkSuggestion.Builder()
            .setSsid(record.ssid)
            .setIsHiddenSsid(record.isHidden)
            .apply {
                // We need to set a passphrase to build the suggestion, but it doesn't
                // need to match for removal - only SSID matters
                if (record.securityType == "WPA3") {
                    setWpa3Passphrase("placeholder")
                } else {
                    setWpa2Passphrase("placeholder")
                }
            }
            .build()

        val status = wifiManager.removeNetworkSuggestions(listOf(suggestion))

        return when (status) {
            WifiManager.STATUS_NETWORK_SUGGESTIONS_SUCCESS -> {
                suggestionTracker.removeSuggestion(suggestionId)
                Result.success(Unit)
            }
            else -> {
                // Even if removal fails, remove from tracking
                suggestionTracker.removeSuggestion(suggestionId)
                Result.success(Unit) // Treat as success (idempotent)
            }
        }
    }

    /**
     * Remove all network suggestions.
     *
     * @return Result with the number of removed suggestions or an error.
     */
    fun removeAllSuggestions(): Result<Int> {
        val trackedSuggestions = suggestionTracker.getAllSuggestions()

        // Build suggestions to remove
        val suggestions = trackedSuggestions.map { record ->
            WifiNetworkSuggestion.Builder()
                .setSsid(record.ssid)
                .setIsHiddenSsid(record.isHidden)
                .apply {
                    if (record.securityType == "WPA3") {
                        setWpa3Passphrase("placeholder")
                    } else {
                        setWpa2Passphrase("placeholder")
                    }
                }
                .build()
        }

        if (suggestions.isNotEmpty()) {
            wifiManager.removeNetworkSuggestions(suggestions)
        }

        val count = trackedSuggestions.size
        suggestionTracker.clearAll()

        return Result.success(count)
    }

    /**
     * List all tracked suggestions.
     *
     * @return List of suggestion records.
     */
    fun listSuggestions(): List<SuggestionRecord> {
        return suggestionTracker.getAllSuggestions()
    }

    /**
     * Get the number of installed suggestions.
     */
    val suggestionCount: Int
        get() = suggestionTracker.getAllSuggestions().size

    /**
     * Get the maximum allowed suggestions (approximate).
     */
    val suggestionLimit: Int
        get() = 50 // Android's default limit
}

/**
 * Record of an installed network suggestion.
 */
data class SuggestionRecord(
    val id: String,
    val ssid: String,
    val securityType: String,
    val isHidden: Boolean,
    val installedAt: Long
)

/**
 * Error codes for WiFi operations.
 */
enum class WifiErrorCode {
    PERMISSION_DENIED,
    DUPLICATE,
    EXCEEDS_LIMIT,
    APP_DISALLOWED,
    INTERNAL_ERROR,
    NOT_FOUND,
    UNKNOWN
}

/**
 * Exception for WiFi operations.
 */
class WifiException(
    message: String,
    val errorCode: WifiErrorCode
) : Exception(message)

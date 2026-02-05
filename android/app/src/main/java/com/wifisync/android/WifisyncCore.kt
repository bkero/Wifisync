package com.wifisync.android

import com.google.gson.Gson
import com.google.gson.reflect.TypeToken

/**
 * Kotlin wrapper for the Wifisync Rust core library.
 *
 * All methods return JSON strings that are parsed into Kotlin data classes.
 * This provides a clean interface while keeping the JNI boundary simple.
 */
object WifisyncCore {

    private val gson = Gson()

    // ========================================================================
    // Native method declarations (implemented in Rust via JNI)
    // ========================================================================

    @JvmStatic
    private external fun nativeInit(filesDir: String): Boolean

    @JvmStatic
    private external fun nativeListCredentials(): String

    @JvmStatic
    private external fun nativeImportCredentials(filePath: String, password: String?): String

    @JvmStatic
    private external fun nativeExportCredentials(
        collectionName: String,
        filePath: String,
        password: String?
    ): String

    @JvmStatic
    private external fun nativeGetCredential(ssid: String, includePassword: Boolean): String

    @JvmStatic
    private external fun nativeCreateCollection(name: String): String

    @JvmStatic
    private external fun nativeListCollections(): String

    // Sync-related native methods
    @JvmStatic
    private external fun nativeSyncLogin(serverUrl: String, username: String, password: String): String

    @JvmStatic
    private external fun nativeSyncLogout(): String

    @JvmStatic
    private external fun nativeSyncStatus(): String

    @JvmStatic
    private external fun nativeSyncPush(password: String): String

    @JvmStatic
    private external fun nativeSyncPull(password: String): String

    @JvmStatic
    private external fun nativeListDevices(): String

    @JvmStatic
    private external fun nativeGetVersion(): String

    // ========================================================================
    // Public Kotlin API
    // ========================================================================

    /**
     * Initialize the Wifisync core library.
     *
     * @param filesDir The app's internal files directory path.
     * @return true if initialization succeeded.
     */
    fun init(filesDir: String): Boolean {
        return nativeInit(filesDir)
    }

    /**
     * List all credentials across all collections.
     *
     * @return Result containing list of credentials or an error.
     */
    fun listCredentials(): Result<List<CredentialSummary>> {
        return parseResponse(nativeListCredentials())
    }

    /**
     * Import credentials from a file.
     *
     * @param filePath Path to the file to import.
     * @param password Optional password for encrypted files.
     * @return Result containing import summary or an error.
     */
    fun importCredentials(filePath: String, password: String? = null): Result<ImportSummary> {
        return parseResponse(nativeImportCredentials(filePath, password))
    }

    /**
     * Export a collection to a file.
     *
     * @param collectionName Name of the collection to export.
     * @param filePath Path to write the export file.
     * @param password Optional password for encryption.
     * @return Result containing export summary or an error.
     */
    fun exportCredentials(
        collectionName: String,
        filePath: String,
        password: String? = null
    ): Result<ExportSummary> {
        return parseResponse(nativeExportCredentials(collectionName, filePath, password))
    }

    /**
     * Get details of a specific credential by SSID.
     *
     * @param ssid The network SSID to look up.
     * @param includePassword Whether to include the password in the response.
     * @return Result containing credential details or an error.
     */
    fun getCredential(ssid: String, includePassword: Boolean = false): Result<CredentialDetail> {
        return parseResponse(nativeGetCredential(ssid, includePassword))
    }

    /**
     * Create a new collection.
     *
     * @param name Name for the new collection.
     * @return Result indicating success or an error.
     */
    fun createCollection(name: String): Result<Unit> {
        val response = nativeCreateCollection(name)
        val apiResponse: ApiResponse<Any> = gson.fromJson(response, object : TypeToken<ApiResponse<Any>>() {}.type)
        return if (apiResponse.success) {
            Result.success(Unit)
        } else {
            Result.failure(WifisyncException(apiResponse.error ?: "Unknown error"))
        }
    }

    /**
     * List all collections.
     *
     * @return Result containing list of collections or an error.
     */
    fun listCollections(): Result<List<CollectionSummary>> {
        return parseResponse(nativeListCollections())
    }

    // ========================================================================
    // Sync API
    // ========================================================================

    /**
     * Login to a sync server.
     *
     * @param serverUrl The server URL (e.g., "https://sync.example.com")
     * @param username The username
     * @param password The master password
     * @return Result containing login response or an error.
     */
    fun syncLogin(serverUrl: String, username: String, password: String): Result<SyncLoginResponse> {
        return parseResponse(nativeSyncLogin(serverUrl, username, password))
    }

    /**
     * Logout from the sync server.
     *
     * @return Result indicating success or an error.
     */
    fun syncLogout(): Result<Unit> {
        val response = nativeSyncLogout()
        val apiResponse: ApiResponse<Any> = gson.fromJson(response, object : TypeToken<ApiResponse<Any>>() {}.type)
        return if (apiResponse.success) {
            Result.success(Unit)
        } else {
            Result.failure(WifisyncException(apiResponse.error ?: "Unknown error"))
        }
    }

    /**
     * Get the current sync status.
     *
     * @return Result containing sync status or an error.
     */
    fun syncStatus(): Result<SyncStatus> {
        return parseResponse(nativeSyncStatus())
    }

    /**
     * Push local changes to the server.
     *
     * @param password The master password for encryption.
     * @return Result containing push summary or an error.
     */
    fun syncPush(password: String): Result<SyncPushResponse> {
        return parseResponse(nativeSyncPush(password))
    }

    /**
     * Pull remote changes from the server.
     *
     * @param password The master password for decryption.
     * @return Result containing pull summary or an error.
     */
    fun syncPull(password: String): Result<SyncPullResponse> {
        return parseResponse(nativeSyncPull(password))
    }

    /**
     * List devices logged in to the current account.
     *
     * @return Result containing list of devices or an error.
     */
    fun listDevices(): Result<List<DeviceInfo>> {
        return parseResponse(nativeListDevices())
    }

    /**
     * Get the app version.
     *
     * @return Result containing version info or an error.
     */
    fun getVersion(): Result<VersionInfo> {
        return parseResponse(nativeGetVersion())
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    private inline fun <reified T> parseResponse(json: String): Result<T> {
        return try {
            val type = object : TypeToken<ApiResponse<T>>() {}.type
            val response: ApiResponse<T> = gson.fromJson(json, type)

            if (response.success && response.data != null) {
                Result.success(response.data)
            } else {
                Result.failure(WifisyncException(response.error ?: "Unknown error"))
            }
        } catch (e: Exception) {
            Result.failure(WifisyncException("Failed to parse response: ${e.message}"))
        }
    }
}

// ============================================================================
// Data classes matching the Rust API response types
// ============================================================================

/**
 * Generic API response wrapper.
 */
data class ApiResponse<T>(
    val success: Boolean,
    val data: T? = null,
    val error: String? = null,
    val message: String? = null
)

/**
 * Credential summary (without password).
 */
data class CredentialSummary(
    val id: String,
    val ssid: String,
    val securityType: String,
    val hidden: Boolean,
    val managed: Boolean,
    val tags: List<String>
)

/**
 * Credential detail (optionally with password).
 */
data class CredentialDetail(
    val id: String,
    val ssid: String,
    val securityType: String,
    val password: String? = null,
    val hidden: Boolean,
    val managed: Boolean,
    val tags: List<String>
)

/**
 * Collection summary.
 */
data class CollectionSummary(
    val id: String,
    val name: String,
    val credentialCount: Int,
    val isShared: Boolean
)

/**
 * Import operation summary.
 */
data class ImportSummary(
    val name: String,
    val count: Int
)

/**
 * Export operation summary.
 */
data class ExportSummary(
    val name: String,
    val count: Int,
    val path: String,
    val encrypted: Boolean
)

// ============================================================================
// Sync-related data classes
// ============================================================================

/**
 * Sync login response.
 */
data class SyncLoginResponse(
    val serverUrl: String,
    val username: String,
    val deviceId: String
)

/**
 * Current sync status.
 */
data class SyncStatus(
    val enabled: Boolean,
    val serverUrl: String? = null,
    val username: String? = null,
    val deviceId: String? = null,
    val lastSync: String? = null,
    val pendingChanges: Int = 0,
    val hasValidToken: Boolean = false
)

/**
 * Sync push response.
 */
data class SyncPushResponse(
    val accepted: Int,
    val conflicts: Int
)

/**
 * Sync pull response.
 */
data class SyncPullResponse(
    val applied: Int,
    val errors: Int
)

/**
 * Device information.
 */
data class DeviceInfo(
    val id: String,
    val name: String,
    val lastSyncAt: String? = null,
    val createdAt: String,
    val isCurrentDevice: Boolean = false
)

/**
 * Version information.
 */
data class VersionInfo(
    val version: String,
    val rustVersion: String? = null,
    val buildDate: String? = null
)

/**
 * Exception for Wifisync errors.
 */
class WifisyncException(message: String) : Exception(message)

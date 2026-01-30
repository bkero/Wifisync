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

/**
 * Exception for Wifisync errors.
 */
class WifisyncException(message: String) : Exception(message)

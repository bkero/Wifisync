package com.wifisync.android.wifi

import android.content.Context
import android.content.SharedPreferences
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken

/**
 * Tracks installed WiFi network suggestions in local storage.
 *
 * Android's WifiManager doesn't provide a way to list installed suggestions,
 * so we maintain our own tracking to know what we've installed.
 */
class SuggestionTracker(context: Context) {

    private val prefs: SharedPreferences = context.getSharedPreferences(
        PREFS_NAME,
        Context.MODE_PRIVATE
    )

    private val gson = Gson()

    /**
     * Track a newly installed suggestion.
     */
    fun trackSuggestion(record: SuggestionRecord) {
        val suggestions = getAllSuggestions().toMutableList()
        suggestions.add(record)
        saveSuggestions(suggestions)
    }

    /**
     * Get a specific suggestion by ID.
     */
    fun getSuggestion(id: String): SuggestionRecord? {
        return getAllSuggestions().find { it.id == id }
    }

    /**
     * Get a suggestion by SSID.
     */
    fun getSuggestionBySsid(ssid: String): SuggestionRecord? {
        return getAllSuggestions().find { it.ssid == ssid }
    }

    /**
     * Remove a suggestion from tracking.
     */
    fun removeSuggestion(id: String): Boolean {
        val suggestions = getAllSuggestions().toMutableList()
        val removed = suggestions.removeAll { it.id == id }
        if (removed) {
            saveSuggestions(suggestions)
        }
        return removed
    }

    /**
     * Get all tracked suggestions.
     */
    fun getAllSuggestions(): List<SuggestionRecord> {
        val json = prefs.getString(KEY_SUGGESTIONS, null) ?: return emptyList()
        return try {
            val type = object : TypeToken<List<SuggestionRecord>>() {}.type
            gson.fromJson(json, type)
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * Clear all tracked suggestions.
     */
    fun clearAll() {
        prefs.edit().remove(KEY_SUGGESTIONS).apply()
    }

    /**
     * Mark a suggestion as removed by the user (detected during sync).
     */
    fun markAsRemovedByUser(id: String) {
        // For now, just remove from tracking
        // In the future, we could keep a "removed by user" flag
        removeSuggestion(id)
    }

    private fun saveSuggestions(suggestions: List<SuggestionRecord>) {
        val json = gson.toJson(suggestions)
        prefs.edit().putString(KEY_SUGGESTIONS, json).apply()
    }

    companion object {
        private const val PREFS_NAME = "wifisync_suggestions"
        private const val KEY_SUGGESTIONS = "suggestions"
    }
}

package com.wifisync.android

import android.app.Application
import android.util.Log

/**
 * Application class for Wifisync.
 *
 * Initializes the Rust core library on startup.
 */
class WifisyncApplication : Application() {

    companion object {
        private const val TAG = "WifisyncApp"

        init {
            // Load the native Rust library
            try {
                System.loadLibrary("wifisync_jni")
                Log.i(TAG, "Loaded wifisync_jni native library")
            } catch (e: UnsatisfiedLinkError) {
                Log.e(TAG, "Failed to load native library: ${e.message}")
            }
        }
    }

    override fun onCreate() {
        super.onCreate()

        // Initialize the Rust core with our files directory
        val filesDir = filesDir.absolutePath
        Log.i(TAG, "Initializing Wifisync core with filesDir: $filesDir")

        val initialized = WifisyncCore.init(filesDir)
        if (initialized) {
            Log.i(TAG, "Wifisync core initialized successfully")
        } else {
            Log.e(TAG, "Failed to initialize Wifisync core")
        }
    }
}

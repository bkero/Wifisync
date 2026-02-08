package com.wifisync.android

import androidx.test.platform.app.InstrumentationRegistry

/**
 * Shared configuration for live sync integration tests.
 * Reads server credentials from instrumentation arguments passed via build.gradle.kts.
 *
 * Tests using this config should call Assume.assumeTrue(LiveSyncTestConfig.isConfigured)
 * in @Before to skip gracefully when credentials are not provided.
 */
object LiveSyncTestConfig {

    val serverUrl: String by lazy {
        InstrumentationRegistry.getArguments().getString("syncServerUrl", "") ?: ""
    }

    val username: String by lazy {
        InstrumentationRegistry.getArguments().getString("syncUsername", "") ?: ""
    }

    val password: String by lazy {
        InstrumentationRegistry.getArguments().getString("syncPassword", "") ?: ""
    }

    val isConfigured: Boolean
        get() = serverUrl.isNotBlank() && username.isNotBlank() && password.isNotBlank()

    fun getSkipMessage(): String =
        "Live sync tests require WIFISYNC_SERVER_URL, WIFISYNC_USERNAME, and WIFISYNC_PASSWORD environment variables"
}

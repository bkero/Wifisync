# Wifisync ProGuard Rules

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep the WifisyncCore class and all its methods
-keep class com.wifisync.android.WifisyncCore { *; }

# Keep all data classes used for JSON serialization
-keep class com.wifisync.android.ApiResponse { *; }
-keep class com.wifisync.android.CredentialSummary { *; }
-keep class com.wifisync.android.CredentialDetail { *; }
-keep class com.wifisync.android.CollectionSummary { *; }
-keep class com.wifisync.android.ImportSummary { *; }
-keep class com.wifisync.android.ExportSummary { *; }

# Gson uses reflection
-keepattributes Signature
-keepattributes *Annotation*
-dontwarn sun.misc.**
-keep class com.google.gson.** { *; }

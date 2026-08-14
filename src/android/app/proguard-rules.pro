# Keep MSAL entry points and the browser tab activity intact. MSAL ships
# consumer rules in v5+, so this is mostly belt-and-braces.
-keep class com.microsoft.identity.client.** { *; }
-keep class com.microsoft.identity.common.** { *; }

# The Mikepenz multiplatform-markdown-renderer uses Compose's composable
# machinery and some reflection. Keep the public surface; let R8 strip the
# rest of the unused Compose runtime.
-keep class com.mikepenz.markdown.** { *; }
-keep class org.jetbrains.markdown.** { *; }

# OkHttp + okio warnings R8 sometimes surfaces.
-dontwarn org.bouncycastle.**
-dontwarn org.conscrypt.**
-dontwarn org.openjsse.**

# Preserve Compose @Composable function metadata for tooling/preview.
-keepclassmembers class * {
    @androidx.compose.runtime.Composable <methods>;
}

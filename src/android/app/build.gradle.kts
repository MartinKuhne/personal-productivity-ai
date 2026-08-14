plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    // Kotlin 2.x bundles the Compose compiler; the version is now driven by the
    // Kotlin plugin (2.1.0 here) instead of `composeOptions.kotlinCompilerExtensionVersion`.
    id("org.jetbrains.kotlin.plugin.compose")
    // @Parcelize on FastMDUiState — we use it for SavedStateHandle persistence.
    id("org.jetbrains.kotlin.plugin.parcelize")
    // Static analysis; `./gradlew detekt` is part of the quality gate.
    id("io.gitlab.arturbosch.detekt")
}

android {
    namespace = "com.fastmd.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.fastmd.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "1.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        manifestPlaceholders["appAuthRedirectScheme"] = "msauth"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    // No `composeOptions` block: with Kotlin 2.x + the Compose plugin the
    // compiler version is derived from the Kotlin plugin version.
    testOptions {
        unitTests {
            isIncludeAndroidResources = true
            isReturnDefaultValues = true
        }
    }
    packaging {
        resources {
            excludes += setOf(
                "/META-INF/{AL2.0,LGPL2.1}",
                "META-INF/LICENSE*",
                "META-INF/NOTICE*",
            )
        }
    }
}

dependencies {
    // --- AndroidX core / lifecycle ----------------------------------------
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-savedstate:2.8.7")
    implementation("androidx.activity:activity-compose:1.9.3")

    // --- Jetpack Compose (BOM-driven versions) ---------------------------
    // Compose BOM 2025.04.01 → Compose 1.8. Bumping the BOM upgrades every
    // androidx.compose.* artifact transitively.
    val composeBom = platform("androidx.compose:compose-bom:2025.04.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.runtime:runtime")
    debugImplementation("androidx.compose.ui:ui-tooling")

    // --- Markdown rendering (mikepenz multiplatform-markdown-renderer) ---
    // Core engine + the M3-themed module. Use
    // `com.mikepenz.markdown.m3.Markdown` from Compose.
    implementation("com.mikepenz:multiplatform-markdown-renderer:0.38.1")
    implementation("com.mikepenz:multiplatform-markdown-renderer-m3:0.38.1")

    // --- Auth (MSAL) ------------------------------------------------------
    implementation("com.microsoft.identity.client:msal:5.0.0") {
        exclude(group = "io.opentelemetry", module = "opentelemetry-bom")
    }

    // --- Networking -------------------------------------------------------
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("org.json:json:20260522")

    // --- Unit tests -------------------------------------------------------
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
    testImplementation("androidx.test:core:1.6.1")
    testImplementation("androidx.test.ext:junit:1.2.1")
    testImplementation("androidx.arch.core:core-testing:2.2.0")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.robolectric:robolectric:4.14")
    testImplementation(platform("androidx.compose:compose-bom:2025.04.01"))
    testImplementation("androidx.compose.ui:ui-test-junit4")
    testImplementation("androidx.compose.ui:ui-test-manifest")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}

// Pin the Kotlin stdlib to the Kotlin plugin version (2.1.0). The
// multiplatform-markdown-renderer pulls stdlib 2.2.x transitively; Gradle
// resolves that fine, but a hard pin keeps the toolchain internally
// consistent and surfaces any future drift at build time.
configurations.all {
    resolutionStrategy {
        force(
            "org.jetbrains.kotlin:kotlin-stdlib:2.1.0",
            "org.jetbrains.kotlin:kotlin-stdlib-common:2.1.0",
        )
    }
}

detekt {
    config.setFrom("$rootDir/config/detekt/detekt.yml")
    buildUponDefaultConfig = true
    autoCorrect = false
    // Detekt's `config` is at the root dir; this subproject uses the
    // same ruleset.
    source.setFrom(files("src/main/java", "src/test/java"))
}

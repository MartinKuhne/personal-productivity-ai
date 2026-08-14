// Top-level build file for the FastMD Android companion app.
//
// Versions are intentionally pinned at the root so submodules stay consistent.
// Modern Android toolchain (Kotlin 2.x + the bundled Compose Compiler plugin).
plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.1.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.0" apply false
    id("org.jetbrains.kotlin.plugin.parcelize") version "2.1.0" apply false
    id("io.gitlab.arturbosch.detekt") version "1.23.7" apply false
}

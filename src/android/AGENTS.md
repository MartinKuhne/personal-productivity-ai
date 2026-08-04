# AI Agent Instructions — `src/android/`

The Android Kotlin/Gradle companion app. Repo-root `AGENTS.md` provides shared
principles; this file pins the Android-specific toolchain and conventions.

## 1. Toolchain
- Build and test from `src/android/` using the Gradle wrapper: `./gradlew assembleDebug`, `./gradlew testDebugUnitTest`, `./gradlew lint`.
- Use `./gradlew ktlintCheck` (or the configured Detekt task) and ensure it passes cleanly.
- Do not commit `local.properties`; SDK paths are machine-specific.

## 2. Conventions
- Kotlin only; target the project's configured `minSdk` / `compileSdk` / JVM target — do not bump them without a design record.
- Follow the existing package layout under `app/src/main/java/com/...`. Keep new code in feature-cohesive packages, not by-layer dumps.
- Prefer AndroidX / Jetpack libraries already on the dependency list over hand-rolled equivalents.

## 3. Quality gate
Before marking a task complete, from `src/android/`:
- `./gradlew testDebugUnitTest` — green
- `./gradlew lint` and `./gradlew ktlintCheck` — no warnings
- `./gradlew assembleDebug` — builds cleanly

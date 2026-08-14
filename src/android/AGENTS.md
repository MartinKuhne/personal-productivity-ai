# AI Agent Instructions — `src/android/`

The Android Kotlin/Gradle companion app. Repo-root `AGENTS.md` provides shared
principles; this file pins the Android-specific toolchain, layout, and
conventions.

## 1. Toolchain

- Build and test from `src/android/` using the Gradle wrapper: `./gradlew assembleDebug`, `./gradlew testDebugUnitTest`, `./gradlew lint`.
- The quality gate also runs `./gradlew detekt`. CI mirrors this in
  `.github/workflows/android-quality-gate.yml`.
- Do not commit `local.properties`; SDK paths are machine-specific.

### Pinned versions (root `build.gradle.kts`)

- Android Gradle Plugin: `8.7.3`
- Kotlin: `2.1.0`
- Kotlin Compose Compiler plugin: `2.1.0` (Kotlin 2.x bundles the Compose compiler — do **not** also set `composeOptions.kotlinCompilerExtensionVersion`).
- Compose BOM: `2025.04.01` (Compose 1.8 — resolved transitively for all `androidx.compose.*` artifacts).
- `com.mikepenz:multiplatform-markdown-renderer[-m3]:0.38.1`. The library is wired to use Material 3 styling (`com.mikepenz.markdown.m3.Markdown`). The `app/build.gradle.kts` pins `kotlin-stdlib` to `2.1.0` to keep the toolchain internally consistent — bump that pin if you bump the Kotlin plugin.
- Detekt: `1.23.7`. Config lives at `src/android/config/detekt/detekt.yml`.
- compileSdk / targetSdk: `35`. minSdk stays at `26` until a design record says otherwise.

### Gradle wrapper

The repository does **not** commit a `gradlew` wrapper. Generate one with
`gradle wrapper --gradle-version 8.10.2 --distribution-type bin` from
`src/android/` and commit the resulting `gradlew`, `gradlew.bat`, and
`gradle/wrapper/` directory. The CI workflow assumes the wrapper is
present.

## 2. Conventions

- Kotlin only; keep `compileSdk` / `minSdk` / `targetSdk` aligned with the values above unless a design record says otherwise.
- Follow the package layout under `app/src/main/java/com/fastmd/android/`:
  - `com.fastmd.android` — `MainActivity`, `AppContent`. Thin Android plumbing.
  - `com.fastmd.android.data` — pure data layer. The public surface is
    [`OneDriveSource`](app/src/main/java/com/fastmd/android/data/OneDriveSource.kt);
    underneath it splits into [`OneDriveAuth`](app/src/main/java/com/fastmd/android/data/OneDriveAuth.kt) (MSAL) and
    [`OneDriveDataSource`](app/src/main/java/com/fastmd/android/data/OneDriveDataSource.kt) (Graph) so each can be
    faked in tests. Concrete impls: `MsalOneDriveAuth`,
    `GraphOneDriveDataSource`, `MsalOneDriveSource`.
  - `com.fastmd.android.ui.viewmodel` — `FastMDViewModel`, `FastMDUiState`. `FastMDUiState` is `@Parcelize` for `SavedStateHandle` persistence.
  - `com.fastmd.android.ui.theme` — `FastMDTheme` and `FastMDColors`.
  - `com.fastmd.android.ui.screen` — one composable per file (`AuthScreen`, `FileBrowserScreen`, `FileTreeView`, `FileViewerPane`, `MarkdownContent`).
- One class / interface / composable per file. Use `stringResource(R.string.…)` for user-facing strings; do not roll a new `Strings` object.
- Prefer AndroidX / Jetpack libraries already on the dependency list over hand-rolled equivalents.
- For markdown rendering, use `com.mikepenz.markdown.m3.Markdown` from `multiplatform-markdown-renderer-m3`. Wrap it in a small `MarkdownContent` composable rather than importing the library directly from screens, so we have one swap point.
- Errors flow as the `OneDriveError` sealed hierarchy from `data/`. The
  view layer maps to display strings via `ui/ErrorMessages.kt`. The
  ViewModel never depends on Android `R.string.*` — that lets the
  ViewModel be unit-tested on the JVM.
- `FileNode` is immutable. File content is fetched via the stable
  `/me/drive/items/{id}/content` endpoint (not the short-lived
  `@microsoft.graph.downloadUrl`).
- A folder load is partial-failure tolerant: per-folder errors are
  collected in `TreeFetch.failedFolders` and surfaced as a snackbar in
  the UI.

## 3. Quality gate

Before marking a task complete, from `src/android/`:

- `./gradlew testDebugUnitTest` — green (4 `FileTreeProcessorTest` cases, 6 `GraphOneDriveDataSourceTest` cases, 8 `FastMDViewModelTest` cases, 3 `AuthScreenTest` cases, 4 `FileBrowserScreenTest` cases).
- `./gradlew lint` — no warnings
- `./gradlew detekt` — no warnings (config at `config/detekt/detekt.yml`)
- `./gradlew assembleDebug` — builds cleanly

CI runs the same gate in `.github/workflows/android-quality-gate.yml` on
every PR touching this directory.

## 4. Authentication setup

The shipped `res/raw/auth_config_single_account.json` has placeholder
`client_id` and `redirect_uri` values. `MsalOneDriveAuth` validates the
config at init time and surfaces a `OneDriveError.Misconfigured` with a
human-readable message if the placeholders are still in place. To wire
up a real Azure AD app registration:

1. Create an app registration in the Azure portal with the **Mobile and
   desktop applications** platform enabled.
2. Replace `client_id` in `auth_config_single_account.json` with the
   app's client id.
3. Replace the `signature_hash_here` segment in `redirect_uri` with
   the SHA-1 of your debug signing key (`./gradlew signingReport`).
4. Update the matching `android:path` in `AndroidManifest.xml`'s
   `BrowserTabActivity` intent filter.

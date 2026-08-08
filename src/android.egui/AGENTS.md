# AI Agent Instructions — `src/android.egui/`

The Rust + `eframe` (egui) sibling to the Kotlin/Compose app in `src/android/`.
It targets the same use case — browse a OneDrive folder tree, filter to markdown,
open one — but ships the UI in pure Rust and the auth flow in pure Rust instead
of relying on the Android-Java MSAL library.

The repo-root `AGENTS.md` provides shared principles; this file pins the
toolchain, layout, and quality gate for this crate.

## 1. Toolchain

- Build/test from `src/android.egui/` using `cargo`. (Not a workspace member;
  the repo has no root `Cargo.toml`. Treat the crate as a standalone project,
  same as `src/desktop/`.)
- Android packaging uses `cargo apk` (from `rust-windowing/cargo-apk`).
  Requires the Android NDK and the `aarch64-linux-android`,
  `armv7-linux-androideabi`, and `x86_64-linux-android` Rust targets.
- Required env: `ANDROID_NDK_HOME` (path to the unpacked NDK; e.g.
  `C:\Users\mkuhn\AppData\Local\Android\Sdk\ndk\27.3.13750724`).

## 2. Conventions

- Rust 2024 edition (matches `src/desktop/`).
- `eframe = "0.35"` is the UI framework. The `android-native-activity` feature
  flag is what wires eframe into `winit`'s Android backend; keep it on.
- One type per file, SRP. The `ui/` module owns egui widgets; the `auth/`,
  `onedrive/`, and `file_node` modules are egui-free and unit-testable.
- The public API is re-exported through `lib.rs`; the rest of the crate is
  private by default.

## 3. Layout

```
src/android.egui/
├── AGENTS.md
├── README.md
├── Cargo.toml
├── assets/                 # bundled at compile time (icons, MSAL config JSON)
├── src/
│   ├── lib.rs              # crate facade — re-exports public API only
│   ├── app.rs              # FastMdApp — owns the eframe App impl
│   ├── auth.rs             # OAuth 2.0 PKCE flow against Microsoft v2.0
│   ├── onedrive.rs         # Graph API client: /me/drive, /me/drive/root:...
│   ├── file_node.rs        # FileNode + FileTreeProcessor (port of FileNode.kt)
│   ├── config.rs           # AuthConfig, AppPaths — bundled config + dirs
│   ├── error.rs            # thiserror AppError
│   └── ui/
│       ├── mod.rs
│       ├── sign_in.rs      # sign-in screen widget
│       ├── file_tree.rs    # collapsible tree view (port of FileTreeView)
│       └── file_viewer.rs  # right-pane viewer
└── tests/
    └── file_tree_processor.rs  # port of AppTest.kt
```

## 4. Quality gate

Before marking any task complete, run from `src/android.egui/`:
- `cargo check --quiet` — no errors or warnings
- `cargo test --quiet` — all tests pass (the `FileTreeProcessor` regression suite)
- `cargo clippy --all-targets -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings
- `cargo apk build --lib` — produces a debuggable APK (smoke test the build
  pipeline; does not require a connected device)

A successful `cargo apk build` is the equivalent of the Kotlin module's
`./gradlew assembleDebug` and is the bar for "this crate builds for Android."

## 5. What this crate is NOT

- It is not a drop-in replacement for the Kotlin/Compose app. It is a parallel
  implementation of the same user-facing behaviour, written in Rust, that
  builds an APK via `cargo apk` instead of Gradle.
- It is not a productised MSAL replacement. The OAuth 2.0 PKCE flow here is
  hand-rolled using `ring` for SHA-256 and CSPRNG (same approach as the
  desktop `fastmd` crate for its MCP OAuth 2.1 flow). The MSAL library's
  SSO/cache/silent-acquisition features are out of scope.
- It does not currently ship a debug-signed keystore. For local `adb install`
  use, `cargo apk run` signs with a debug key automatically; for sideloading
  APKs you will need to sign with your own keystore (see `README.md`).

## 6. Implementation notes

### 6.1 JNI glue (`src/android.rs`)

The two functions in `src/android.rs` drive the OAuth round-trip on
Android:

- `open_in_browser(url)` constructs an `android.content.Intent` with
  `ACTION_VIEW`, wraps the URL via `Uri.parse`, and calls
  `Activity.startActivity(...)`.
- `current_intent_uri()` reads the activity's current `Intent` via
  `Activity.getIntent()`, follows `Intent.getData()`, and returns the
  URI as a Rust `String`.

Both go through the `jni` 0.21 crate (pinned; 0.22's `Env`/`EnvUnowned`
split is heavier than we need). The `JavaVM*` and `Activity*` come
from `ndk_context::android_context()`; the thread is attached via
`JavaVM::attach_current_thread` which gives a `JNIEnv` for the JNI
work. The shared `with_env` helper in `jni_glue` keeps the attach/
detach bookkeeping in one place.

### 6.2 Deep-link polling (`FastMdApp::poll_deep_link`)

Called every frame from `FastMdApp::logic`. Reads the current intent
URI via JNI, diffs against `self.last_deep_link`, and on a new
`msauth://...` URI parses the auth code and kicks off the token
exchange in a `std::thread`. The same state machine drives the
PKCE flow on host (where the redirect is captured by a stub
`webbrowser` browser handoff) and on Android.

### 6.3 Manifest patching (`tools/patch-msauth-intent.ps1`)

`cargo apk` 0.10 only emits the `MAIN`/`LAUNCHER` intent filter, and
its `Cargo.toml` metadata doesn't accept extra filters. The patch
script:
1. Uses `aapt2 link` to compile a text manifest that includes both the
   launcher filter and the `msauth://com.fastmd.android.egui` deep-
   link filter, with the `VIEW`/`BROWSABLE`/`DEFAULT` categories and
   the matching `data` element.
2. Splices the compiled AXML into the original APK (preserving the
   `.so` libraries and resources) using the .NET `ZipFile` API.
3. Re-signs the result with the debug keystore, invoking the
   `apksigner.jar` directly via `java -jar` to bypass the
   `apksigner.bat` wrapper's JVM warning.

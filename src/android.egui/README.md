# `fastmd-android-egui`

Rust + [`eframe`](https://github.com/emilk/egui) (egui) port of the
Kotlin/Compose OneDrive viewer in `src/android/`. Same user-facing
behaviour, same OAuth 2.0 redirect URI, same OneDrive Graph calls — but
the UI is pure Rust and the APK is built by `cargo apk` instead of
Gradle.

This directory is a sibling of `src/android/`, not a replacement. The
Kotlin app stays as the production path; this crate is a research
experiment that demonstrates `eframe 0.35` builds for Android via
`cargo-apk 0.10` and what an idiomatic Rust port of the same UI looks
like.

## Layout

```
src/android.egui/
├── AGENTS.md           # rules + quality gate for this crate
├── README.md           # you are here
├── Cargo.toml          # deps + Android packaging metadata
├── assets/             # MSAL-style auth config JSON (bundled at compile time)
├── src/
│   ├── lib.rs          # facade + android_main entry point
│   ├── app.rs          # eframe::App + state machine + background workers
│   ├── auth.rs         # OAuth 2.0 PKCE against Microsoft v2.0
│   ├── config.rs       # AuthConfig (bundled JSON)
│   ├── error.rs        # AppError (thiserror)
│   ├── file_node.rs    # FileNode + FileTreeProcessor (port of FileNode.kt)
│   ├── onedrive.rs     # OneDriveClient (Microsoft Graph)
│   ├── android.rs      # Android-only glue (see "Known gaps" below)
│   └── ui/             # egui widgets
└── tests/
    └── file_tree_processor.rs   # port of AppTest.kt
```

## Build

The crate is a standalone Rust project; treat it like `src/desktop/`.

### Host build (test the data model + UI on the desktop)

```sh
cd src/android.egui
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

### Android build (produces an APK)

Requires:
- `ANDROID_HOME` pointing at the Android SDK
  (`C:\Users\mkuhn\AppData\Local\Android\Sdk` on this box).
- `ANDROID_NDK_HOME` pointing at the unpacked NDK
  (`C:\Users\mkuhn\AppData\Local\Android\Sdk\ndk\27.3.13750724` on this box).
- Rust targets: `aarch64-linux-android`, `armv7-linux-androideabi`,
  `x86_64-linux-android` (installed via `rustup target add`).
- `cargo-apk` 0.10 (`cargo install --git
  https://github.com/rust-mobile/cargo-apk --locked cargo-apk`).

```sh
cd src/android.egui
cargo apk build --lib
# APK lands at: target/debug/apk/fastmd-android-egui.apk
```

To install on a connected device or running emulator:

```sh
cargo apk run --lib        # builds, installs, and launches in one step
# or:
adb install target/debug/apk/fastmd-android-egui.apk
```

## Configuration

The crate ships with a placeholder Azure app registration under
`assets/auth_config_single_account.json` (client id
`YOUR_CLIENT_ID_HERE`, redirect `msauth://com.fastmd.android.egui/signature_hash_here`).
Replace the file with your real client id + signature hash and rebuild —
the JSON is loaded at compile time via `include_str!`.

## What works

- **Host build (Windows / Linux / macOS).** The data model, the Graph
  client, the PKCE flow, the egui widgets, the unit tests, the clippy
  lints — all green.
- **Android APK build.** `cargo apk build` produces a signed-debug APK
  for `aarch64-linux-android`, `armv7-linux-androideabi`, and
  `x86_64-linux-android`. The NativeActivity launches, the egui
  renderer initialises, the sign-in screen renders.
- **The Four Regression Tests from `AppTest.kt`.** Filter to `.md`
  files, drop empty directories, drop non-md files, sort dirs-before-files.
  All four pass as Rust integration tests in
  `tests/file_tree_processor.rs`.
- **The Android OAuth round-trip end-to-end.** The two JNI helpers in
  `src/android.rs` (`open_in_browser` and `current_intent_uri`) talk
  to the Java `Activity` via the `jni` crate, using the `JavaVM*` and
  `Activity*` from `ndk_context`. The `msauth://` deep-link
  intent filter is added by `tools/patch-msauth-intent.ps1` after
  `cargo apk build` (cargo-apk 0.10 doesn't accept custom intent
  filters via `Cargo.toml`).

## Build & install (Android, end-to-end)

```sh
cd src/android.egui
cargo apk build --lib
pwsh -NoProfile -ExecutionPolicy Bypass -File tools/patch-msauth-intent.ps1
adb install target/debug/apk/fastmd-android-egui-patched.apk
```

The patch script:
1. Uses `aapt2 link` to compile a text manifest with both the
   `MAIN`/`LAUNCHER` and the `msauth://com.fastmd.android.egui` deep-link
   filters.
2. Splices the compiled `AndroidManifest.xml` into the original APK,
   preserving the `.so` libraries and resources.
3. Re-signs the result with the debug keystore (the Java 24+ JVM
   "restricted method" warning is suppressed because we invoke the
   apksigner JAR directly via `java -jar`).

Install with `adb install`, then launch the app from the home screen
or via `adb shell am start -n com.fastmd.android.egui/.MainActivity`. The
sign-in button dispatches an `Intent.ACTION_VIEW` to the system
browser; after completing the Microsoft sign-in, the browser redirects
to `msauth://com.fastmd.android.egui/...`; the JNI deep-link poller
picks up the redirect, trades the auth code for an access token via
the v2.0 token endpoint, and the OneDrive tree loads in the egui UI.

## Known gaps

- **No tests for the JNI glue.** The `open_in_browser` and
  `current_intent_uri` functions are gated to `target_os = "android"`
  and call into the live JavaVM, so they only run on-device. The
  `cargo test` gate only covers the egui-free modules
  (`file_node`, `auth`, `onedrive`).
- **No automated integration of the manifest patch into
  `cargo apk build`.** The patch script has to be run separately.
  Cargo doesn't have a post-build hook for `cargo apk`, and
  cargo-apk's metadata format doesn't accept extra intent filters
  via `Cargo.toml`. (A future improvement: a `xtask` binary that runs
  `cargo apk build` then the patch in one command.)
- **MSAL is not actually used.** The OAuth flow is hand-rolled
  PKCE against the v2.0 endpoint, same as the desktop `fastmd`
  crate's MCP OAuth 2.1 path. SSO, account caching, and silent
  token refresh are not implemented; you have to click "Sign in"
  each time the app starts (and after the access token expires).

## Why this exists

To answer a single question: *can `src/android/` use `egui`?* The answer
is yes, with the caveats spelled out above and in the top-level
conversation. This crate is the working demonstration; the Kotlin app
stays the production path until someone has the time to finish the JNI
glue and decide whether shipping a single Rust UI for desktop + mobile
is worth the cost.

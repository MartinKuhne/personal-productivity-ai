//! Android-only glue. The crate compiles to both a `cdylib` (for `cargo apk`)
//! and an `rlib` (for `cargo test` on the host), so every symbol in this
//! module is gated to `target_os = "android"`. The host build simply omits
//! it.
//!
//! # OAuth round-trip
//!
//! The two functions here back the OAuth 2.0 PKCE flow on Android:
//!
//! - [`open_in_browser`] dispatches an `Intent.ACTION_VIEW` through
//!   `Activity.startActivity`, which hands off to the system browser.
//! - [`current_intent_uri`] reads the activity's current `Intent` and
//!   returns its data URI as a Rust `String`. The caller diffs against the
//!   last seen URI to detect the `msauth://...` redirect that completes
//!   the flow.
//!
//! Both go through the `jni` 0.21 crate; the `JavaVM*` and `Activity*`
//! pointers come from `ndk_context::android_context()`, which is
//! populated by `android-activity 0.6` before `android_main` runs.

#[cfg(target_os = "android")]
use std::sync::OnceLock;

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

/// Called from `android_main` before `eframe::run_native` so the eframe App
/// can poll the activity for new intents (specifically, the `msauth://`
/// redirect that completes the OAuth flow).
#[cfg(target_os = "android")]
pub fn install(app: AndroidApp) {
    let _ = ANDROID_APP.set(app);
}

#[cfg(target_os = "android")]
pub fn android_app() -> Option<&'static AndroidApp> {
    ANDROID_APP.get()
}

// ---------------------------------------------------------------------
// JNI helpers shared by the two entry points below.
// ---------------------------------------------------------------------

#[cfg(target_os = "android")]
mod jni_glue {
    use jni::objects::JObject;
    use jni::{JavaVM, JNIEnv};
    use ndk_context::android_context;

    /// Attach the current thread to the JavaVM and run `f` with a
    /// `&mut JNIEnv`. The `android-activity` glue attaches the thread to
    /// the `JavaVM` before `android_main` runs, so this is effectively
    /// a no-op after the first call.
    pub fn with_env<F, R>(f: F) -> Result<R, String>
    where
        F: FnOnce(&mut JNIEnv) -> jni::errors::Result<R>,
    {
        let ctx = android_context();
        // SAFETY: `ctx.vm()` returns the live `JavaVM*` Java passed into
        // the NativeActivity constructor. We construct exactly one
        // `JavaVM` from this pointer.
        let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut _) }
            .map_err(|e| format!("JavaVM::from_raw: {e}"))?;
        let mut guard = vm
            .attach_current_thread()
            .map_err(|e| format!("attach_current_thread: {e}"))?;
        // `AttachGuard` derefs to `JNIEnv`; the deref gives us a `&mut`
        // through which we can run the JNI work.
        f(&mut guard).map_err(|e| format!("JNI: {e}"))
    }

    /// Wrap the raw `Activity*` pointer as a `JObject<'_>`. The pointer
    /// comes from `ndk_context` and is the same reference the
    /// `NativeActivity` Java class is bound to. The lifetime on the
    /// returned `JObject` is tied to the `JNIEnv` we received from the
    /// attach guard so local-ref accounting still lines up.
    pub fn activity_obj<'local>(env: &JNIEnv<'local>) -> JObject<'local> {
        let ctx = android_context();
        // The lifetime is purely a marker here — jni 0.21 doesn't
        // actively track the local ref inside `from_raw`, but binding
        // it to `&JNIEnv<'local>` keeps callers honest about scoping.
        let _ = env;
        unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) }
    }
}

// ---------------------------------------------------------------------
// Public API used by the App.
// ---------------------------------------------------------------------

/// Launch `url` in the system browser via `Intent.ACTION_VIEW`.
///
/// The current `Activity` is grabbed through `ndk_context`; the URL is
/// wrapped in a `java.net.URI.parse(...)` and bound to a fresh
/// `android.content.Intent` with `ACTION_VIEW`. The intent is started
/// without `FLAG_ACTIVITY_NEW_TASK` because we already have a
/// `NativeActivity` on the stack — adding it would just push a second
/// task onto the back stack for no reason.
#[cfg(target_os = "android")]
pub fn open_in_browser(url: &str) -> Result<(), String> {
    use jni::objects::JValue;

    jni_glue::with_env(|env| -> jni::errors::Result<()> {
        let activity = jni_glue::activity_obj(env);

        // Intent intent = new Intent();
        let intent = env.new_object("android/content/Intent", "()V", &[])?;

        // intent.setAction("android.intent.action.VIEW");
        let action_view = env.new_string("android.intent.action.VIEW")?;
        env.call_method(
            &intent,
            "setAction",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::from(&action_view)],
        )?;

        // Uri uri = Uri.parse(url);
        let url_jstr = env.new_string(url)?;
        let uri_class = env.find_class("android/net/Uri")?;
        let uri = env
            .call_static_method(
                &uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::from(&url_jstr)],
            )?
            .l()?;

        // intent.setData(uri);
        env.call_method(
            &intent,
            "setData",
            "(Landroid/net/Uri;)Landroid/content/Intent;",
            &[JValue::from(&uri)],
        )?;

        // activity.startActivity(intent);
        env.call_method(
            &activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::from(&intent)],
        )?;

        Ok(())
    })
}

/// Read the activity's current `Intent` data URI. Returns `Ok(None)` if
/// there is no intent, no data, or the data is `null`. JNI errors are
/// surfaced as `Err(...)`.
///
/// The caller is responsible for diffing against the last seen URI; this
/// function does no state tracking of its own.
#[cfg(target_os = "android")]
pub fn current_intent_uri() -> Result<Option<String>, String> {
    use jni::objects::JString;

    jni_glue::with_env(|env| -> jni::errors::Result<Option<String>> {
        let activity = jni_glue::activity_obj(env);

        // Intent intent = activity.getIntent();
        let intent = env
            .call_method(&activity, "getIntent", "()Landroid/content/Intent;", &[])?
            .l()?;
        if intent.is_null() {
            return Ok(None);
        }

        // Uri data = intent.getData();
        let data = env
            .call_method(&intent, "getData", "()Landroid/net/Uri;", &[])?
            .l()?;
        if data.is_null() {
            return Ok(None);
        }

        // String s = data.toString();
        let s_jobj = env
            .call_method(&data, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        if s_jobj.is_null() {
            return Ok(None);
        }

        // Convert the Java String to a Rust String. The JString wrapper
        // ensures the local ref is dropped at the end of this scope.
        let jstring: JString = s_jobj.into();
        let s = env.get_string(&jstring)?.into();
        Ok(Some(s))
    })
}

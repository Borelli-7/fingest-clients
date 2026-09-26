//! Android Keystore-backed secret storage.
//!
//! `EncryptedSharedPreferences` does the work: the AES key lives in the Keystore, which on
//! most devices is hardware-backed, so the ciphertext on disk is useless without the
//! device. That is the property the web client cannot have at any price — a browser token
//! is readable by any script on the origin.
//!
//! Reached through JNI because the Keystore has no C ABI; it is Java all the way down. The
//! calls are confined to this file so the rest of the mobile client never sees a `JNIEnv`.

use jni::JavaVM;
use jni::objects::{JObject, JString, JValue};

use crate::{SecretError, SecretStore, committed};

/// One entry, namespaced so it cannot collide with anything else the app stores.
const PREFS_FILE: &str = "fingest_secure";
const KEY: &str = "session";

const CREATE_SIG: &str = "(Landroid/content/Context;Ljava/lang/String;Landroidx/security/crypto/MasterKey;Landroidx/security/crypto/EncryptedSharedPreferences$PrefKeyEncryptionScheme;Landroidx/security/crypto/EncryptedSharedPreferences$PrefValueEncryptionScheme;)Landroid/content/SharedPreferences;";
const EDITOR_SIG: &str = "()Landroid/content/SharedPreferences$Editor;";
const PUT_SIG: &str =
    "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;";

pub struct AndroidKeystore;

impl AndroidKeystore {
    pub fn new() -> Self {
        Self
    }

    /// Every call re-attaches rather than caching a `JNIEnv`: an env is only valid on the
    /// thread that obtained it, and these run from async tasks that may be rescheduled.
    #[allow(unsafe_code)]
    fn with_prefs<T>(
        &self,
        action: impl FnOnce(&mut jni::JNIEnv, &JObject) -> Result<T, jni::errors::Error>,
    ) -> Result<T, SecretError> {
        let context = ndk_context::android_context();

        // The pointers come from `ndk_context`, which the Android entry point populated
        // before any of this ran. JNI exposes no safe way to adopt them.
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }
            .map_err(|e| SecretError::Unavailable(format!("no JVM: {e}")))?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };

        let mut env = vm
            .attach_current_thread()
            .map_err(|e| SecretError::Unavailable(format!("cannot attach thread: {e}")))?;

        let result = (|| -> Result<T, jni::errors::Error> {
            let scheme = env
                .get_static_field(
                    "androidx/security/crypto/MasterKey$KeyScheme",
                    "AES256_GCM",
                    "Landroidx/security/crypto/MasterKey$KeyScheme;",
                )?
                .l()?;
            let builder = env.new_object(
                "androidx/security/crypto/MasterKey$Builder",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&activity)],
            )?;
            let builder = env
                .call_method(
                    &builder,
                    "setKeyScheme",
                    "(Landroidx/security/crypto/MasterKey$KeyScheme;)Landroidx/security/crypto/MasterKey$Builder;",
                    &[JValue::Object(&scheme)],
                )?
                .l()?;
            let master_key = env
                .call_method(
                    &builder,
                    "build",
                    "()Landroidx/security/crypto/MasterKey;",
                    &[],
                )?
                .l()?;

            let file = env.new_string(PREFS_FILE)?;
            let key_scheme = env
                .get_static_field(
                    "androidx/security/crypto/EncryptedSharedPreferences$PrefKeyEncryptionScheme",
                    "AES256_SIV",
                    "Landroidx/security/crypto/EncryptedSharedPreferences$PrefKeyEncryptionScheme;",
                )?
                .l()?;
            let value_scheme = env
                .get_static_field(
                    "androidx/security/crypto/EncryptedSharedPreferences$PrefValueEncryptionScheme",
                    "AES256_GCM",
                    "Landroidx/security/crypto/EncryptedSharedPreferences$PrefValueEncryptionScheme;",
                )?
                .l()?;

            let prefs = env
                .call_static_method(
                    "androidx/security/crypto/EncryptedSharedPreferences",
                    "create",
                    CREATE_SIG,
                    &[
                        JValue::Object(&activity),
                        JValue::Object(&file),
                        JValue::Object(&master_key),
                        JValue::Object(&key_scheme),
                        JValue::Object(&value_scheme),
                    ],
                )?
                .l()?;

            action(&mut env, &prefs)
        })();

        // A Java exception left pending aborts the thread the next time the env is touched
        // or detached — so a missing class would kill the app instead of degrading. Clearing
        // it is what actually makes the fallback below a fallback.
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }

        result.map_err(|e| SecretError::Unavailable(e.to_string()))
    }
}

impl Default for AndroidKeystore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for AndroidKeystore {
    fn read(&self) -> Option<String> {
        let found = self.with_prefs(|env, prefs| {
            let key = env.new_string(KEY)?;
            let value = env
                .call_method(
                    prefs,
                    "getString",
                    "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[JValue::Object(&key), JValue::Object(&JObject::null())],
                )?
                .l()?;

            if value.is_null() {
                return Ok(None);
            }

            let owned: String = env.get_string(&JString::from(value))?.into();
            Ok(Some(owned))
        });

        match found {
            Ok(value) => value,
            Err(error) => {
                // A locked device or an invalidated key reads as "signed out", not a crash.
                tracing::warn!(%error, "secure storage unreadable");
                None
            }
        }
    }

    fn write(&self, value: &str) -> Result<(), SecretError> {
        let value = value.to_owned();

        self.with_prefs(move |env, prefs| {
            let editor = env.call_method(prefs, "edit", EDITOR_SIG, &[])?.l()?;
            let key = env.new_string(KEY)?;
            let value = env.new_string(&value)?;

            let editor = env
                .call_method(
                    &editor,
                    "putString",
                    PUT_SIG,
                    &[JValue::Object(&key), JValue::Object(&value)],
                )?
                .l()?;

            // `commit`, not `apply`: its boolean is the only report of a failed write.
            env.call_method(&editor, "commit", "()Z", &[])?.z()
        })
        .and_then(|ok| committed(ok, "session write"))
    }

    fn clear(&self) {
        let outcome = self
            .with_prefs(|env, prefs| {
                let editor = env.call_method(prefs, "edit", EDITOR_SIG, &[])?.l()?;
                let editor = env.call_method(&editor, "clear", EDITOR_SIG, &[])?.l()?;

                env.call_method(&editor, "commit", "()Z", &[])?.z()
            })
            .and_then(|ok| committed(ok, "session clear"));

        if let Err(error) = outcome {
            // The token may still be on disk after the user signed out.
            tracing::error!(%error, "could not clear secure storage");
        }
    }
}

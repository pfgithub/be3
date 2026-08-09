use jni::{
    errors::Error as JniInnerError,
    jni_sig, jni_str,
    objects::{JObject, JValue},
    sys::jobject,
    vm::JavaVM,
    Env,
};

const APK_MIME_TYPE: &str = "application/vnd.android.package-archive";

/// Writes `apk_bytes` into the system's `MediaStore.Downloads` collection and
/// asks the package installer to open it.
///
/// This goes through `MediaStore` rather than a `FileProvider`: this app has
/// no bundled Java/Kotlin classes for `cargo-apk` to register a provider
/// against, and passing another app a `file://` URI directly is rejected by
/// `StrictMode` on modern Android. `MediaStore.Downloads` is the system's own
/// content provider, so it needs no manifest changes here.
pub(super) fn install(apk_bytes: &[u8], display_name: &str) -> Result<(), String> {
    let context = ndk_context::android_context();
    // SAFETY: android-activity initializes this before `android_main` runs and
    // it stays valid for the life of the process.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    let activity: jobject = context.context().cast();

    vm.attach_current_thread_for_scope(|env| {
        install_with_env(env, activity, apk_bytes, display_name)
    })
    .map_err(|error: JniError| error.0)
}

struct JniError(String);

impl From<JniInnerError> for JniError {
    fn from(error: JniInnerError) -> Self {
        Self(error.to_string())
    }
}

fn install_with_env(
    env: &mut Env<'_>,
    activity: jobject,
    apk_bytes: &[u8],
    display_name: &str,
) -> Result<(), JniError> {
    // SAFETY: `activity` is the process-wide Activity reference android-activity
    // keeps alive for the whole process. It is only read through here, never
    // deleted, so treating it as a borrowed local reference is sound.
    let activity = unsafe { JObject::from_raw(env, activity) };

    let resolver = env
        .call_method(
            &activity,
            jni_str!("getContentResolver"),
            jni_sig!("()Landroid/content/ContentResolver;"),
            &[],
        )?
        .l()?;

    let values = env.new_object(
        jni_str!("android/content/ContentValues"),
        jni_sig!("()V"),
        &[],
    )?;
    put_string(env, &values, "_display_name", display_name)?;
    put_string(env, &values, "mime_type", APK_MIME_TYPE)?;

    let downloads_class = env.find_class(jni_str!("android/provider/MediaStore$Downloads"))?;
    let collection = env
        .get_static_field(
            &downloads_class,
            jni_str!("EXTERNAL_CONTENT_URI"),
            jni_sig!("Landroid/net/Uri;"),
        )?
        .l()?;

    let item = env
        .call_method(
            &resolver,
            jni_str!("insert"),
            jni_sig!("(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;"),
            &[JValue::Object(&collection), JValue::Object(&values)],
        )?
        .l()?;
    if item.is_null() {
        return Err(JniError(
            "the system declined to create a download entry".to_owned(),
        ));
    }

    write_bytes(env, &resolver, &item, apk_bytes)?;

    let intent = build_view_intent(env, &item)?;
    env.call_method(
        &activity,
        jni_str!("startActivity"),
        jni_sig!("(Landroid/content/Intent;)V"),
        &[JValue::Object(&intent)],
    )?;

    Ok(())
}

fn put_string(
    env: &mut Env<'_>,
    values: &JObject<'_>,
    key: &str,
    value: &str,
) -> Result<(), JniError> {
    let key = env.new_string(key)?;
    let value = env.new_string(value)?;
    env.call_method(
        values,
        jni_str!("put"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        &[JValue::Object(key.as_ref()), JValue::Object(value.as_ref())],
    )?;
    Ok(())
}

fn write_bytes(
    env: &mut Env<'_>,
    resolver: &JObject<'_>,
    item: &JObject<'_>,
    bytes: &[u8],
) -> Result<(), JniError> {
    let output_stream = env
        .call_method(
            resolver,
            jni_str!("openOutputStream"),
            jni_sig!("(Landroid/net/Uri;)Ljava/io/OutputStream;"),
            &[JValue::Object(item)],
        )?
        .l()?;

    let byte_array = env.byte_array_from_slice(bytes)?;
    env.call_method(
        &output_stream,
        jni_str!("write"),
        jni_sig!("([B)V"),
        &[JValue::Object(byte_array.as_ref())],
    )?;
    env.call_method(&output_stream, jni_str!("close"), jni_sig!("()V"), &[])?;
    Ok(())
}

fn build_view_intent<'local>(
    env: &mut Env<'local>,
    item: &JObject<'_>,
) -> Result<JObject<'local>, JniError> {
    let action = env.new_string("android.intent.action.VIEW")?;
    let intent = env.new_object(
        jni_str!("android/content/Intent"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(action.as_ref())],
    )?;

    let mime_type = env.new_string(APK_MIME_TYPE)?;
    env.call_method(
        &intent,
        jni_str!("setDataAndType"),
        jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;"),
        &[JValue::Object(item), JValue::Object(mime_type.as_ref())],
    )?;

    let intent_class = env.find_class(jni_str!("android/content/Intent"))?;

    let read_permission_flag = env
        .get_static_field(
            &intent_class,
            jni_str!("FLAG_GRANT_READ_URI_PERMISSION"),
            jni_sig!("I"),
        )?
        .i()?;
    env.call_method(
        &intent,
        jni_str!("addFlags"),
        jni_sig!("(I)Landroid/content/Intent;"),
        &[JValue::Int(read_permission_flag)],
    )?;

    let new_task_flag = env
        .get_static_field(
            &intent_class,
            jni_str!("FLAG_ACTIVITY_NEW_TASK"),
            jni_sig!("I"),
        )?
        .i()?;
    env.call_method(
        &intent,
        jni_str!("addFlags"),
        jni_sig!("(I)Landroid/content/Intent;"),
        &[JValue::Int(new_task_flag)],
    )?;

    Ok(intent)
}

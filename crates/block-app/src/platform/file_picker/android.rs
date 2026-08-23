use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex, OnceLock,
};

use jni::{
    errors::Error as JniError,
    jni_sig, jni_str,
    objects::{JByteArray, JClass, JObject, JString, JValue},
    refs::Reference,
    vm::JavaVM,
    Env, EnvUnowned, Outcome,
};

use super::{FileFilter, PickResult, PickedFile};

static PENDING: OnceLock<Mutex<Option<Sender<PickResult>>>> = OnceLock::new();

fn pending() -> &'static Mutex<Option<Sender<PickResult>>> {
    PENDING.get_or_init(Default::default)
}

pub(super) fn open(filter: &FileFilter) -> Receiver<PickResult> {
    let (sender, receiver) = mpsc::channel();
    let Ok(mut pending) = pending().lock() else {
        let _ = sender.send(Err("The file picker is unavailable".into()));
        return receiver;
    };
    *pending = Some(sender.clone());
    if let Err(error) = start(filter) {
        *pending = None;
        let _ = sender.send(Err(error));
    }
    receiver
}

fn start(filter: &FileFilter) -> Result<(), String> {
    let context = ndk_context::android_context();

    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    let started = vm
        .attach_current_thread_for_scope(|env| {
            let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
            let class = main_activity(env, &activity)?;
            let mime_types = env.new_string(filter.mime_types.join(","))?;
            env.call_static_method(
                &class,
                jni_str!("pickFile"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&mime_types)],
            )?
            .z()
        })
        .map_err(|error: JniError| error.to_string())?;
    if started {
        Ok(())
    } else {
        Err("Choosing a file is unavailable right now".into())
    }
}

fn main_activity<'local>(
    env: &mut Env<'local>,
    activity: &JObject<'local>,
) -> Result<JClass<'local>, JniError> {
    let class_loader = env
        .call_method(
            activity,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let class_name = env.new_string("com.be3.block.MainActivity")?;
    let class = env
        .call_method(
            &class_loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )?
        .l()?;
    env.cast_local::<JClass<'local>>(class)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeFilePicked(
    mut env: EnvUnowned<'_>,
    _: JClass<'_>,
    name: JString<'_>,
    data: JByteArray<'_>,
    error: JString<'_>,
) {
    let result = match env
        .with_env(|env| collect(env, &name, &data, &error))
        .into_outcome()
    {
        Outcome::Ok(result) => result,
        Outcome::Err(_) | Outcome::Panic(_) => Err("The chosen file could not be read".to_owned()),
    };
    let Ok(mut pending) = pending().lock() else {
        return;
    };
    if let Some(sender) = pending.take() {
        let _ = sender.send(result);
    }
}

fn collect(
    env: &mut Env<'_>,
    name: &JString<'_>,
    data: &JByteArray<'_>,
    error: &JString<'_>,
) -> Result<PickResult, JniError> {
    if !error.is_null() {
        return Ok(Err(error.to_string()));
    }
    if data.is_null() {
        return Ok(Ok(None));
    }
    let name = if name.is_null() {
        String::new()
    } else {
        name.to_string()
    };
    let data = env.convert_byte_array(data)?;
    Ok(Ok(Some(PickedFile { name, data })))
}

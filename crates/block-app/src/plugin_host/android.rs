use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use eframe::egui;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
enum State {
    #[default]
    Closed,
    Binding,
    Connected,
    Error(String),
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    _client: std::sync::Arc<BlockClient>,
    _block: BlockHandle<Counter>,
    instance: block_plugin_api::EditorInstanceId,
    region: block_plugin_api::EditorRegion,
    size: egui::Vec2,
) {
    ensure_bound();
    let Ok(state) = STATE.get_or_init(Default::default).lock() else {
        ui.colored_label(egui::Color32::RED, "Counter plugin state is unavailable");
        return;
    };
    match &*state {
        State::Closed | State::Binding => {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
        State::Error(error) => {
            let error = error.clone();
            drop(state);
            ui.vertical_centered(|ui| {
                ui.colored_label(egui::Color32::RED, error);
                if ui.button("Retry").clicked() {
                    close(ui.ctx(), instance);
                    ensure_bound();
                }
            });
        }
        State::Connected => {
            drop(state);
            ui.allocate_ui(size, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("TODO: present the plugin's {region:?} region"));
                });
            });
        }
    }
}

fn ensure_bound() {
    let state = STATE.get_or_init(Default::default);
    let Ok(mut state) = state.lock() else {
        return;
    };
    if !matches!(*state, State::Closed) {
        return;
    }
    *state = State::Binding;
    if let Err(error) = bind() {
        *state = State::Error(error);
    }
}

pub(crate) fn close(_context: &egui::Context, _instance: block_plugin_api::EditorInstanceId) {
    unbind();
    if let Ok(mut state) = STATE.get_or_init(Default::default).lock() {
        *state = State::Closed;
    }
}

fn bind() -> Result<(), String> {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let bound = vm
        .attach_current_thread_for_scope(|environment| {
            let activity =
                unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
            let bridge = plugin_host_bridge(environment, &activity)?;
            environment
                .call_static_method(
                    &bridge,
                    jni::jni_str!("bind"),
                    jni::jni_sig!("(Landroid/content/Context;)Z"),
                    &[jni::objects::JValue::Object(&activity)],
                )
                .and_then(|value| value.z())
        })
        .map_err(|error| error.to_string())?;
    if bound {
        Ok(())
    } else {
        Err("Counter plugin is unavailable; Android API 26 or newer is required".into())
    }
}

fn unbind() {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let _ = vm.attach_current_thread_for_scope(|environment| {
        let activity =
            unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
        let bridge = plugin_host_bridge(environment, &activity)?;
        environment.call_static_method(
            &bridge,
            jni::jni_str!("unbind"),
            jni::jni_sig!("(Landroid/content/Context;)V"),
            &[jni::objects::JValue::Object(&activity)],
        )?;
        Ok::<_, jni::errors::Error>(())
    });
}

fn plugin_host_bridge<'local>(
    environment: &mut jni::Env<'local>,
    activity: &jni::objects::JObject<'local>,
) -> jni::errors::Result<jni::objects::JClass<'local>> {
    let class_loader = environment
        .call_method(
            activity,
            jni::jni_str!("getClassLoader"),
            jni::jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let class_name = environment.new_string("com.be3.block.plugin.PluginHostBridge")?;
    let class = environment
        .call_method(
            &class_loader,
            jni::jni_str!("loadClass"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[jni::objects::JValue::Object(&class_name)],
        )?
        .l()?;
    environment.cast_local::<jni::objects::JClass<'local>>(class)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeConnected(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
    if let Ok(mut state) = STATE.get_or_init(Default::default).lock() {
        *state = State::Connected;
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeDisconnected(
    mut environment: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    reason: jni::objects::JString<'_>,
) {
    let reason = match environment
        .with_env(|_| Ok::<_, jni::errors::Error>(reason.to_string()))
        .into_outcome()
    {
        jni::Outcome::Ok(reason) => reason,
        jni::Outcome::Err(_) | jni::Outcome::Panic(_) => "Counter plugin disconnected".into(),
    };
    if let Ok(mut state) = STATE.get_or_init(Default::default).lock() {
        *state = State::Error(reason);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativePacket(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    _: jni::objects::JByteArray<'_>,
) {
}

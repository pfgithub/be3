use block_client::BlockClient;
use block_plugin_api::{EditorInstanceId, EditorRegion, PluginManifest};
use eframe::egui;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use uuid::Uuid;

#[derive(Default)]
enum State {
    #[default]
    Closed,
    Binding,
    Connected,
    Error(String),
}

static RUNTIMES: OnceLock<Mutex<HashMap<String, State>>> = OnceLock::new();

fn runtimes() -> &'static Mutex<HashMap<String, State>> {
    RUNTIMES.get_or_init(Default::default)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    plugin: &PluginManifest,
    _client: std::sync::Arc<BlockClient>,
    _block_id: Uuid,
    _block_type: Uuid,
    instance: EditorInstanceId,
    region: EditorRegion,
    size: egui::Vec2,
) {
    let plugin_id = plugin.identity.id.clone();
    let service = plugin.entry_points.android.clone().unwrap_or_default();
    ensure_bound(&plugin_id, &service);
    let Ok(runtimes) = runtimes().lock() else {
        ui.colored_label(egui::Color32::RED, "Plugin state is unavailable");
        return;
    };
    match runtimes.get(&plugin_id).unwrap_or(&State::Closed) {
        State::Closed | State::Binding => {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
        State::Error(error) => {
            let error = error.clone();
            drop(runtimes);
            ui.vertical_centered(|ui| {
                ui.colored_label(egui::Color32::RED, error);
                if ui.button("Retry").clicked() {
                    close(ui.ctx(), &plugin_id, instance);
                    ensure_bound(&plugin_id, &service);
                }
            });
        }
        State::Connected => {
            drop(runtimes);
            ui.allocate_ui(size, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("TODO: present the plugin's {region:?} region"));
                });
            });
        }
    }
}

pub(crate) fn region_size(
    _plugin_id: &str,
    _instance: EditorInstanceId,
    _region: EditorRegion,
) -> Option<egui::Vec2> {
    None
}

fn ensure_bound(plugin_id: &str, service: &str) {
    {
        let Ok(mut bound) = runtimes().lock() else {
            return;
        };
        let state = bound.entry(plugin_id.to_owned()).or_default();
        if !matches!(*state, State::Closed) {
            return;
        }
        *state = State::Binding;
    }
    if let Err(error) = bind(plugin_id, service) {
        if let Ok(mut bound) = runtimes().lock() {
            bound.insert(plugin_id.to_owned(), State::Error(error));
        }
    }
}

pub(crate) fn close(_context: &egui::Context, plugin_id: &str, _instance: EditorInstanceId) {
    unbind(plugin_id);
    if let Ok(mut runtimes) = runtimes().lock() {
        runtimes.insert(plugin_id.to_owned(), State::Closed);
    }
}

fn bind(plugin_id: &str, service: &str) -> Result<(), String> {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let bound = vm
        .attach_current_thread_for_scope(|environment| {
            let activity =
                unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
            let bridge = plugin_host_bridge(environment, &activity)?;
            let key = environment.new_string(plugin_id)?;
            let service = environment.new_string(service)?;
            environment
                .call_static_method(
                    &bridge,
                    jni::jni_str!("bind"),
                    jni::jni_sig!(
                        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Z"
                    ),
                    &[
                        jni::objects::JValue::Object(&activity),
                        jni::objects::JValue::Object(&key),
                        jni::objects::JValue::Object(&service),
                    ],
                )
                .and_then(|value| value.z())
        })
        .map_err(|error| error.to_string())?;
    if bound {
        Ok(())
    } else {
        Err("This plugin is unavailable; Android API 26 or newer is required".into())
    }
}

fn unbind(plugin_id: &str) {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let _ = vm.attach_current_thread_for_scope(|environment| {
        let activity =
            unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
        let bridge = plugin_host_bridge(environment, &activity)?;
        let key = environment.new_string(plugin_id)?;
        environment.call_static_method(
            &bridge,
            jni::jni_str!("unbind"),
            jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;)V"),
            &[
                jni::objects::JValue::Object(&activity),
                jni::objects::JValue::Object(&key),
            ],
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

fn read_string(environment: &mut jni::EnvUnowned<'_>, value: &jni::objects::JString<'_>) -> String {
    match environment
        .with_env(|_| Ok::<_, jni::errors::Error>(value.to_string()))
        .into_outcome()
    {
        jni::Outcome::Ok(value) => value,
        jni::Outcome::Err(_) | jni::Outcome::Panic(_) => String::new(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeConnected(
    mut environment: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    plugin_id: jni::objects::JString<'_>,
) {
    let plugin_id = read_string(&mut environment, &plugin_id);
    if let Ok(mut runtimes) = runtimes().lock() {
        runtimes.insert(plugin_id, State::Connected);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeDisconnected(
    mut environment: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    plugin_id: jni::objects::JString<'_>,
    reason: jni::objects::JString<'_>,
) {
    let plugin_id = read_string(&mut environment, &plugin_id);
    let mut reason = read_string(&mut environment, &reason);
    if reason.is_empty() {
        reason = "The plugin disconnected".to_owned();
    }
    if let Ok(mut runtimes) = runtimes().lock() {
        runtimes.insert(plugin_id, State::Error(reason));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativePacket(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    _: jni::objects::JString<'_>,
    _: jni::objects::JByteArray<'_>,
) {
}

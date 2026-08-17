use eframe::egui;
use std::{
    cell::RefCell,
    sync::{Mutex, OnceLock},
};

const MAX_PACKET_BYTES: usize = block_plugin_api::MAX_FRAME_BYTES + 4;

thread_local! {
    static WINDOW: RefCell<Window> = RefCell::new(Window::default());
}

static EVENTS: OnceLock<Mutex<Vec<Event>>> = OnceLock::new();

#[derive(Default)]
struct Window {
    open: bool,
    state: State,
}

#[derive(Default)]
enum State {
    #[default]
    Closed,
    Binding,
    Connected,
    Error(String),
}

enum Event {
    Connected,
    Disconnected(String),
    Packet(Vec<u8>),
}

pub(crate) fn install(_: &eframe::CreationContext<'_>) {}

pub(crate) fn open() {
    WINDOW.with(|window| {
        let mut window = window.borrow_mut();
        if !window.open {
            window.open = true;
            window.state = State::Binding;
            if let Err(error) = bind() {
                window.state = State::Error(error);
            }
        }
    });
}

pub(crate) fn show(ctx: &egui::Context) {
    drain_events();
    WINDOW.with(|window| {
        let mut window = window.borrow_mut();
        if !window.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Plugin Demo")
            .open(&mut open)
            .default_size([360.0, 140.0])
            .show(ctx, |ui| match &window.state {
                State::Closed => ui.label("Plugin service is closed"),
                State::Binding => ui.label("Connecting to plugin service..."),
                State::Connected => ui.label("Plugin service connected; waiting for a surface"),
                State::Error(error) => ui.colored_label(egui::Color32::RED, error),
            });
        if !open {
            unbind();
            window.open = false;
            window.state = State::Closed;
        }
    });
}

fn drain_events() {
    let events = EVENTS.get_or_init(Default::default);
    let Ok(mut events) = events.lock() else {
        return;
    };
    for event in events.drain(..) {
        WINDOW.with(|window| {
            let mut window = window.borrow_mut();
            match event {
                Event::Connected => window.state = State::Connected,
                Event::Disconnected(error) => window.state = State::Error(error),
                Event::Packet(packet) => {
                    if block_plugin_api::decode_frame(&packet).is_err() {
                        window.state =
                            State::Error("Plugin service sent a malformed protocol packet".into());
                    }
                }
            }
        });
    }
}

fn bind() -> Result<(), String> {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let bound = vm
        .attach_current_thread_for_scope(|environment| {
            let activity =
                unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
            environment
                .call_static_method(
                    "com/be3/block/plugin/PluginHostBridge",
                    "bind",
                    "(Landroid/content/Context;)Z",
                    &[jni::objects::JValue::Object(&activity)],
                )
                .and_then(|value| value.z())
        })
        .map_err(|error| error.to_string())?;
    if bound {
        Ok(())
    } else {
        Err("Plugin service is unavailable; Android API 26 or newer is required".into())
    }
}

fn unbind() {
    let context = ndk_context::android_context();
    let vm = unsafe { jni::vm::JavaVM::from_raw(context.vm().cast()) };
    let _ = vm.attach_current_thread_for_scope(|environment| {
        let activity =
            unsafe { jni::objects::JObject::from_raw(environment, context.context().cast()) };
        environment.call_static_method(
            "com/be3/block/plugin/PluginHostBridge",
            "unbind",
            "(Landroid/content/Context;)V",
            &[jni::objects::JValue::Object(&activity)],
        )?;
        Ok::<_, jni::errors::Error>(())
    });
}

fn push(event: Event) {
    if let Ok(mut events) = EVENTS.get_or_init(Default::default).lock() {
        events.push(event);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeConnected(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
) {
    push(Event::Connected);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativeDisconnected(
    mut environment: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    reason: jni::objects::JString<'_>,
) {
    let reason = environment
        .with_env(|environment| environment.get_string(&reason).map(|value| value.into()))
        .unwrap_or_else(|_| "Plugin service disconnected".into());
    push(Event::Disconnected(reason));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_PluginHostBridge_nativePacket(
    mut environment: jni::JNIEnv<'_>,
    _: jni::objects::JClass<'_>,
    packet: jni::objects::JByteArray<'_>,
) {
    match environment.with_env(|environment| environment.convert_byte_array(packet)) {
        Ok(packet) if packet.len() <= MAX_PACKET_BYTES => push(Event::Packet(packet)),
        _ => push(Event::Disconnected(
            "Plugin service packet exceeded the size limit".into(),
        )),
    }
}

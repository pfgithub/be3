pub mod demo;

block_plugin::plugin!(demo::Demo, "be3.plugin-demo", "Plugin Demo");

#[cfg(target_os = "android")]
mod android {
    use std::sync::{Mutex, OnceLock};

    static WORKER: OnceLock<Mutex<Option<std::thread::JoinHandle<()>>>> = OnceLock::new();

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_be3_block_plugin_PluginDemoService_nativeStart(
        _: jni::JNIEnv<'_>,
        _: jni::objects::JClass<'_>,
    ) {
        let Ok(mut worker) = WORKER.get_or_init(Default::default).lock() else {
            return;
        };
        if worker.is_none() {
            *worker = Some(std::thread::spawn(|| {
                let _session = block_plugin::native::ClientSession::new(
                    "be3.plugin-demo",
                    "Plugin Demo",
                    env!("CARGO_PKG_VERSION"),
                );
            }));
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_be3_block_plugin_PluginDemoService_nativeShutdown(
        _: jni::JNIEnv<'_>,
        _: jni::objects::JClass<'_>,
    ) {
        if let Ok(mut worker) = WORKER.get_or_init(Default::default).lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

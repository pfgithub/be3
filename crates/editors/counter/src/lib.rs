pub mod demo;

block_editor_plugin::plugin!(demo::CounterApp, "be3.counter", "Counter");

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_CounterService_nativeStart(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_CounterService_nativeReceive(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    _: jni::objects::JByteArray<'_>,
) {
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_CounterService_nativeShutdown(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
}

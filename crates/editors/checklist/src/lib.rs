pub mod app;

block_editor_plugin::plugin!(app::ChecklistApp, "be3.checklist", "Checklist");

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_ChecklistService_nativeStart(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_ChecklistService_nativeReceive(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
    _: jni::objects::JByteArray<'_>,
) {
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_plugin_ChecklistService_nativeShutdown(
    _: jni::EnvUnowned<'_>,
    _: jni::objects::JClass<'_>,
) {
}

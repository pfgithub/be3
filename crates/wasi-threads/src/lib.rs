#[cfg(all(target_os = "wasi", target_feature = "atomics"))]
unsafe extern "C" {
    fn __wasi_init_tp();
}

pub fn initialize_main_thread() {
    #[cfg(all(target_os = "wasi", target_feature = "atomics"))]
    unsafe {
        __wasi_init_tp();
    }
}

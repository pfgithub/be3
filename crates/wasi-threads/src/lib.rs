#[cfg(all(target_os = "wasi", target_feature = "atomics"))]
unsafe extern "C" {
    fn __wasi_init_tp();
}

#[cfg(target_os = "wasi")]
unsafe extern "C" {
    #[link_name = "__wasi_init_tp"]
    fn init_thread_pointer();
    fn __wasm_init_tls(block: *mut u8);
}

pub fn initialize_main_thread() {
    #[cfg(all(target_os = "wasi", target_feature = "atomics"))]
    unsafe {
        __wasi_init_tp();
    }
}

pub fn initialize_main_thread_storage(size: usize, align: usize) {
    #[cfg(target_os = "wasi")]
    unsafe {
        if size > 0 {
            if let Ok(layout) = std::alloc::Layout::from_size_align(size, align.max(1)) {
                let block = std::alloc::alloc_zeroed(layout);
                if !block.is_null() {
                    __wasm_init_tls(block);
                }
            }
        }
        init_thread_pointer();
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let _ = (size, align);
    }
}

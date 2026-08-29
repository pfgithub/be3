use block_gpu_abi as abi;

#[link(wasm_import_module = "be3_host")]
extern "C" {
    fn host_send(pointer: u32, length: u32);
    fn host_receive(pointer: u32, capacity: u32) -> i64;
    fn host_now() -> f64;
}

pub(crate) fn send(frame: &[u8]) {
    unsafe { host_send(frame.as_ptr() as u32, frame.len() as u32) };
}

pub(crate) fn receive() -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; 4096];
    let needed = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed == abi::NO_MESSAGE {
        return None;
    }
    let needed = needed as usize;
    if needed > buffer.len() {
        buffer = vec![0u8; needed];
        let again = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
        if again == abi::NO_MESSAGE {
            return None;
        }
    }
    buffer.truncate(needed);
    Some(buffer)
}

pub(crate) fn now() -> f64 {
    unsafe { host_now() }
}

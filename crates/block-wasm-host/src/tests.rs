use super::*;

use block_gpu_abi as abi;

fn devices() -> (wgpu::Device, wgpu::Queue) {
    wgpu::Device::noop(&wgpu::DeviceDescriptor::default())
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn buffer_descriptor() -> Vec<u8> {
    abi::encode(&abi::BufferDescriptor {
        label: "vertices".into(),
        size: 256,
        usage: wgpu::BufferUsages::VERTEX.bits(),
        mapped_at_creation: false,
    })
}

fn guest(body: &str) -> String {
    let descriptor = buffer_descriptor();
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "be3_gpu" "create_buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3_gpu" "error_take" (func $error_take (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_receive" (func $host_receive (param i32 i32) (result i64)))
            (data (i32.const 0) "{descriptor}")
            (global $length i32 (i32.const {length}))
            (global (export "__tls_size") i32 (i32.const 0))
            (global (export "__tls_align") i32 (i32.const 1))
            (func (export "plugin_initialize_tls") (param i32 i32))
            (func (export "plugin_start"))
            (func (export "plugin_shutdown"))
            (func (export "plugin_step") {body})
        )"#,
        descriptor = escaped(&descriptor),
        length = descriptor.len(),
    )
}

mod a_guest_creates_a_buffer_and_reports_its_handle;
mod a_guest_pointer_past_its_memory_is_refused;
mod a_guest_reads_a_frame_the_host_queued;

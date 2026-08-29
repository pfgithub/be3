use super::*;

use block_gpu_abi as abi;

fn host() -> Host {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Host::new(device, queue, None).unwrap()
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

fn threaded_guest(body: &str) -> String {
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "wasi" "thread-spawn" (func $spawn (param i32) (result i32)))
            (import "be3_gpu" "create_buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (global (export "__tls_size") i32 (i32.const 0))
            (global (export "__tls_align") i32 (i32.const 1))
            (func (export "plugin_initialize_tls") (param i32 i32))
            (func (export "plugin_start") (drop (call $spawn (i32.const 0))))
            (func (export "plugin_shutdown"))
            (func (export "plugin_step")
                (call $host_send (i32.const 8) (i32.const 1)))
            (func (export "wasi_thread_start") (param i32 i32) {body})
        )"#
    )
}

fn settled(plugin: &mut Plugin, wanted: u8) -> Result<bool, String> {
    for _ in 0..500 {
        plugin.step()?;
        if plugin
            .take_outbound()
            .last()
            .is_some_and(|frame| frame == &[wanted])
        {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Ok(false)
}

mod a_guest_creates_a_buffer_and_reports_its_handle;
mod a_guest_pointer_past_its_memory_is_refused;
mod a_guest_reads_a_frame_the_host_queued;
mod a_plugin_runs_on_a_thread_of_its_own;
mod a_spawned_guest_thread_may_not_reach_the_gpu;
mod a_spawned_guest_thread_shares_the_memory_it_was_given;

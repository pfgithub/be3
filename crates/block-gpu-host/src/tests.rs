use super::*;

fn gpu() -> Gpu {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Gpu::new(device, queue)
}

fn buffer_descriptor() -> Vec<u8> {
    abi::encode(&abi::BufferDescriptor {
        label: "vertices".into(),
        size: 256,
        usage: wgpu::BufferUsages::VERTEX.bits() | wgpu::BufferUsages::COPY_DST.bits(),
        mapped_at_creation: false,
    })
}

mod a_created_buffer_gets_a_live_handle;
mod a_dropped_handle_is_reported_not_reused;
mod a_malformed_descriptor_is_reported;
mod a_surface_must_be_attached_before_it_is_acquired;
mod an_unknown_pass_handle_is_reported;

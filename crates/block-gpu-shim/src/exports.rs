use block_gpu_abi as abi;

use crate::{with, SHIM};

macro_rules! scalar {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)? => $method:ident;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name($($argument: $kind),*) $(-> $result)? {
                with(|shim| shim.gpu.$method($($argument),*), Default::default())
            }
        )+
    };
}

macro_rules! described {
    ($($name:ident => $method:ident;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name(pointer: u32, length: u32) -> u32 {
                with(
                    |shim| {
                        let bytes = read(&shim.scratch, pointer, length);
                        shim.gpu.$method(&bytes)
                    },
                    abi::NULL_HANDLE,
                )
            }
        )+
    };
}

#[no_mangle]
pub extern "C" fn be3_scratch(length: u32) -> u32 {
    SHIM.with(|shim| {
        let mut shim = shim.borrow_mut();
        let Some(shim) = shim.as_mut() else {
            return 0;
        };
        shim.scratch.clear();
        shim.scratch.resize(length as usize, 0);
        shim.scratch.as_ptr() as u32
    })
}

fn read(scratch: &[u8], pointer: u32, length: u32) -> Vec<u8> {
    let base = scratch.as_ptr() as u32;
    let start = pointer.saturating_sub(base) as usize;
    let end = start.saturating_add(length as usize).min(scratch.len());
    scratch.get(start..end).unwrap_or_default().to_vec()
}

fn words(scratch: &[u8], pointer: u32, count: u32) -> Vec<u32> {
    read(scratch, pointer, count.saturating_mul(4))
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn give(scratch: &mut [u8], pointer: u32, capacity: u32, bytes: &[u8]) -> u32 {
    let needed = bytes.len() as u32;
    if needed > capacity {
        return needed;
    }
    let base = scratch.as_ptr() as u32;
    let start = pointer.saturating_sub(base) as usize;
    let Some(destination) = start
        .checked_add(bytes.len())
        .and_then(|end| scratch.get_mut(start..end))
    else {
        return 0;
    };
    destination.copy_from_slice(bytes);
    needed
}

described! {
    create_buffer => create_buffer;
    create_texture => create_texture;
    create_texture_view => create_texture_view;
    create_sampler => create_sampler;
    create_bind_group_layout => create_bind_group_layout;
    create_bind_group => create_bind_group;
    create_pipeline_layout => create_pipeline_layout;
    create_shader_module => create_shader_module;
    create_render_pipeline => create_render_pipeline;
    create_command_encoder => create_command_encoder;
    encoder_begin_render_pass => begin_render_pass;
}

scalar! {
    fn buffer_unmap(buffer: u32) => unmap_buffer;
    fn encoder_finish(encoder: u32) -> u32 => finish_encoder;
    fn pass_set_pipeline(pass: u32, pipeline: u32) => set_pipeline;
    fn pass_set_index_buffer(pass: u32, buffer: u32, format: u32, offset: u64, size: u64)
        => set_index_buffer;
    fn pass_set_vertex_buffer(pass: u32, slot: u32, buffer: u32, offset: u64, size: u64)
        => set_vertex_buffer;
    fn pass_set_viewport(
        pass: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    ) => set_viewport;
    fn pass_set_scissor_rect(pass: u32, x: u32, y: u32, width: u32, height: u32)
        => set_scissor_rect;
    fn pass_set_blend_constant(pass: u32, red: f32, green: f32, blue: f32, alpha: f32)
        => set_blend_constant;
    fn pass_set_stencil_reference(pass: u32, reference: u32) => set_stencil_reference;
    fn pass_draw(
        pass: u32,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    ) => draw;
    fn pass_draw_indexed(
        pass: u32,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    ) => draw_indexed;
    fn pass_end(pass: u32) => end_pass;
    fn resource_drop(kind: u32, handle: u32) => drop_resource;
}

#[no_mangle]
pub extern "C" fn device_limits(pointer: u32, capacity: u32) -> u32 {
    with(
        |shim| {
            let bytes = shim.gpu.limits();
            give(&mut shim.scratch, pointer, capacity, &bytes)
        },
        0,
    )
}

#[no_mangle]
pub extern "C" fn buffer_write_mapped(buffer: u32, offset: u64, pointer: u32, length: u32) {
    with(
        |shim| {
            let data = read(&shim.scratch, pointer, length);
            shim.gpu.write_mapped_buffer(buffer, offset, &data);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn queue_write_buffer(buffer: u32, offset: u64, pointer: u32, length: u32) {
    with(
        |shim| {
            let data = read(&shim.scratch, pointer, length);
            shim.gpu.write_buffer(buffer, offset, &data);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn queue_write_texture(pointer: u32, length: u32, data: u32, data_length: u32) {
    with(
        |shim| {
            let request = read(&shim.scratch, pointer, length);
            let payload = read(&shim.scratch, data, data_length);
            shim.gpu.write_texture(&request, &payload);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn queue_submit(pointer: u32, length: u32) {
    with(
        |shim| {
            let handles = words(&shim.scratch, pointer, length);
            shim.gpu.submit(&handles);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn pass_set_bind_group(
    pass: u32,
    index: u32,
    group: u32,
    offsets: u32,
    offsets_length: u32,
) {
    with(
        |shim| {
            let offsets = words(&shim.scratch, offsets, offsets_length);
            shim.gpu.set_bind_group(pass, index, group, &offsets);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn surface_configure(surface: u32, pointer: u32, length: u32) {
    with(
        |shim| {
            let bytes = read(&shim.scratch, pointer, length);
            let configuration = match abi::decode(&bytes) {
                Ok(configuration) => configuration,
                Err(message) => return shim.report(message),
            };
            let device = shim.gpu.device().clone();
            if let Err(message) = shim.canvas.configure(&device, &configuration) {
                shim.report(message);
                return;
            }
            let _ = surface;
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn surface_acquire(surface: u32) -> u32 {
    with(
        |shim| {
            let device = shim.gpu.device().clone();
            match shim.canvas.acquire(&device) {
                Ok(texture) => {
                    shim.gpu.attach_surface(surface, texture);
                    shim.gpu.acquire_surface(surface)
                }
                Err(message) => {
                    shim.report(message);
                    abi::NULL_HANDLE
                }
            }
        },
        abi::NULL_HANDLE,
    )
}

#[no_mangle]
pub extern "C" fn surface_present(surface: u32) {
    with(
        |shim| {
            shim.canvas.present();
            shim.gpu.present_surface(surface);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn texture_describe(texture: u32, pointer: u32, capacity: u32) -> u32 {
    with(
        |shim| match shim.gpu.describe_texture(texture) {
            Some(bytes) => give(&mut shim.scratch, pointer, capacity, &bytes),
            None => 0,
        },
        0,
    )
}

#[no_mangle]
pub extern "C" fn error_take(pointer: u32, capacity: u32) -> u32 {
    with(
        |shim| {
            let message = shim.gpu.take_error().unwrap_or_default();
            give(&mut shim.scratch, pointer, capacity, message.as_bytes())
        },
        0,
    )
}

#[no_mangle]
pub extern "C" fn host_send(pointer: u32, length: u32) {
    with(
        |shim| {
            let frame = read(&shim.scratch, pointer, length);
            shim.outbox.push(frame);
        },
        (),
    )
}

#[no_mangle]
pub extern "C" fn host_receive(pointer: u32, capacity: u32) -> i64 {
    with(
        |shim| {
            let Some(frame) = shim.inbox.front() else {
                return abi::NO_MESSAGE;
            };
            let needed = frame.len() as u32;
            if needed > capacity {
                return needed as i64;
            }
            let frame = shim.inbox.pop_front().unwrap_or_default();
            give(&mut shim.scratch, pointer, capacity, &frame);
            needed as i64
        },
        abi::NO_MESSAGE,
    )
}

#[no_mangle]
pub extern "C" fn host_now() -> f64 {
    with(|shim| (crate::now() - shim.started) / 1000.0, 0.0)
}

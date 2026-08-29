use block_gpu_abi as abi;
use wasmtime::{Caller, Linker};

use crate::State;

macro_rules! described {
    ($linker:expr, $($name:ident),+ $(,)?) => {
        $(
            wrap(
                $linker,
                stringify!($name),
                |mut caller: Caller<'_, State>, pointer: u32, length: u32| -> u32 {
                    let state = caller.data_mut();
                    match state.read(pointer, length) {
                        Ok(bytes) => state.gpu.$name(&bytes),
                        Err(message) => {
                            state.report(message);
                            abi::NULL_HANDLE
                        }
                    }
                },
            )?;
        )+
    };
}

pub(super) fn link(linker: &mut Linker<State>) -> Result<(), String> {
    described!(
        linker,
        create_buffer,
        create_texture,
        create_texture_view,
        create_sampler,
        create_bind_group_layout,
        create_bind_group,
        create_pipeline_layout,
        create_shader_module,
        create_render_pipeline,
        create_command_encoder,
    );
    wrap(
        linker,
        "device_limits",
        |mut caller: Caller<'_, State>, pointer: u32, capacity: u32| -> u32 {
            let state = caller.data_mut();
            let bytes = state.gpu.limits();
            state.write(pointer, capacity, &bytes)
        },
    )?;
    wrap(
        linker,
        "queue_write_buffer",
        |mut caller: Caller<'_, State>, buffer: u32, offset: u64, pointer: u32, length: u32| {
            let state = caller.data_mut();
            match state.read(pointer, length) {
                Ok(data) => state.gpu.write_buffer(buffer, offset, &data),
                Err(message) => state.report(message),
            }
        },
    )?;
    wrap(
        linker,
        "queue_write_texture",
        |mut caller: Caller<'_, State>, pointer: u32, length: u32, data: u32, data_length: u32| {
            let state = caller.data_mut();
            let request = match state.read(pointer, length) {
                Ok(request) => request,
                Err(message) => return state.report(message),
            };
            let payload = match state.read(data, data_length) {
                Ok(payload) => payload,
                Err(message) => return state.report(message),
            };
            state.gpu.write_texture(&request, &payload);
        },
    )?;
    wrap(
        linker,
        "queue_submit",
        |mut caller: Caller<'_, State>, pointer: u32, length: u32| {
            let state = caller.data_mut();
            match state.read_words(pointer, length) {
                Ok(handles) => state.gpu.submit(&handles),
                Err(message) => state.report(message),
            }
        },
    )?;
    wrap(
        linker,
        "encoder_begin_render_pass",
        |mut caller: Caller<'_, State>, pointer: u32, length: u32| -> u32 {
            let state = caller.data_mut();
            match state.read(pointer, length) {
                Ok(bytes) => state.gpu.begin_render_pass(&bytes),
                Err(message) => {
                    state.report(message);
                    abi::NULL_HANDLE
                }
            }
        },
    )?;
    wrap(
        linker,
        "encoder_finish",
        |mut caller: Caller<'_, State>, encoder: u32| -> u32 {
            caller.data_mut().gpu.finish_encoder(encoder)
        },
    )?;
    wrap(
        linker,
        "pass_set_pipeline",
        |mut caller: Caller<'_, State>, pass: u32, pipeline: u32| {
            caller.data_mut().gpu.set_pipeline(pass, pipeline);
        },
    )?;
    wrap(
        linker,
        "pass_set_bind_group",
        |mut caller: Caller<'_, State>,
         pass: u32,
         index: u32,
         group: u32,
         offsets: u32,
         offsets_length: u32| {
            let state = caller.data_mut();
            match state.read_words(offsets, offsets_length) {
                Ok(offsets) => state.gpu.set_bind_group(pass, index, group, &offsets),
                Err(message) => state.report(message),
            }
        },
    )?;
    wrap(
        linker,
        "pass_set_index_buffer",
        |mut caller: Caller<'_, State>,
         pass: u32,
         buffer: u32,
         format: u32,
         offset: u64,
         size: u64| {
            caller
                .data_mut()
                .gpu
                .set_index_buffer(pass, buffer, format, offset, size);
        },
    )?;
    wrap(
        linker,
        "pass_set_vertex_buffer",
        |mut caller: Caller<'_, State>,
         pass: u32,
         slot: u32,
         buffer: u32,
         offset: u64,
         size: u64| {
            caller
                .data_mut()
                .gpu
                .set_vertex_buffer(pass, slot, buffer, offset, size);
        },
    )?;
    wrap(
        linker,
        "pass_set_viewport",
        |mut caller: Caller<'_, State>,
         pass: u32,
         x: f32,
         y: f32,
         width: f32,
         height: f32,
         minimum_depth: f32,
         maximum_depth: f32| {
            caller.data_mut().gpu.set_viewport(
                pass,
                x,
                y,
                width,
                height,
                minimum_depth,
                maximum_depth,
            );
        },
    )?;
    wrap(
        linker,
        "pass_set_scissor_rect",
        |mut caller: Caller<'_, State>, pass: u32, x: u32, y: u32, width: u32, height: u32| {
            caller
                .data_mut()
                .gpu
                .set_scissor_rect(pass, x, y, width, height);
        },
    )?;
    wrap(
        linker,
        "pass_set_blend_constant",
        |mut caller: Caller<'_, State>, pass: u32, red: f32, green: f32, blue: f32, alpha: f32| {
            caller
                .data_mut()
                .gpu
                .set_blend_constant(pass, red, green, blue, alpha);
        },
    )?;
    wrap(
        linker,
        "pass_set_stencil_reference",
        |mut caller: Caller<'_, State>, pass: u32, reference: u32| {
            caller.data_mut().gpu.set_stencil_reference(pass, reference);
        },
    )?;
    wrap(
        linker,
        "pass_draw",
        |mut caller: Caller<'_, State>,
         pass: u32,
         first_vertex: u32,
         vertex_count: u32,
         first_instance: u32,
         instance_count: u32| {
            caller.data_mut().gpu.draw(
                pass,
                first_vertex,
                vertex_count,
                first_instance,
                instance_count,
            );
        },
    )?;
    wrap(
        linker,
        "pass_draw_indexed",
        |mut caller: Caller<'_, State>,
         pass: u32,
         first_index: u32,
         index_count: u32,
         base_vertex: i32,
         first_instance: u32,
         instance_count: u32| {
            caller.data_mut().gpu.draw_indexed(
                pass,
                first_index,
                index_count,
                base_vertex,
                first_instance,
                instance_count,
            );
        },
    )?;
    wrap(
        linker,
        "pass_end",
        |mut caller: Caller<'_, State>, pass: u32| {
            caller.data_mut().gpu.end_pass(pass);
        },
    )?;
    wrap(
        linker,
        "resource_drop",
        |mut caller: Caller<'_, State>, kind: u32, handle: u32| {
            caller.data_mut().gpu.drop_resource(kind, handle);
        },
    )?;
    wrap(
        linker,
        "surface_acquire",
        |mut caller: Caller<'_, State>, surface: u32| -> u32 {
            caller.data_mut().gpu.acquire_surface(surface)
        },
    )?;
    wrap(
        linker,
        "surface_present",
        |mut caller: Caller<'_, State>, surface: u32| {
            caller.data_mut().gpu.present_surface(surface);
        },
    )?;
    wrap(
        linker,
        "texture_describe",
        |mut caller: Caller<'_, State>, texture: u32, pointer: u32, capacity: u32| -> u32 {
            let state = caller.data_mut();
            match state.gpu.describe_texture(texture) {
                Some(bytes) => state.write(pointer, capacity, &bytes),
                None => 0,
            }
        },
    )?;
    wrap(
        linker,
        "error_take",
        |mut caller: Caller<'_, State>, pointer: u32, capacity: u32| -> u32 {
            let state = caller.data_mut();
            let message = state.gpu.take_error().unwrap_or_default();
            state.write(pointer, capacity, message.as_bytes())
        },
    )?;
    Ok(())
}

fn wrap<Parameters, Results>(
    linker: &mut Linker<State>,
    name: &str,
    function: impl wasmtime::IntoFunc<State, Parameters, Results>,
) -> Result<(), String> {
    linker
        .func_wrap(abi::GPU_MODULE, name, function)
        .map(|_| ())
        .map_err(|error| format!("{name} could not be linked: {error}"))
}

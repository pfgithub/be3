macro_rules! gpu_imports {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        #[cfg(target_arch = "wasm32")]
        #[link(wasm_import_module = "be3_gpu")]
        extern "C" {
            $(pub(crate) fn $name($($argument: $kind),*) $(-> $result)?;)+
        }

        #[cfg(not(target_arch = "wasm32"))]
        mod absent {
            $(
                pub(crate) unsafe fn $name($(_: $kind),*) $(-> $result)? {
                    unreachable!("the plugin gpu abi is only callable from wasm")
                }
            )+
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub(crate) use absent::*;
    };
}

gpu_imports! {
    fn device_limits(pointer: u32, capacity: u32) -> u32;
    fn create_buffer(pointer: u32, length: u32) -> u32;
    fn create_texture(pointer: u32, length: u32) -> u32;
    fn create_texture_view(pointer: u32, length: u32) -> u32;
    fn create_sampler(pointer: u32, length: u32) -> u32;
    fn create_bind_group_layout(pointer: u32, length: u32) -> u32;
    fn create_bind_group(pointer: u32, length: u32) -> u32;
    fn create_pipeline_layout(pointer: u32, length: u32) -> u32;
    fn create_shader_module(pointer: u32, length: u32) -> u32;
    fn create_render_pipeline(pointer: u32, length: u32) -> u32;
    fn create_command_encoder(pointer: u32, length: u32) -> u32;
    fn buffer_write_mapped(buffer: u32, offset: u64, pointer: u32, length: u32);
    fn buffer_unmap(buffer: u32);
    fn queue_write_buffer(buffer: u32, offset: u64, pointer: u32, length: u32);
    fn queue_write_texture(pointer: u32, length: u32, data: u32, data_length: u32);
    fn queue_submit(pointer: u32, length: u32);
    fn encoder_begin_render_pass(pointer: u32, length: u32) -> u32;
    fn encoder_finish(encoder: u32) -> u32;
    fn pass_set_pipeline(pass: u32, pipeline: u32);
    fn pass_set_bind_group(
        pass: u32,
        index: u32,
        bind_group: u32,
        offsets: u32,
        offsets_length: u32,
    );
    fn pass_set_index_buffer(pass: u32, buffer: u32, format: u32, offset: u64, size: u64);
    fn pass_set_vertex_buffer(pass: u32, slot: u32, buffer: u32, offset: u64, size: u64);
    fn pass_set_viewport(
        pass: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    );
    fn pass_set_scissor_rect(pass: u32, x: u32, y: u32, width: u32, height: u32);
    fn pass_set_blend_constant(pass: u32, red: f32, green: f32, blue: f32, alpha: f32);
    fn pass_set_stencil_reference(pass: u32, reference: u32);
    fn pass_draw(
        pass: u32,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    );
    fn pass_draw_indexed(
        pass: u32,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    );
    fn pass_end(pass: u32);
    fn resource_drop(kind: u32, handle: u32);
    fn surface_configure(surface: u32, pointer: u32, length: u32);
    fn surface_acquire(surface: u32) -> u32;
    fn surface_present(surface: u32);
    fn texture_describe(texture: u32, pointer: u32, capacity: u32) -> u32;
    fn error_take(pointer: u32, capacity: u32) -> u32;
}

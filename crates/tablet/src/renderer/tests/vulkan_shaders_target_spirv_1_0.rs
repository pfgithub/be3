use super::super::vulkan::compile_shader;

#[test]
fn vulkan_shaders_target_spirv_1_0() {
    for (stage, entry_point) in [
        (naga::ShaderStage::Vertex, "vs_main"),
        (naga::ShaderStage::Fragment, "fs_main"),
    ] {
        let words = compile_shader(stage, entry_point).expect("shader should compile");

        assert_eq!(words[0], 0x0723_0203, "SPIR-V magic number");
        assert_eq!(words[1], 0x0001_0000, "SPIR-V version 1.0");
    }
}

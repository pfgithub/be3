use super::*;

#[test]
fn every_resource_kind_survives_its_code() {
    let kinds = [
        ResourceKind::Buffer,
        ResourceKind::Texture,
        ResourceKind::TextureView,
        ResourceKind::Sampler,
        ResourceKind::BindGroupLayout,
        ResourceKind::BindGroup,
        ResourceKind::PipelineLayout,
        ResourceKind::ShaderModule,
        ResourceKind::RenderPipeline,
        ResourceKind::CommandEncoder,
        ResourceKind::CommandBuffer,
        ResourceKind::RenderPass,
    ];
    for kind in kinds {
        assert_eq!(ResourceKind::from_code(kind.code()), Some(kind));
    }
    assert_eq!(ResourceKind::from_code(0), None);
    assert_eq!(ResourceKind::from_code(13), None);
}

use super::*;

fn render_pipeline() -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: "egui".into(),
        layout: Some(7),
        vertex: VertexState {
            module: 3,
            entry_point: Some("vs_main".into()),
            buffers: vec![VertexBufferLayout {
                array_stride: 20,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    VertexAttribute {
                        format: VertexFormat::Unorm8x4,
                        offset: 16,
                        shader_location: 2,
                    },
                ],
            }],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(FragmentState {
            module: 3,
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Bgra8Unorm,
                blend: Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::OneMinusDstAlpha,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                }),
                write_mask: 0xf,
            })],
        }),
    }
}

mod a_render_pass_descriptor_round_trips;
mod a_render_pipeline_descriptor_round_trips;
mod decoding_rejects_truncated_bytes;
mod every_resource_kind_survives_its_code;

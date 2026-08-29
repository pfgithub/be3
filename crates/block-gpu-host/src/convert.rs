use block_gpu_abi as abi;

macro_rules! plain_map {
    ($function:ident, $from:path, $to:path, { $($variant:ident),+ $(,)? }) => {
        pub(crate) fn $function(value: $from) -> $to {
            use $from as From;
            use $to as To;
            match value {
                $(From::$variant => To::$variant,)+
            }
        }
    };
}

plain_map!(texture_format, abi::TextureFormat, wgpu::TextureFormat, {
    R8Unorm, R8Snorm, R8Uint, R8Sint,
    R16Uint, R16Sint, R16Float,
    Rg8Unorm, Rg8Snorm, Rg8Uint, Rg8Sint,
    R32Uint, R32Sint, R32Float,
    Rg16Uint, Rg16Sint, Rg16Float,
    Rgba8Unorm, Rgba8UnormSrgb, Rgba8Snorm, Rgba8Uint, Rgba8Sint,
    Bgra8Unorm, Bgra8UnormSrgb,
    Rgb9e5Ufloat, Rgb10a2Uint, Rgb10a2Unorm, Rg11b10Ufloat,
    Rg32Uint, Rg32Sint, Rg32Float,
    Rgba16Uint, Rgba16Sint, Rgba16Float,
    Rgba32Uint, Rgba32Sint, Rgba32Float,
    Stencil8, Depth16Unorm, Depth24Plus, Depth24PlusStencil8, Depth32Float, Depth32FloatStencil8,
});

plain_map!(texture_dimension, abi::TextureDimension, wgpu::TextureDimension, {
    D1, D2, D3,
});

plain_map!(texture_view_dimension, abi::TextureViewDimension, wgpu::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(texture_aspect, abi::TextureAspect, wgpu::TextureAspect, {
    All, StencilOnly, DepthOnly,
});

plain_map!(address_mode, abi::AddressMode, wgpu::AddressMode, {
    ClampToEdge, Repeat, MirrorRepeat, ClampToBorder,
});

plain_map!(filter_mode, abi::FilterMode, wgpu::FilterMode, { Nearest, Linear });

plain_map!(mipmap_filter_mode, abi::FilterMode, wgpu::MipmapFilterMode, { Nearest, Linear });

plain_map!(compare_function, abi::CompareFunction, wgpu::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(border_color, abi::SamplerBorderColor, wgpu::SamplerBorderColor, {
    TransparentBlack, OpaqueBlack, OpaqueWhite, Zero,
});

plain_map!(vertex_format, abi::VertexFormat, wgpu::VertexFormat, {
    Uint8x2, Uint8x4, Sint8x2, Sint8x4,
    Unorm8x2, Unorm8x4, Snorm8x2, Snorm8x4,
    Uint16x2, Uint16x4, Sint16x2, Sint16x4,
    Unorm16x2, Unorm16x4, Snorm16x2, Snorm16x4,
    Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
});

plain_map!(step_mode, abi::VertexStepMode, wgpu::VertexStepMode, { Vertex, Instance });

plain_map!(topology, abi::PrimitiveTopology, wgpu::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(index_format, abi::IndexFormat, wgpu::IndexFormat, { Uint16, Uint32 });

plain_map!(front_face, abi::FrontFace, wgpu::FrontFace, { Ccw, Cw });

plain_map!(face, abi::Face, wgpu::Face, { Front, Back });

plain_map!(polygon_mode, abi::PolygonMode, wgpu::PolygonMode, { Fill, Line, Point });

plain_map!(blend_factor, abi::BlendFactor, wgpu::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
});

plain_map!(blend_operation, abi::BlendOperation, wgpu::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(stencil_operation, abi::StencilOperation, wgpu::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(sampler_binding, abi::SamplerBindingType, wgpu::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(storage_access, abi::StorageTextureAccess, wgpu::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(store_op, abi::StoreOp, wgpu::StoreOp, { Store, Discard });

pub(crate) fn label(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

pub(crate) fn extent(value: abi::Extent3d) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: value.width,
        height: value.height,
        depth_or_array_layers: value.depth_or_array_layers,
    }
}

pub(crate) fn size(value: u64) -> Option<wgpu::BufferSize> {
    wgpu::BufferSize::new(value)
}

pub(crate) fn blend_component(value: abi::BlendComponent) -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        src_factor: blend_factor(value.src_factor),
        dst_factor: blend_factor(value.dst_factor),
        operation: blend_operation(value.operation),
    }
}

pub(crate) fn stencil_face(value: abi::StencilFaceState) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare: compare_function(value.compare),
        fail_op: stencil_operation(value.fail_op),
        depth_fail_op: stencil_operation(value.depth_fail_op),
        pass_op: stencil_operation(value.pass_op),
    }
}

pub(crate) fn color(value: abi::Color) -> wgpu::Color {
    wgpu::Color {
        r: value.red,
        g: value.green,
        b: value.blue,
        a: value.alpha,
    }
}

pub(crate) fn binding_type(value: abi::BindingType) -> wgpu::BindingType {
    match value {
        abi::BindingType::Buffer {
            kind,
            has_dynamic_offset,
            min_binding_size,
        } => wgpu::BindingType::Buffer {
            ty: match kind {
                abi::BufferBindingType::Uniform => wgpu::BufferBindingType::Uniform,
                abi::BufferBindingType::Storage => {
                    wgpu::BufferBindingType::Storage { read_only: false }
                }
                abi::BufferBindingType::ReadOnlyStorage => {
                    wgpu::BufferBindingType::Storage { read_only: true }
                }
            },
            has_dynamic_offset,
            min_binding_size: size(min_binding_size),
        },
        abi::BindingType::Sampler(kind) => wgpu::BindingType::Sampler(sampler_binding(kind)),
        abi::BindingType::Texture {
            sample_kind,
            view_dimension,
            multisampled,
        } => wgpu::BindingType::Texture {
            sample_type: match sample_kind {
                abi::TextureSampleKind::FloatFilterable => {
                    wgpu::TextureSampleType::Float { filterable: true }
                }
                abi::TextureSampleKind::FloatUnfilterable => {
                    wgpu::TextureSampleType::Float { filterable: false }
                }
                abi::TextureSampleKind::Depth => wgpu::TextureSampleType::Depth,
                abi::TextureSampleKind::Sint => wgpu::TextureSampleType::Sint,
                abi::TextureSampleKind::Uint => wgpu::TextureSampleType::Uint,
            },
            view_dimension: texture_view_dimension(view_dimension),
            multisampled,
        },
        abi::BindingType::StorageTexture {
            access,
            format,
            view_dimension,
        } => wgpu::BindingType::StorageTexture {
            access: storage_access(access),
            format: texture_format(format),
            view_dimension: texture_view_dimension(view_dimension),
        },
    }
}

pub(crate) fn limits(value: &wgpu::Limits) -> abi::DeviceLimits {
    abi::DeviceLimits {
        max_texture_dimension_1d: value.max_texture_dimension_1d,
        max_texture_dimension_2d: value.max_texture_dimension_2d,
        max_texture_dimension_3d: value.max_texture_dimension_3d,
        max_texture_array_layers: value.max_texture_array_layers,
        max_bind_groups: value.max_bind_groups,
        max_bindings_per_bind_group: value.max_bindings_per_bind_group,
        max_sampled_textures_per_shader_stage: value.max_sampled_textures_per_shader_stage,
        max_samplers_per_shader_stage: value.max_samplers_per_shader_stage,
        max_uniform_buffers_per_shader_stage: value.max_uniform_buffers_per_shader_stage,
        max_uniform_buffer_binding_size: value.max_uniform_buffer_binding_size,
        max_storage_buffer_binding_size: value.max_storage_buffer_binding_size,
        max_vertex_buffers: value.max_vertex_buffers,
        max_buffer_size: value.max_buffer_size,
        max_vertex_attributes: value.max_vertex_attributes,
        max_vertex_buffer_array_stride: value.max_vertex_buffer_array_stride,
        min_uniform_buffer_offset_alignment: value.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: value.min_storage_buffer_offset_alignment,
    }
}

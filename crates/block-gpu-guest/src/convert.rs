use block_gpu_abi as abi;

use crate::objects::{
    BindGroupLayout, Buffer, PipelineLayout, Sampler, ShaderModule, Texture, TextureView,
};

fn unsupported(name: &str, value: &dyn core::fmt::Debug) -> ! {
    panic!("the plugin gpu abi does not support {name} {value:?}")
}

macro_rules! plain_map {
    ($function:ident, $reverse:ident, $from:path, $to:path, { $($variant:ident),+ $(,)? }) => {
        #[allow(unreachable_patterns)]
        pub(crate) fn $function(value: $from) -> $to {
            use $from as From;
            use $to as To;
            match value {
                $(From::$variant => To::$variant,)+
                other => unsupported(stringify!($from), &other),
            }
        }

        #[allow(dead_code)]
        pub(crate) fn $reverse(value: $to) -> $from {
            use $from as From;
            use $to as To;
            match value {
                $(To::$variant => From::$variant,)+
            }
        }
    };
}

plain_map!(texture_format, wgpu_texture_format, wgpu::TextureFormat, abi::TextureFormat, {
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

plain_map!(texture_dimension, wgpu_texture_dimension, wgpu::TextureDimension, abi::TextureDimension, {
    D1, D2, D3,
});

plain_map!(texture_view_dimension, wgpu_texture_view_dimension, wgpu::TextureViewDimension, abi::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(texture_aspect, wgpu_texture_aspect, wgpu::TextureAspect, abi::TextureAspect, {
    All, StencilOnly, DepthOnly,
});

plain_map!(address_mode, wgpu_address_mode, wgpu::AddressMode, abi::AddressMode, {
    ClampToEdge, Repeat, MirrorRepeat, ClampToBorder,
});

plain_map!(filter_mode, wgpu_filter_mode, wgpu::FilterMode, abi::FilterMode, {
    Nearest, Linear,
});

plain_map!(compare_function, wgpu_compare_function, wgpu::CompareFunction, abi::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(border_color, wgpu_border_color, wgpu::SamplerBorderColor, abi::SamplerBorderColor, {
    TransparentBlack, OpaqueBlack, OpaqueWhite, Zero,
});

plain_map!(vertex_format, wgpu_vertex_format, wgpu::VertexFormat, abi::VertexFormat, {
    Uint8x2, Uint8x4, Sint8x2, Sint8x4,
    Unorm8x2, Unorm8x4, Snorm8x2, Snorm8x4,
    Uint16x2, Uint16x4, Sint16x2, Sint16x4,
    Unorm16x2, Unorm16x4, Snorm16x2, Snorm16x4,
    Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
});

plain_map!(step_mode, wgpu_step_mode, wgpu::VertexStepMode, abi::VertexStepMode, {
    Vertex, Instance,
});

plain_map!(topology, wgpu_topology, wgpu::PrimitiveTopology, abi::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(index_format, wgpu_index_format, wgpu::IndexFormat, abi::IndexFormat, {
    Uint16, Uint32,
});

plain_map!(front_face, wgpu_front_face, wgpu::FrontFace, abi::FrontFace, { Ccw, Cw });

plain_map!(face, wgpu_face, wgpu::Face, abi::Face, { Front, Back });

plain_map!(polygon_mode, wgpu_polygon_mode, wgpu::PolygonMode, abi::PolygonMode, {
    Fill, Line, Point,
});

plain_map!(blend_factor, wgpu_blend_factor, wgpu::BlendFactor, abi::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
});

plain_map!(blend_operation, wgpu_blend_operation, wgpu::BlendOperation, abi::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(stencil_operation, wgpu_stencil_operation, wgpu::StencilOperation, abi::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(sampler_binding, wgpu_sampler_binding, wgpu::SamplerBindingType, abi::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(storage_access, wgpu_storage_access, wgpu::StorageTextureAccess, abi::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(store_op, wgpu_store_op, wgpu::StoreOp, abi::StoreOp, { Store, Discard });

#[allow(unreachable_patterns)]
pub(crate) fn mipmap_filter_mode(value: wgpu::MipmapFilterMode) -> abi::FilterMode {
    match value {
        wgpu::MipmapFilterMode::Nearest => abi::FilterMode::Nearest,
        wgpu::MipmapFilterMode::Linear => abi::FilterMode::Linear,
        other => unsupported("mipmap filter", &other),
    }
}

pub(crate) fn label(value: Option<&str>) -> String {
    value.unwrap_or_default().to_owned()
}

pub(crate) fn extent(value: wgpu::Extent3d) -> abi::Extent3d {
    abi::Extent3d {
        width: value.width,
        height: value.height,
        depth_or_array_layers: value.depth_or_array_layers,
    }
}

pub(crate) fn size(value: Option<wgpu::BufferSize>) -> u64 {
    value.map_or(abi::WHOLE_SIZE, |size| size.get())
}

pub(crate) fn binding_type(value: wgpu::BindingType) -> abi::BindingType {
    match value {
        wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset,
            min_binding_size,
        } => abi::BindingType::Buffer {
            kind: match ty {
                wgpu::BufferBindingType::Uniform => abi::BufferBindingType::Uniform,
                wgpu::BufferBindingType::Storage { read_only: false } => {
                    abi::BufferBindingType::Storage
                }
                wgpu::BufferBindingType::Storage { read_only: true } => {
                    abi::BufferBindingType::ReadOnlyStorage
                }
            },
            has_dynamic_offset,
            min_binding_size: size(min_binding_size),
        },
        wgpu::BindingType::Sampler(kind) => abi::BindingType::Sampler(sampler_binding(kind)),
        wgpu::BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled,
        } => abi::BindingType::Texture {
            sample_kind: match sample_type {
                wgpu::TextureSampleType::Float { filterable: true } => {
                    abi::TextureSampleKind::FloatFilterable
                }
                wgpu::TextureSampleType::Float { filterable: false } => {
                    abi::TextureSampleKind::FloatUnfilterable
                }
                wgpu::TextureSampleType::Depth => abi::TextureSampleKind::Depth,
                wgpu::TextureSampleType::Sint => abi::TextureSampleKind::Sint,
                wgpu::TextureSampleType::Uint => abi::TextureSampleKind::Uint,
            },
            view_dimension: texture_view_dimension(view_dimension),
            multisampled,
        },
        wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension,
        } => abi::BindingType::StorageTexture {
            access: storage_access(access),
            format: texture_format(format),
            view_dimension: texture_view_dimension(view_dimension),
        },
        other => unsupported("binding type", &other),
    }
}

pub(crate) fn binding_resource(value: &wgpu::BindingResource<'_>) -> abi::BindingResource {
    match value {
        wgpu::BindingResource::Buffer(binding) => abi::BindingResource::Buffer {
            buffer: handle_of(binding.buffer.as_custom::<Buffer>()),
            offset: binding.offset,
            size: size(binding.size),
        },
        wgpu::BindingResource::Sampler(sampler) => {
            abi::BindingResource::Sampler(handle_of(sampler.as_custom::<Sampler>()))
        }
        wgpu::BindingResource::TextureView(view) => {
            abi::BindingResource::TextureView(handle_of(view.as_custom::<TextureView>()))
        }
        other => unsupported("binding resource", &other),
    }
}

pub(crate) trait Handled {
    fn handle(&self) -> abi::Handle;
}

fn handle_of<T: Handled>(value: Option<&T>) -> abi::Handle {
    match value {
        Some(value) => value.handle(),
        None => panic!("a resource from another wgpu backend reached the plugin gpu abi"),
    }
}

pub(crate) fn texture_handle(value: &wgpu::Texture) -> abi::Handle {
    handle_of(value.as_custom::<Texture>())
}

pub(crate) fn view_handle(value: &wgpu::TextureView) -> abi::Handle {
    handle_of(value.as_custom::<TextureView>())
}

pub(crate) fn layout_handle(value: &wgpu::PipelineLayout) -> abi::Handle {
    handle_of(value.as_custom::<PipelineLayout>())
}

pub(crate) fn group_layout_handle(value: &wgpu::BindGroupLayout) -> abi::Handle {
    handle_of(value.as_custom::<BindGroupLayout>())
}

pub(crate) fn module_handle(value: &wgpu::ShaderModule) -> abi::Handle {
    handle_of(value.as_custom::<ShaderModule>())
}

pub(crate) fn blend_component(value: wgpu::BlendComponent) -> abi::BlendComponent {
    abi::BlendComponent {
        src_factor: blend_factor(value.src_factor),
        dst_factor: blend_factor(value.dst_factor),
        operation: blend_operation(value.operation),
    }
}

pub(crate) fn stencil_face(value: wgpu::StencilFaceState) -> abi::StencilFaceState {
    abi::StencilFaceState {
        compare: compare_function(value.compare),
        fail_op: stencil_operation(value.fail_op),
        depth_fail_op: stencil_operation(value.depth_fail_op),
        pass_op: stencil_operation(value.pass_op),
    }
}

pub(crate) fn color(value: wgpu::Color) -> abi::Color {
    abi::Color {
        red: value.r,
        green: value.g,
        blue: value.b,
        alpha: value.a,
    }
}

use serde::{Deserialize, Serialize};

macro_rules! plain_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }
    };
}

plain_enum!(TextureFormat {
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgb9e5Ufloat,
    Rgb10a2Uint,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
    Stencil8,
    Depth16Unorm,
    Depth24Plus,
    Depth24PlusStencil8,
    Depth32Float,
    Depth32FloatStencil8,
});

plain_enum!(TextureDimension { D1, D2, D3 });

plain_enum!(TextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
});

plain_enum!(TextureAspect {
    All,
    StencilOnly,
    DepthOnly,
});

plain_enum!(AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
});

plain_enum!(FilterMode { Nearest, Linear });

plain_enum!(CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
});

plain_enum!(SamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    Zero,
});

plain_enum!(VertexFormat {
    Uint8x2,
    Uint8x4,
    Sint8x2,
    Sint8x4,
    Unorm8x2,
    Unorm8x4,
    Snorm8x2,
    Snorm8x4,
    Uint16x2,
    Uint16x4,
    Sint16x2,
    Sint16x4,
    Unorm16x2,
    Unorm16x4,
    Snorm16x2,
    Snorm16x4,
    Float16x2,
    Float16x4,
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
});

plain_enum!(VertexStepMode { Vertex, Instance });

plain_enum!(PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
});

plain_enum!(IndexFormat { Uint16, Uint32 });

plain_enum!(FrontFace { Ccw, Cw });

plain_enum!(Face { Front, Back });

plain_enum!(PolygonMode { Fill, Line, Point });

plain_enum!(BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
});

plain_enum!(BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
});

plain_enum!(StencilOperation {
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
});

plain_enum!(BufferBindingType {
    Uniform,
    Storage,
    ReadOnlyStorage,
});

plain_enum!(SamplerBindingType {
    Filtering,
    NonFiltering,
    Comparison,
});

plain_enum!(TextureSampleKind {
    FloatFilterable,
    FloatUnfilterable,
    Depth,
    Sint,
    Uint,
});

plain_enum!(StorageTextureAccess {
    WriteOnly,
    ReadOnly,
    ReadWrite,
});

plain_enum!(StoreOp { Store, Discard });

mod descriptors;
mod enums;

#[cfg(test)]
mod tests;

pub use descriptors::*;
pub use enums::*;

use serde::{de::DeserializeOwned, Serialize};

pub const GPU_MODULE: &str = "be3_gpu";
pub const HOST_MODULE: &str = "be3_host";

pub const ABI_VERSION: u32 = 1;

pub type Handle = u32;

pub const NULL_HANDLE: Handle = 0;

pub const WHOLE_SIZE: u64 = 0;

pub const NO_MESSAGE: i64 = -1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum ResourceKind {
    Buffer,
    Texture,
    TextureView,
    Sampler,
    BindGroupLayout,
    BindGroup,
    PipelineLayout,
    ShaderModule,
    RenderPipeline,
    CommandEncoder,
    CommandBuffer,
    RenderPass,
}

impl ResourceKind {
    pub fn code(self) -> u32 {
        match self {
            Self::Buffer => 1,
            Self::Texture => 2,
            Self::TextureView => 3,
            Self::Sampler => 4,
            Self::BindGroupLayout => 5,
            Self::BindGroup => 6,
            Self::PipelineLayout => 7,
            Self::ShaderModule => 8,
            Self::RenderPipeline => 9,
            Self::CommandEncoder => 10,
            Self::CommandBuffer => 11,
            Self::RenderPass => 12,
        }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        let kind = match code {
            1 => Self::Buffer,
            2 => Self::Texture,
            3 => Self::TextureView,
            4 => Self::Sampler,
            5 => Self::BindGroupLayout,
            6 => Self::BindGroup,
            7 => Self::PipelineLayout,
            8 => Self::ShaderModule,
            9 => Self::RenderPipeline,
            10 => Self::CommandEncoder,
            11 => Self::CommandBuffer,
            12 => Self::RenderPass,
            _ => return None,
        };
        Some(kind)
    }
}

pub fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).unwrap_or_default()
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    bincode::deserialize(bytes).map_err(|error| error.to_string())
}

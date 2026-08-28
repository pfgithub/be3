mod capture;
mod compare;
mod format;
mod raster;

pub use capture::{capture, TextureStore};
pub use compare::{difference, Difference};
pub use format::{Content, Frame, Primitive, Snapshot, Texture, TextureKey, Triangle, Vertex};
pub use raster::render;

#[cfg(test)]
mod tests;

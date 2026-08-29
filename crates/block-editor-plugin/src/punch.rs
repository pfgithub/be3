pub(crate) const SLOTS: u32 = 256;
pub(crate) const BYTES: u64 = 32;
pub(crate) const SHADER: &str = include_str!("punch.wgsl");

#[cfg(test)]
mod tests;

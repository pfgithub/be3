mod convert;
mod imports;
mod objects;

use block_gpu_abi as abi;

pub fn device_and_queue() -> (wgpu::Device, wgpu::Queue) {
    (
        wgpu::Device::from_custom(objects::Device::new()),
        wgpu::Queue::from_custom(objects::Queue),
    )
}

pub fn acquire_surface_texture(surface: u32) -> Result<wgpu::Texture, String> {
    let handle = unsafe { imports::surface_acquire(surface) };
    if handle == abi::NULL_HANDLE {
        return Err(host_error());
    }
    let descriptor = describe(handle)?;
    let descriptor = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: descriptor.size.width,
            height: descriptor.size.height,
            depth_or_array_layers: descriptor.size.depth_or_array_layers,
        },
        mip_level_count: descriptor.mip_level_count,
        sample_count: descriptor.sample_count,
        dimension: convert::wgpu_texture_dimension(descriptor.dimension),
        format: convert::wgpu_texture_format(descriptor.format),
        usage: wgpu::TextureUsages::from_bits_truncate(descriptor.usage),
        view_formats: &[],
    };
    Ok(wgpu::Texture::from_custom(
        objects::Texture { handle },
        &descriptor,
    ))
}

pub fn present_surface(surface: u32) {
    unsafe { imports::surface_present(surface) };
}

fn describe(texture: abi::Handle) -> Result<abi::TextureDescriptor, String> {
    let mut buffer = vec![0u8; 512];
    let needed = unsafe {
        imports::texture_describe(texture, buffer.as_mut_ptr() as u32, buffer.len() as u32)
    };
    if needed == 0 {
        return Err(host_error());
    }
    if needed as usize > buffer.len() {
        buffer = vec![0u8; needed as usize];
        unsafe {
            imports::texture_describe(texture, buffer.as_mut_ptr() as u32, buffer.len() as u32)
        };
    }
    buffer.truncate(needed as usize);
    abi::decode(&buffer)
}

fn host_error() -> String {
    let mut buffer = vec![0u8; 256];
    let needed = unsafe { imports::error_take(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed as usize > buffer.len() {
        buffer = vec![0u8; needed as usize];
        unsafe { imports::error_take(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    }
    buffer.truncate(needed as usize);
    String::from_utf8(buffer).unwrap_or_else(|_| "the host reported an unreadable error".into())
}

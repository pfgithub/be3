use super::*;

#[test]
fn unsupported_backend_is_rejected() {
    assert_eq!(
        LinuxSurfaceDescriptor::supports_backend(LinuxGraphicsBackend::Other),
        Err(LinuxSurfaceError::UnsupportedBackend)
    );
    assert_eq!(
        LinuxSurfaceDescriptor::supports_backend(LinuxGraphicsBackend::Vulkan),
        Ok(())
    );
    assert_eq!(
        LinuxSurfaceDescriptor::supports_backend(LinuxGraphicsBackend::Gl),
        Ok(())
    );
}

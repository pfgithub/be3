use crate::{
    desktop_attachments::UnixAttachmentCarrier, AlphaMode, AttachmentDescriptor,
    AttachmentOwnership, AttachmentType, ColorFormat, ColorSpace, Message, SurfaceDescriptor,
    SurfaceMechanism, SurfaceRole,
};
use std::{fs::File, os::fd::AsRawFd, os::unix::net::UnixStream, thread};

#[test]
fn unix_carrier_transfers_close_on_exec_descriptor() {
    let (left, right) = UnixStream::pair().unwrap();
    let file = File::open("/dev/null").unwrap();
    let message = Message::Surface(SurfaceDescriptor {
        request_id: 1,
        generation: 2,
        role: SurfaceRole::Screens,
        mechanism: SurfaceMechanism::LinuxDmaBuf,
        width: 1,
        height: 1,
        format: ColorFormat::Rgba8Srgb,
        color_space: ColorSpace::Srgb,
        alpha_mode: AlphaMode::Opaque,
        attachments: vec![AttachmentDescriptor {
            attachment_type: AttachmentType::Image,
            ownership: AttachmentOwnership::Transferred,
        }],
        opaque: Vec::new(),
    });
    let sender = thread::spawn(move || {
        UnixAttachmentCarrier::new(left)
            .send(&message, &[file.as_raw_fd()])
            .unwrap();
        message
    });
    let (received, descriptors) = UnixAttachmentCarrier::new(right).receive().unwrap();
    assert_eq!(received, sender.join().unwrap());
    assert_eq!(descriptors.len(), 1);
    let flags = unsafe { libc::fcntl(descriptors[0].as_raw_fd(), libc::F_GETFD) };
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
}

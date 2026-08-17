use crate::{decode_frame, encode_frame, AttachmentError, Message, MAX_FRAME_BYTES};
use std::{fmt, io};

#[derive(Debug)]
pub enum CarrierError {
    Io(io::Error),
    Protocol,
    Attachments(AttachmentError),
    MissingManifest,
}

impl fmt::Display for CarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CarrierError {}

impl From<io::Error> for CarrierError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<AttachmentError> for CarrierError {
    fn from(value: AttachmentError) -> Self {
        Self::Attachments(value)
    }
}

fn descriptors(message: &Message) -> &[crate::AttachmentDescriptor] {
    match message {
        Message::Surface(surface) => &surface.attachments,
        _ => &[],
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use crate::{validate_attachments, AttachmentType, MAX_ATTACHMENTS};
    use std::{
        io::{Read, Write},
        mem,
        os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        os::unix::net::UnixStream,
    };

    pub struct UnixAttachmentCarrier {
        stream: UnixStream,
    }

    impl UnixAttachmentCarrier {
        pub fn new(stream: UnixStream) -> Self {
            Self { stream }
        }

        pub fn send(
            &mut self,
            message: &Message,
            attachments: &[RawFd],
        ) -> Result<(), CarrierError> {
            let expected = descriptors(message);
            validate_attachments(
                expected,
                &expected
                    .iter()
                    .map(|value| value.attachment_type)
                    .collect::<Vec<_>>(),
            )?;
            if expected.len() != attachments.len() {
                return Err(AttachmentError::CountMismatch {
                    expected: expected.len(),
                    received: attachments.len(),
                }
                .into());
            }
            let frame = encode_frame(message).map_err(|_| CarrierError::Protocol)?;
            let sent = send_with_descriptors(self.stream.as_raw_fd(), &frame, attachments)?;
            self.stream.write_all(&frame[sent..])?;
            Ok(())
        }

        pub fn receive(&mut self) -> Result<(Message, Vec<OwnedFd>), CarrierError> {
            let mut header = [0; 4];
            let attachments = receive_header(self.stream.as_raw_fd(), &mut header)?;
            let length = u32::from_be_bytes(header) as usize;
            if length > MAX_FRAME_BYTES {
                return Err(CarrierError::Protocol);
            }
            let mut frame = Vec::with_capacity(length + 4);
            frame.extend_from_slice(&header);
            frame.resize(length + 4, 0);
            self.stream.read_exact(&mut frame[4..])?;
            let message = decode_frame(&frame).map_err(|_| CarrierError::Protocol)?;
            let expected = descriptors(&message);
            let received = expected
                .iter()
                .take(attachments.len())
                .map(|value| value.attachment_type)
                .chain(std::iter::repeat(AttachmentType::Image))
                .take(attachments.len())
                .collect::<Vec<_>>();
            validate_attachments(expected, &received)?;
            Ok((message, attachments))
        }

        pub fn into_inner(self) -> UnixStream {
            self.stream
        }
    }

    fn send_with_descriptors(fd: RawFd, frame: &[u8], attachments: &[RawFd]) -> io::Result<usize> {
        let mut io = libc::iovec {
            iov_base: frame.as_ptr().cast_mut().cast(),
            iov_len: frame.len(),
        };
        let bytes = std::mem::size_of_val(attachments);
        let space = unsafe { libc::CMSG_SPACE(bytes as u32) as usize };
        let mut control = vec![0_u8; space];
        let mut message: libc::msghdr = unsafe { mem::zeroed() };
        message.msg_iov = &raw mut io;
        message.msg_iovlen = 1;
        if !attachments.is_empty() {
            message.msg_control = control.as_mut_ptr().cast();
            message.msg_controllen = control.len() as _;
            let header = unsafe { libc::CMSG_FIRSTHDR(&message) };
            unsafe {
                (*header).cmsg_level = libc::SOL_SOCKET;
                (*header).cmsg_type = libc::SCM_RIGHTS;
                (*header).cmsg_len = libc::CMSG_LEN(bytes as u32) as _;
                std::ptr::copy_nonoverlapping(
                    attachments.as_ptr(),
                    libc::CMSG_DATA(header).cast(),
                    attachments.len(),
                );
            }
        }
        let sent = unsafe { libc::sendmsg(fd, &message, libc::MSG_NOSIGNAL) };
        if sent < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(sent as usize)
        }
    }

    fn receive_header(fd: RawFd, header: &mut [u8; 4]) -> io::Result<Vec<OwnedFd>> {
        let mut io = libc::iovec {
            iov_base: header.as_mut_ptr().cast(),
            iov_len: header.len(),
        };
        let space = unsafe {
            libc::CMSG_SPACE((MAX_ATTACHMENTS * mem::size_of::<RawFd>()) as u32) as usize
        };
        let mut control = vec![0_u8; space];
        let mut message: libc::msghdr = unsafe { mem::zeroed() };
        message.msg_iov = &raw mut io;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = control.len() as _;
        #[cfg(target_vendor = "apple")]
        let flags = 0;
        #[cfg(not(target_vendor = "apple"))]
        let flags = libc::MSG_CMSG_CLOEXEC;
        let received = unsafe { libc::recvmsg(fd, &raw mut message, flags) };
        if received < 0 {
            return Err(io::Error::last_os_error());
        }
        if received == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "plugin disconnected",
            ));
        }
        if message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated attachment frame",
            ));
        }
        if received != header.len() as isize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "split attachment header",
            ));
        }
        let mut result = Vec::new();
        let mut control_header = unsafe { libc::CMSG_FIRSTHDR(&message) };
        while !control_header.is_null() {
            let value = unsafe { &*control_header };
            if value.cmsg_level != libc::SOL_SOCKET || value.cmsg_type != libc::SCM_RIGHTS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected attachment type",
                ));
            }
            let data_length = value.cmsg_len as usize - unsafe { libc::CMSG_LEN(0) as usize };
            if !data_length.is_multiple_of(mem::size_of::<RawFd>()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed descriptors",
                ));
            }
            let count = data_length / mem::size_of::<RawFd>();
            if result.len() + count > MAX_ATTACHMENTS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "too many descriptors",
                ));
            }
            let descriptors = unsafe {
                std::slice::from_raw_parts(libc::CMSG_DATA(control_header).cast::<RawFd>(), count)
            };
            result.extend(descriptors.iter().map(|fd| {
                #[cfg(target_vendor = "apple")]
                unsafe {
                    libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
                unsafe { OwnedFd::from_raw_fd(*fd) }
            }));
            control_header = unsafe { libc::CMSG_NXTHDR(&message, control_header) };
        }
        Ok(result)
    }
}

#[cfg(unix)]
pub use unix::UnixAttachmentCarrier;

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::{validate_attachments, AttachmentType, MAX_ATTACHMENTS};
    use std::{
        fs::File,
        io::{Read, Write},
        os::windows::io::{FromRawHandle, OwnedHandle, RawHandle},
    };
    use windows_sys::Win32::{
        Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE},
        System::Threading::GetCurrentProcess,
    };

    pub struct WindowsAttachmentCarrier {
        stream: File,
        peer_process: HANDLE,
    }

    impl WindowsAttachmentCarrier {
        pub fn new(stream: File, peer_process: HANDLE) -> Self {
            Self {
                stream,
                peer_process,
            }
        }

        pub fn send(
            &mut self,
            message: &Message,
            attachments: &[RawHandle],
        ) -> Result<(), CarrierError> {
            let expected = descriptors(message);
            if expected.len() > MAX_ATTACHMENTS {
                return Err(AttachmentError::TooMany {
                    count: expected.len(),
                    maximum: MAX_ATTACHMENTS,
                }
                .into());
            }
            if expected.len() != attachments.len() {
                return Err(AttachmentError::CountMismatch {
                    expected: expected.len(),
                    received: attachments.len(),
                }
                .into());
            }
            let mut duplicated = Vec::with_capacity(attachments.len());
            for attachment in attachments {
                let mut target = std::ptr::null_mut();
                let succeeded = unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        *attachment as HANDLE,
                        self.peer_process,
                        &raw mut target,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                };
                if succeeded == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                duplicated.push(target as usize as u64);
            }
            let frame = encode_frame(message).map_err(|_| CarrierError::Protocol)?;
            self.stream
                .write_all(&(duplicated.len() as u32).to_be_bytes())?;
            for handle in duplicated {
                self.stream.write_all(&handle.to_be_bytes())?;
            }
            self.stream.write_all(&frame)?;
            Ok(())
        }

        pub fn receive(&mut self) -> Result<(Message, Vec<OwnedHandle>), CarrierError> {
            let mut count = [0; 4];
            self.stream.read_exact(&mut count)?;
            let count = u32::from_be_bytes(count) as usize;
            if count > MAX_ATTACHMENTS {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too many handles").into());
            }
            let mut attachments = Vec::with_capacity(count);
            for _ in 0..count {
                let mut encoded = [0; 8];
                self.stream.read_exact(&mut encoded)?;
                let handle = u64::from_be_bytes(encoded) as usize as RawHandle;
                if handle.is_null() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid handle").into());
                }
                attachments.push(unsafe { OwnedHandle::from_raw_handle(handle) });
            }
            let mut header = [0; 4];
            self.stream.read_exact(&mut header)?;
            let length = u32::from_be_bytes(header) as usize;
            if length > MAX_FRAME_BYTES {
                return Err(CarrierError::Protocol);
            }
            let mut frame = Vec::with_capacity(length + 4);
            frame.extend_from_slice(&header);
            frame.resize(length + 4, 0);
            self.stream.read_exact(&mut frame[4..])?;
            let message = decode_frame(&frame).map_err(|_| CarrierError::Protocol)?;
            let expected = descriptors(&message);
            let received = expected
                .iter()
                .take(attachments.len())
                .map(|value| value.attachment_type)
                .chain(std::iter::repeat(AttachmentType::Image))
                .take(attachments.len())
                .collect::<Vec<_>>();
            validate_attachments(expected, &received)?;
            Ok((message, attachments))
        }

        pub fn into_inner(self) -> File {
            self.stream
        }
    }
}

#[cfg(windows)]
pub use windows::WindowsAttachmentCarrier;

#[cfg(test)]
mod tests;

use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, RawHandle},
    },
};

use block_plugin_api::{desktop_attachments::WindowsAttachmentCarrier, Message};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING},
    System::{
        Pipes::GetNamedPipeServerProcessId,
        Threading::{OpenProcess, PROCESS_DUP_HANDLE},
    },
};

pub(crate) type Attachment = RawHandle;

pub(crate) fn connect(endpoint: &str) -> io::Result<Connection> {
    Ok(Connection {
        reader: open(&format!("{endpoint}-to-plugin"), FILE_GENERIC_READ)?,
        writer: open(&format!("{endpoint}-to-host"), FILE_GENERIC_WRITE)?,
    })
}

pub(crate) struct Connection {
    reader: File,
    writer: File,
}

impl Connection {
    pub(crate) fn reader(&mut self) -> &mut impl io::Read {
        &mut self.reader
    }

    pub(crate) fn writer(&mut self) -> &mut impl io::Write {
        &mut self.writer
    }

    /// Splits the connection, opening the host process the surface handles
    /// are duplicated into. A plugin may only do that because the host is the
    /// server end of the pipe it is already connected to.
    pub(crate) fn split(self) -> io::Result<(Reader, Sender)> {
        let mut host_pid = 0;
        if unsafe {
            GetNamedPipeServerProcessId(self.writer.as_raw_handle().cast(), &raw mut host_pid)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let host = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, host_pid) };
        if host.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok((
            Reader(WindowsAttachmentCarrier::receiving(self.reader)),
            Sender {
                carrier: WindowsAttachmentCarrier::new(self.writer, host),
                host,
            },
        ))
    }
}

pub(crate) struct Reader(WindowsAttachmentCarrier);

impl Reader {
    pub(crate) fn receive(&mut self) -> Result<Message, String> {
        self.0
            .receive()
            .map(|(message, _)| message)
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct Sender {
    carrier: WindowsAttachmentCarrier,
    host: HANDLE,
}

impl Sender {
    pub(crate) fn send(
        &mut self,
        message: &Message,
        attachments: &[Attachment],
    ) -> Result<(), String> {
        self.carrier
            .send(message, attachments)
            .map_err(|error| error.to_string())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.host) };
    }
}

fn open(name: &str, access: u32) -> io::Result<File> {
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle.cast()) })
    }
}

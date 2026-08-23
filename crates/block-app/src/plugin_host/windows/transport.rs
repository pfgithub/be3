use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    process::{Child, Command},
};

use block_plugin_api::{
    desktop_attachments::{CarrierError, WindowsAttachmentCarrier},
    Message, PluginManifest, SurfaceMechanism,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
    },
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
        PIPE_TYPE_BYTE, PIPE_WAIT,
    },
};

use crate::plugin_host::process::{Reading, Writing};

pub(crate) const SURFACE_MECHANISM: SurfaceMechanism = SurfaceMechanism::WindowsDxgi;

pub(crate) type Attachment = OwnedHandle;
pub(crate) type Receiver = WindowsAttachmentCarrier;
pub(crate) type Sender = WindowsAttachmentCarrier;

const PIPE_BUFFER_BYTES: u32 = 1_048_580;
const CONNECT_TIMEOUT_MILLISECONDS: u32 = 5_000;

pub(crate) fn entry_point(plugin: &PluginManifest) -> PathBuf {
    let entry = plugin.entry_points.windows.as_deref().unwrap_or_default();
    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry).unwrap_or_default()
}

pub(crate) fn prepare(command: &mut Command, executable: &Path) {
    if let Some(directory) = executable.parent() {
        command.current_dir(directory).env_remove("PATH");
    }
}

pub(crate) struct Endpoint {
    inbound: HANDLE,
    outbound: HANDLE,
    name: String,
}

impl Endpoint {
    pub(crate) fn create() -> io::Result<Self> {
        let name = format!(r"\\.\pipe\be3-plugin-{}-{}", std::process::id(), unique());
        let mut endpoint = Self {
            inbound: INVALID_HANDLE_VALUE,
            outbound: INVALID_HANDLE_VALUE,
            name,
        };
        endpoint.inbound = pipe(&to_host(&endpoint.name), PIPE_ACCESS_INBOUND)?;
        endpoint.outbound = pipe(&to_plugin(&endpoint.name), PIPE_ACCESS_OUTBOUND)?;
        Ok(endpoint)
    }

    pub(crate) fn argument(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn accept(mut self, child: &Child) -> io::Result<Connection> {
        connect(self.inbound, child)?;
        connect(self.outbound, child)?;
        let inbound = std::mem::replace(&mut self.inbound, INVALID_HANDLE_VALUE);
        let outbound = std::mem::replace(&mut self.outbound, INVALID_HANDLE_VALUE);
        Ok(unsafe {
            Connection {
                reader: File::from_raw_handle(inbound.cast()),
                writer: File::from_raw_handle(outbound.cast()),
            }
        })
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        for handle in [self.inbound, self.outbound] {
            if handle != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(handle) };
            }
        }
    }
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

    pub(crate) fn split(self, child: &Child) -> io::Result<(Receiver, Sender)> {
        let peer: HANDLE = child.as_raw_handle().cast();
        Ok((
            WindowsAttachmentCarrier::receiving(self.reader),
            WindowsAttachmentCarrier::new(self.writer, peer),
        ))
    }
}

impl Reading for WindowsAttachmentCarrier {
    fn read(&mut self) -> Result<(Message, Vec<Attachment>), CarrierError> {
        self.receive()
    }
}

impl Writing for WindowsAttachmentCarrier {
    fn write(&mut self, message: &Message) -> Result<(), CarrierError> {
        self.send(message, &[])
    }
}

fn pipe(name: &str, access: u32) -> io::Result<HANDLE> {
    let wide = wide(name);
    let handle = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            access | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            CONNECT_TIMEOUT_MILLISECONDS,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(handle)
    }
}

fn connect(handle: HANDLE, child: &Child) -> io::Result<()> {
    let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
    if connected == 0
        && io::Error::last_os_error().raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32)
    {
        return Err(io::Error::last_os_error());
    }
    let mut process_id = 0;
    if unsafe { GetNamedPipeClientProcessId(handle, &raw mut process_id) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if process_id != child.id() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unexpected plugin peer",
        ));
    }
    Ok(())
}

fn to_host(endpoint: &str) -> String {
    format!("{endpoint}-to-host")
}

fn to_plugin(endpoint: &str) -> String {
    format!("{endpoint}-to-plugin")
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn unique() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

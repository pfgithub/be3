use std::{
    fs, io,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command},
};

use block_plugin_api::{
    desktop_attachments::{CarrierError, UnixAttachmentCarrier},
    Message, PluginManifest, SurfaceMechanism,
};

use crate::plugin_host::process::{Reading, Writing};

pub(crate) const SURFACE_MECHANISM: SurfaceMechanism = SurfaceMechanism::LinuxDmaBuf;

pub(crate) type Attachment = OwnedFd;
pub(crate) type Receiver = UnixAttachmentCarrier;
pub(crate) type Sender = UnixAttachmentCarrier;

pub(crate) fn entry_point(plugin: &PluginManifest) -> PathBuf {
    let entry = plugin.entry_points.linux.as_deref().unwrap_or_default();
    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry).unwrap_or_default()
}

pub(crate) fn prepare(_command: &mut Command, _executable: &Path) {}

pub(crate) struct Endpoint {
    listener: UnixListener,
    path: PathBuf,
}

impl Endpoint {
    pub(crate) fn create() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "be3-plugin-{}-{}.sock",
            std::process::id(),
            unique()
        ));
        let listener = UnixListener::bind(&path)?;
        Ok(Self { listener, path })
    }

    pub(crate) fn argument(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    pub(crate) fn accept(self, child: &Child) -> io::Result<Connection> {
        let (stream, _) = self.listener.accept()?;
        verify_peer(&stream, child)?;
        Ok(Connection { stream })
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

pub(crate) struct Connection {
    stream: UnixStream,
}

impl Connection {
    pub(crate) fn reader(&mut self) -> &mut impl io::Read {
        &mut self.stream
    }

    pub(crate) fn writer(&mut self) -> &mut impl io::Write {
        &mut self.stream
    }

    pub(crate) fn split(self, _child: &Child) -> io::Result<(Receiver, Sender)> {
        let reader = self.stream.try_clone()?;
        Ok((
            UnixAttachmentCarrier::new(reader),
            UnixAttachmentCarrier::new(self.stream),
        ))
    }
}

impl Reading for UnixAttachmentCarrier {
    fn read(&mut self) -> Result<(Message, Vec<Attachment>), CarrierError> {
        self.receive()
    }
}

impl Writing for UnixAttachmentCarrier {
    fn write(&mut self, message: &Message) -> Result<(), CarrierError> {
        self.send(message, &[])
    }
}

fn verify_peer(stream: &UnixStream, child: &Child) -> io::Result<()> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut credentials).cast(),
            &raw mut length,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    if credentials.pid as u32 != child.id() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unexpected plugin peer",
        ));
    }
    Ok(())
}

fn unique() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

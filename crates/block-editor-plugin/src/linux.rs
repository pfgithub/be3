mod surface;
mod transport;

pub(crate) use surface::{Surface, SURFACE_KIND};

pub(crate) const SURFACE_MECHANISM: block_plugin_api::SurfaceMechanism =
    block_plugin_api::SurfaceMechanism::LinuxDmaBuf;
pub(crate) use transport::{connect, Attachment, Connection, Reader};

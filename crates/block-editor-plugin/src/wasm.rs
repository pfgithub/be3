mod host;
mod surface;
mod transport;

pub(crate) use surface::{Surface, SURFACE_KIND};

pub(crate) const SHARED_PREVIEWS: bool = false;

pub(crate) const SURFACE_MECHANISM: block_plugin_api::SurfaceMechanism =
    block_plugin_api::SurfaceMechanism::HostTexture;

pub(crate) use transport::{initialize_storage, shutdown, start, step, Attachment};

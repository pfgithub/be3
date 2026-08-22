use std::{cell::RefCell, collections::HashMap, sync::Arc};

use block_plugin_api::PluginManifest;
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::load;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub(crate) use android::load;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod native;

#[cfg(target_arch = "wasm32")]
pub(crate) type Location = String;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Location = std::path::PathBuf;

#[derive(Default)]
pub(crate) struct Plugins {
    manifests: Vec<Arc<PluginManifest>>,
    directories: HashMap<String, Location>,
    errors: Vec<String>,
}

impl Plugins {
    pub(crate) fn manifests(&self) -> &[Arc<PluginManifest>] {
        &self.manifests
    }

    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
    }

    fn add(&mut self, source: &str, directory: Location, document: &str) {
        let manifest = match block_plugin_api::manifest_from_json(document) {
            Ok(manifest) => manifest,
            Err(error) => {
                self.errors.push(format!("{source}: {error}"));
                return;
            }
        };
        let block_type = Uuid::from_bytes(manifest.block_type);
        if !block_client::blocks::TYPE_IDS.contains(&block_type) {
            self.errors.push(format!(
                "{source}: {block_type} is not a block type this app has"
            ));
            return;
        }
        if let Some(existing) = self
            .manifests
            .iter()
            .find(|existing| existing.identity.id == manifest.identity.id)
        {
            self.errors.push(format!(
                "{source}: {} is already the id of a plugin that was found first",
                existing.identity.id
            ));
            return;
        }
        if let Some(existing) = self
            .manifests
            .iter()
            .find(|existing| existing.block_type == manifest.block_type)
        {
            self.errors.push(format!(
                "{source}: {} already edits {block_type}",
                existing.identity.id
            ));
            return;
        }
        self.directories
            .insert(manifest.identity.id.clone(), directory);
        self.manifests.push(Arc::new(manifest));
    }

    fn error(&mut self, source: &str, error: impl std::fmt::Display) {
        self.errors.push(format!("{source}: {error}"));
    }
}

thread_local! {
    static PLUGINS: RefCell<Option<Arc<Plugins>>> = const { RefCell::new(None) };
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), not(target_os = "android")),
    allow(dead_code)
)]
fn install(plugins: Plugins) {
    PLUGINS.with(|cell| *cell.borrow_mut() = Some(Arc::new(plugins)));
}

pub(crate) fn plugins() -> Arc<Plugins> {
    PLUGINS.with(|cell| {
        let mut cell = cell.borrow_mut();
        Arc::clone(cell.get_or_insert_with(|| Arc::new(scan())))
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn scan() -> Plugins {
    native::scan()
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn scan() -> Plugins {
    Plugins::default()
}

pub(crate) fn manifests() -> Vec<Arc<PluginManifest>> {
    plugins().manifests.clone()
}

#[cfg_attr(
    not(any(target_arch = "wasm32", target_os = "windows", target_os = "linux")),
    allow(dead_code)
)]
pub(crate) fn entry_point(plugin_id: &str, entry: &str) -> Option<Location> {
    let plugins = plugins();
    let directory = plugins.directories.get(plugin_id)?;
    #[cfg(target_arch = "wasm32")]
    let location = format!("{directory}/{entry}");
    #[cfg(not(target_arch = "wasm32"))]
    let location = directory.join(entry);
    Some(location)
}

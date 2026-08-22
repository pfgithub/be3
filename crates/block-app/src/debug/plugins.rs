use std::cell::RefCell;

use eframe::egui;

thread_local! {
    static OPEN: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) fn open() {
    OPEN.with(|open| *open.borrow_mut() = true);
}

pub(crate) fn show(ctx: &egui::Context) {
    let mut open = OPEN.with(|open| *open.borrow());
    if !open {
        return;
    }
    let mut kill = None;
    egui::Window::new("Plugins")
        .open(&mut open)
        .default_size([420.0, 320.0])
        .resizable(true)
        .show(ctx, |ui| {
            let discovered = crate::editors::plugin::discovery::plugins();
            ui.strong("Discovered");
            if discovered.manifests().is_empty() {
                ui.weak("    no plugins were found");
            }
            for manifest in discovered.manifests() {
                ui.small(format!(
                    "    {} {} — {}",
                    manifest.identity.id,
                    manifest.identity.version,
                    uuid::Uuid::from_bytes(manifest.block_type)
                ));
            }
            for error in discovered.errors() {
                ui.colored_label(egui::Color32::RED, format!("    {error}"));
            }
            ui.separator();
            let running = crate::plugin_host::running();
            if running.is_empty() {
                ui.weak("No editor plugins are running.");
                return;
            }
            for runtime in running {
                ui.horizontal(|ui| {
                    ui.strong(&runtime.plugin_id);
                    ui.weak(format!("surface {}", runtime.surface));
                    ui.weak(&runtime.state);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Kill").clicked() {
                            kill = Some(runtime.plugin_id.clone());
                        }
                    });
                });
                if runtime.instances.is_empty() {
                    ui.weak("    idle");
                }
                for instance in &runtime.instances {
                    let block = instance
                        .block
                        .map_or_else(|| "creating a block".to_owned(), |id| id.to_string());
                    let regions: Vec<_> = instance
                        .regions
                        .iter()
                        .map(|region| format!("{region:?}"))
                        .collect();
                    ui.small(format!(
                        "    {} — {block} [{}]",
                        instance.instance.0,
                        regions.join(", ")
                    ));
                }
                ui.separator();
            }
        });
    if let Some(plugin_id) = kill {
        crate::plugin_host::kill(ctx, &plugin_id);
    }
    OPEN.with(|state| *state.borrow_mut() = open);
}

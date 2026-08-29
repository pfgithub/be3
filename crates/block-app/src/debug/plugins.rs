use std::{cell::RefCell, time::Duration};

use block_plugin_api::{EditorCapabilities, EditorRegion, PluginManifest};
use eframe::egui;

use crate::plugin_host::{InstanceStatus, RuntimeStatus, ScreenStatus};

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
        .default_size([520.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                show_discovered(ui);
                ui.separator();
                kill = show_running(ui);
            });
        });
    if let Some(plugin_id) = kill {
        crate::plugin_host::kill(ctx, &plugin_id);
    }
    OPEN.with(|state| *state.borrow_mut() = open);
}

fn show_discovered(ui: &mut egui::Ui) {
    let discovered = crate::editors::plugin::discovery::plugins();
    ui.strong(format!("Discovered ({})", discovered.manifests().len()));
    if discovered.manifests().is_empty() {
        ui.weak("    no plugins were found");
    }
    for manifest in discovered.manifests() {
        egui::CollapsingHeader::new(format!(
            "{} {} — {}",
            manifest.identity.id,
            manifest.identity.version,
            uuid::Uuid::from_bytes(manifest.block_type)
        ))
        .id_salt(&manifest.identity.id)
        .show(ui, |ui| show_manifest(ui, manifest));
    }
    for error in discovered.errors() {
        ui.colored_label(egui::Color32::RED, format!("    {error}"));
    }
}

fn show_manifest(ui: &mut egui::Ui, manifest: &PluginManifest) {
    ui.small(format!(
        "name {} ({})",
        manifest.display_name, manifest.identity.name
    ));
    ui.small(format!("regions {}", regions(&manifest.regions)));
    ui.small(format!(
        "creation {:?} · interaction {:?} · resize {:?} · important {}",
        manifest.creation, manifest.interaction, manifest.resize, manifest.important
    ));
    ui.small(format!(
        "children add {} · delete {} · replace {}",
        manifest.children.add, manifest.children.delete, manifest.children.replace
    ));
    ui.small(format!(
        "capabilities {}",
        capabilities(&manifest.capabilities)
    ));
    ui.small(format!("entry point {}", manifest.entry_point));
}

fn show_running(ui: &mut egui::Ui) -> Option<String> {
    let running = crate::plugin_host::running();
    ui.strong(format!("Running ({})", running.len()));
    if running.is_empty() {
        ui.weak("    no editor plugins are running");
        return None;
    }
    let mut kill = None;
    for runtime in running {
        if show_runtime(ui, &runtime) {
            kill = Some(runtime.plugin_id.clone());
        }
        ui.separator();
    }
    kill
}

fn show_runtime(ui: &mut egui::Ui, runtime: &RuntimeStatus) -> bool {
    let mut kill = false;
    ui.horizontal(|ui| {
        ui.strong(&runtime.plugin_id);
        ui.weak(&runtime.state);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kill = ui.button("Kill").clicked();
        });
    });
    let surface = &runtime.surface;
    ui.small(format!(
        "    surface {} — {}x{} px, {}, {} placement(s), generation {}",
        surface.index,
        surface.width,
        surface.height,
        bytes(u64::from(surface.width) * u64::from(surface.height) * 4),
        surface.placements,
        surface.generation
    ));
    ui.small(format!(
        "    pass {}{}",
        runtime.pass,
        runtime
            .uptime
            .map(|uptime| format!(" · up {}", elapsed(uptime)))
            .unwrap_or_default()
    ));
    if runtime.instances.is_empty() {
        ui.weak("    idle");
    }
    for instance in &runtime.instances {
        show_instance(ui, instance);
    }
    kill
}

fn show_instance(ui: &mut egui::Ui, instance: &InstanceStatus) {
    let block = instance
        .block
        .map_or_else(|| "creating a block".to_owned(), |id| id.to_string());
    ui.small(format!(
        "    {} — {} {block}{}",
        instance.instance.0,
        instance.role,
        if instance.opened { "" } else { ", opening" }
    ));
    let mut details = Vec::new();
    if let Some(ratio) = instance.aspect_ratio {
        details.push(format!("aspect {ratio:.3}"));
    }
    if let Some(intrinsic) = instance.intrinsic {
        details.push(format!("intrinsic {}", size(intrinsic)));
    }
    if let Some(view) = instance.view {
        details.push(format!(
            "view {:.0},{:.0} {}",
            view.min.x,
            view.min.y,
            size(view.size())
        ));
    }
    if !details.is_empty() {
        ui.small(format!("        {}", details.join(" · ")));
    }
    if let Some(artifact) = &instance.artifact {
        ui.small(format!(
            "        artifact {}{} — {}",
            bytes(artifact.data as u64),
            artifact
                .draft
                .map(|draft| format!(", draft {}", bytes(draft as u64)))
                .unwrap_or_default(),
            artifact.description.as_deref().unwrap_or("undescribed")
        ));
    }
    if instance.screens.is_empty() {
        ui.small("        no screens");
    }
    for screen in &instance.screens {
        show_screen(ui, screen);
    }
}

fn show_screen(ui: &mut egui::Ui, screen: &ScreenStatus) {
    let text = format!(
        "        {:?} #{} — {} pt @ {:.2} ({}x{} px){}{}{}{}",
        screen.region,
        screen.screen.0,
        size(screen.logical),
        screen.scale_factor,
        screen.pixels[0],
        screen.pixels[1],
        screen
            .used
            .map(|used| format!(" · used {} pt", size(used)))
            .unwrap_or_default(),
        screen
            .placement
            .map(|[x, y, width, height]| format!(" · at {x},{y} {width}x{height} px"))
            .unwrap_or_else(|| " · unplaced".to_owned()),
        if screen.drawn { "" } else { " · stale" },
        match screen.children {
            0 => String::new(),
            children => format!(
                " · {children} children at generation {}",
                screen.child_generation
            ),
        }
    );
    if screen.drawn {
        ui.small(text);
    } else {
        ui.small(egui::RichText::new(text).weak());
    }
}

fn regions(regions: &[EditorRegion]) -> String {
    list(regions.iter().map(|region| format!("{region:?}")))
}

fn capabilities(capabilities: &EditorCapabilities) -> String {
    let named = [
        ("rotation", capabilities.rotation),
        ("preserve aspect ratio", capabilities.preserve_aspect_ratio),
        ("pan and zoom", capabilities.pan_and_zoom),
    ];
    list(
        named
            .iter()
            .filter(|(_, held)| *held)
            .map(|(name, _)| (*name).to_owned()),
    )
}

fn list(items: impl Iterator<Item = String>) -> String {
    let items: Vec<_> = items.collect();
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

fn size(size: egui::Vec2) -> String {
    format!("{:.0}x{:.0}", size.x, size.y)
}

fn bytes(count: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut amount = count as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{count} B")
    } else {
        format!("{amount:.1} {}", UNITS[unit])
    }
}

fn elapsed(uptime: Duration) -> String {
    let seconds = uptime.as_secs();
    match (seconds / 3600, (seconds / 60) % 60, seconds % 60) {
        (0, 0, seconds) => format!("{seconds}s"),
        (0, minutes, seconds) => format!("{minutes}m {seconds:02}s"),
        (hours, minutes, _) => format!("{hours}h {minutes:02}m"),
    }
}

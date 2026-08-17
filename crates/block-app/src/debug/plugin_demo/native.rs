use std::{cell::RefCell, path::PathBuf};

use eframe::egui;

thread_local! {
    static WINDOW: RefCell<Window> = RefCell::new(Window::default());
}

pub(crate) fn install(_creation_context: &eframe::CreationContext<'_>) {}

pub(crate) fn open() {
    WINDOW.with(|window| {
        let mut window = window.borrow_mut();
        window.open = true;
        #[cfg(not(target_os = "android"))]
        if window.process.is_none() {
            window.process = Some(super::process::Process::launch(plugin_path()));
        }
    });
}

pub(crate) fn show(ctx: &egui::Context) {
    WINDOW.with(|window| {
        let mut window = window.borrow_mut();
        let mut is_open = window.open;
        if !is_open {
            return;
        }
        egui::Window::new("Plugin Demo")
            .open(&mut is_open)
            .default_size([360.0, 140.0])
            .show(ctx, |ui| {
                #[cfg(target_os = "android")]
                ui.label("Native plugins require the Android service transport.");
                #[cfg(not(target_os = "android"))]
                ui.label(window.process.as_mut().map_or_else(
                    || "Plugin process is not running".to_owned(),
                    super::process::Process::status,
                ));
            });
        if window.open && !is_open {
            #[cfg(not(target_os = "android"))]
            if let Some(mut process) = window.process.take() {
                process.shutdown();
            }
        }
        window.open = is_open;
    });
}

#[derive(Default)]
struct Window {
    open: bool,
    #[cfg(not(target_os = "android"))]
    process: Option<super::process::Process>,
}

#[cfg(not(target_os = "android"))]
fn plugin_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    #[cfg(target_os = "windows")]
    path.set_file_name("plugin-demo/plugin-demo.exe");
    #[cfg(target_os = "macos")]
    path.set_file_name("plugin-demo");
    #[cfg(target_os = "linux")]
    {
        path.pop();
        path.push("../libexec/be3/plugin-demo");
    }
    path
}

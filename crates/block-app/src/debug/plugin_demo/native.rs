use std::cell::RefCell;

#[cfg(not(target_os = "android"))]
use std::path::PathBuf;

use eframe::egui;

thread_local! {
    static WINDOW: RefCell<Window> = RefCell::new(Window::default());
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    #[cfg(target_os = "windows")]
    WINDOW.with(|window| {
        window.borrow_mut().presenter_status = super::windows::install(creation_context);
    });
    #[cfg(not(target_os = "windows"))]
    let _ = creation_context;
}

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
        #[cfg(target_os = "windows")]
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        egui::Window::new("Plugin Demo")
            .open(&mut is_open)
            .default_size([360.0, 140.0])
            .show(ctx, |ui| {
                #[cfg(target_os = "android")]
                window.demo.show(ui);
                #[cfg(all(not(target_os = "android"), not(target_os = "windows")))]
                ui.label(window.process.as_mut().map_or_else(
                    || "Plugin process is not running".to_owned(),
                    super::process::Process::status,
                ));
                #[cfg(target_os = "windows")]
                {
                    use super::presenter::{PresenterCallback, PresenterCommand};
                    if window.presenter_status.is_none() {
                        ui.colored_label(
                            egui::Color32::RED,
                            "Windows plugins require the D3D12 renderer.",
                        );
                    } else {
                        ui.label(window.process.as_mut().map_or_else(
                            || "Plugin process is not running".to_owned(),
                            super::process::Process::status,
                        ));
                        let (response, painter) =
                            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                        let messages = window.input.update(ui, &response, ctx.pixels_per_point());
                        if let Some(process) = &window.process {
                            process.send(messages);
                            if let Some(frame) = process.latest_surface() {
                                window.pending_frame = Some(frame.into());
                            }
                        }
                        if let Some(status) = window.presenter_status.clone() {
                            let frame = window
                                .pending_frame
                                .take()
                                .unwrap_or(super::windows::WindowsFrame::Paint);
                            painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                                response.rect,
                                PresenterCallback {
                                    command: PresenterCommand::Present(frame),
                                    status,
                                },
                            ));
                        }
                    }
                }
            });
        if window.open && !is_open {
            #[cfg(not(target_os = "android"))]
            if let Some(mut process) = window.process.take() {
                process.shutdown();
            }
            #[cfg(target_os = "windows")]
            if let Some(status) = window.presenter_status.clone() {
                use super::presenter::{PresenterCallback, PresenterCommand};
                ctx.debug_painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                        PresenterCallback::<super::windows::WindowsFrame> {
                            command: PresenterCommand::Release,
                            status,
                        },
                    ));
            }
        }
        window.open = is_open;
    });
}

#[derive(Default)]
struct Window {
    open: bool,
    #[cfg(target_os = "android")]
    demo: plugin_demo::demo::Demo,
    #[cfg(not(target_os = "android"))]
    process: Option<super::process::Process>,
    #[cfg(target_os = "windows")]
    input: super::input::InputAdapter,
    #[cfg(target_os = "windows")]
    presenter_status: Option<super::presenter::PresenterStatus>,
    #[cfg(target_os = "windows")]
    pending_frame: Option<super::windows::WindowsFrame>,
}

#[cfg(not(target_os = "android"))]
fn plugin_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    #[cfg(target_os = "windows")]
    path.set_file_name("plugin-demo-host.exe");
    #[cfg(target_os = "macos")]
    path.set_file_name("plugin-demo");
    #[cfg(target_os = "linux")]
    {
        path.pop();
        path.push("../libexec/be3/plugin-demo");
    }
    path
}

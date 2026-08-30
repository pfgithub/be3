use block_client::blocks::image::Image;
use block_plugin_api::ClipboardImage;
use eframe::egui;

pub(super) enum ClipboardImagePasteResult {
    NoImage,
    Image(Image),
    Error(String),
}

#[derive(Default)]
pub(super) struct ClipboardImagePaste {
    shortcut_down: bool,
}

impl ClipboardImagePaste {
    pub(super) fn poll(
        &mut self,
        context: &egui::Context,
        enabled: bool,
    ) -> Option<ClipboardImagePasteResult> {
        let event_paste = context.input(|input| {
            input
                .raw
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Paste(_)))
                || (input.modifiers.command && input.key_pressed(egui::Key::V))
        });
        let shortcut_down = crate::plugin_host::paste_shortcut_down();
        let shortcut_pressed = shortcut_down && !self.shortcut_down;
        self.shortcut_down = shortcut_down;
        if !enabled || (!event_paste && !shortcut_pressed) {
            return None;
        }
        Some(match crate::plugin_host::read_clipboard_image() {
            ClipboardImage::Pasted { name, data } => {
                ClipboardImagePasteResult::Image(Image::new(name, data))
            }
            ClipboardImage::Empty => ClipboardImagePasteResult::NoImage,
            ClipboardImage::Failed(error) => ClipboardImagePasteResult::Error(error),
        })
    }
}

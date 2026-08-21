use block_client::blocks::image::Image;
use eframe::egui;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

#[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
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
        let shortcut_down = paste_shortcut_down();
        let shortcut_pressed = shortcut_down && !self.shortcut_down;
        self.shortcut_down = shortcut_down;
        if !enabled || (!event_paste && !shortcut_pressed) {
            return None;
        }
        Some(read_clipboard_image())
    }
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn read_clipboard_image() -> ClipboardImagePasteResult {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(error) => return ClipboardImagePasteResult::Error(error.to_string()),
    };
    let clipboard_image = match clipboard.get_image() {
        Ok(image) => image,
        Err(arboard::Error::ContentNotAvailable) => return ClipboardImagePasteResult::NoImage,
        Err(error) => return ClipboardImagePasteResult::Error(error.to_string()),
    };
    let mut encoded = Vec::new();
    if let Err(error) = PngEncoder::new(&mut encoded).write_image(
        clipboard_image.bytes.as_ref(),
        clipboard_image.width as u32,
        clipboard_image.height as u32,
        ExtendedColorType::Rgba8,
    ) {
        return ClipboardImagePasteResult::Error(format!("Could not encode pasted image: {error}"));
    }
    ClipboardImagePasteResult::Image(Image::new("Pasted Image.png", encoded))
}

/// Android has no image clipboard, and the browser only exposes one through an
/// asynchronous API that cannot answer a synchronous paste.
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn read_clipboard_image() -> ClipboardImagePasteResult {
    ClipboardImagePasteResult::NoImage
}

#[cfg(target_os = "windows")]
fn paste_shortcut_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};

    const VK_V: i32 = 0x56;
    // SAFETY: GetAsyncKeyState reads process-independent keyboard state and has no pointer or
    // lifetime requirements.
    unsafe { GetAsyncKeyState(VK_CONTROL as i32) < 0 && GetAsyncKeyState(VK_V) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn paste_shortcut_down() -> bool {
    false
}

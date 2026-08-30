use block_plugin_api::ClipboardImage;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
const PASTED_NAME: &str = "Pasted Image.png";

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub(crate) fn read_clipboard_image() -> ClipboardImage {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(error) => return ClipboardImage::Failed(error.to_string()),
    };
    let clipboard_image = match clipboard.get_image() {
        Ok(image) => image,
        Err(arboard::Error::ContentNotAvailable) => return ClipboardImage::Empty,
        Err(error) => return ClipboardImage::Failed(error.to_string()),
    };
    let mut encoded = Vec::new();
    if let Err(error) = PngEncoder::new(&mut encoded).write_image(
        clipboard_image.bytes.as_ref(),
        clipboard_image.width as u32,
        clipboard_image.height as u32,
        ExtendedColorType::Rgba8,
    ) {
        return ClipboardImage::Failed(format!("Could not encode pasted image: {error}"));
    }
    ClipboardImage::Pasted {
        name: PASTED_NAME.to_owned(),
        data: encoded,
    }
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
pub(crate) fn read_clipboard_image() -> ClipboardImage {
    ClipboardImage::Empty
}

#[cfg(target_os = "windows")]
pub(crate) fn paste_shortcut_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};

    const VK_V: i32 = 0x56;

    unsafe { GetAsyncKeyState(VK_CONTROL as i32) < 0 && GetAsyncKeyState(VK_V) < 0 }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn paste_shortcut_down() -> bool {
    false
}

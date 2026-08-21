use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
};

use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

use super::{FileFilter, PickResult, PickedFile};

pub(super) fn open(filter: &FileFilter) -> Receiver<PickResult> {
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = show(filter, sender.clone()) {
        let _ = sender.send(Err(error));
    }
    receiver
}

/// Browsers only open a file chooser for a real `<input type="file">`, so the
/// page grows a hidden one, is clicked for the user, and takes it away again
/// once the choice comes back.
fn show(filter: &FileFilter, sender: Sender<PickResult>) -> Result<(), String> {
    let document = web_sys::window()
        .ok_or("no browser window is available")?
        .document()
        .ok_or("no browser document is available")?;
    let body = document.body().ok_or("the page has no body to attach to")?;
    let input: HtmlInputElement = document
        .create_element("input")
        .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?
        .dyn_into()
        .map_err(|_| "could not open a file picker".to_owned())?;
    input.set_type("file");
    input.set_accept(&accept(filter));
    let _ = input.style().set_property("display", "none");
    body.append_child(&input)
        .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?;

    // Whichever of the two events fires first answers; the other then finds the
    // slot empty and leaves the already answered picker alone.
    let slot = Rc::new(RefCell::new(Some((sender, input.clone()))));
    let chosen = slot.clone();
    let on_change = Closure::<dyn FnMut()>::new(move || {
        let Some((sender, input)) = chosen.borrow_mut().take() else {
            return;
        };
        input.remove();
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            let _ = sender.send(Ok(None));
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            let _ = sender.send(read(file).await);
        });
    });
    let cancelled = slot.clone();
    let on_cancel = Closure::<dyn FnMut()>::new(move || {
        if let Some((sender, input)) = cancelled.borrow_mut().take() {
            input.remove();
            let _ = sender.send(Ok(None));
        }
    });
    for (event, listener) in [("change", &on_change), ("cancel", &on_cancel)] {
        input
            .add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
            .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?;
    }
    // The listeners outlive this call, and the input is thrown away once one of
    // them has run, so there is nothing left to keep them alive here.
    on_change.forget();
    on_cancel.forget();

    input.click();
    Ok(())
}

/// The `accept` attribute takes MIME types and dotted extensions alike, and
/// browsers differ in which they honour, so it is given both.
fn accept(filter: &FileFilter) -> String {
    let mime_types = filter.mime_types.iter().cloned();
    let extensions = filter.extensions.iter().map(|end| format!(".{end}"));
    mime_types.chain(extensions).collect::<Vec<_>>().join(",")
}

async fn read(file: web_sys::File) -> PickResult {
    let name = file.name();
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|error| format!("Could not read {name}: {}", describe(&error)))?;
    let data = js_sys::Uint8Array::new(&buffer).to_vec();
    Ok(Some(PickedFile { name, data }))
}

fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{error:?}"))
}

use std::{io::Read, sync::mpsc, thread};

use block_editor_plugin::Waker;

use super::{Download, Painting};

const LIMIT: u64 = 32 * 1024 * 1024;

pub(super) fn start(waker: Waker) -> Download {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("paint-review-download".into())
        .spawn(move || {
            let _ = sender.send(download());
            waker.wake();
        })
        .expect("failed to start the paint download");
    Download { receiver }
}

fn download() -> Result<Vec<Painting>, String> {
    let tree = get(&super::tree_url())?;
    super::paths_in(&tree)?
        .into_iter()
        .map(|path| {
            let data = get(&super::file_url(&path))?;
            Ok(super::painting(path, data))
        })
        .collect()
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| format!("could not download {url}: {error}"))?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(LIMIT)
        .read_to_end(&mut body)
        .map_err(|error| format!("could not read {url}: {error}"))?;
    Ok(body)
}

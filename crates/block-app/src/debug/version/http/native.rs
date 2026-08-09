use std::{io::Read, sync::mpsc, thread, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(30);

/// A GET request running on a background thread, polled from the UI thread
/// each frame since there is no async runtime borrowed from egui here.
pub(in crate::debug::version) struct Fetch {
    receiver: mpsc::Receiver<Result<Vec<u8>, String>>,
}

impl Fetch {
    pub(in crate::debug::version) fn get(
        url: String,
        headers: Vec<(&'static str, String)>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(run(&url, &headers));
        });
        Self { receiver }
    }

    pub(in crate::debug::version) fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        self.receiver.try_recv().ok()
    }
}

fn run(url: &str, headers: &[(&'static str, String)]) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).build();
    let mut request = agent.get(url);
    for (key, value) in headers {
        request = request.set(key, value);
    }
    let response = match request.call() {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.to_string()),
    };
    let status = response.status();
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}: {}", String::from_utf8_lossy(&body)));
    }
    Ok(body)
}

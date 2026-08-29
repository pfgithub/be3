use std::{io::Read, sync::mpsc, thread, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct Fetch {
    receiver: mpsc::Receiver<Result<Vec<u8>, String>>,
}

impl Fetch {
    pub(crate) fn get(url: String, headers: Vec<(&'static str, String)>) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(run(&url, &headers));
        });
        Self { receiver }
    }

    pub(crate) fn refused(reason: String) -> Self {
        let (sender, receiver) = mpsc::channel();
        let _ = sender.send(Err(reason));
        Self { receiver }
    }

    pub(crate) fn poll(&self) -> Option<Result<Vec<u8>, String>> {
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
    let limit = super::MAX_BODY_BYTES;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if body.len() > limit {
        return Err(format!("{url} sent more than {limit} bytes"));
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}: {}", String::from_utf8_lossy(&body)));
    }
    Ok(body)
}

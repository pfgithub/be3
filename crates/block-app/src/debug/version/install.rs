mod media_store;

use std::{io::Read, sync::mpsc, thread, time::Duration};

use serde::Deserialize;

use super::github;

const ARTIFACT_NAME: &str = "block-app-android-aarch64";
const APK_ENTRY_NAME: &str = "block-app.apk";

/// A background job that downloads a workflow run's Android artifact, unzips
/// the APK inside it, and hands it to the system installer. Polled from the
/// UI thread each frame like `http::Fetch`.
pub(super) struct Install {
    pub(super) run_id: u64,
    receiver: mpsc::Receiver<Result<(), String>>,
    result: Option<Result<(), String>>,
}

impl Install {
    pub(super) fn start(run_id: u64, short_sha: &str) -> Self {
        let display_name = format!("block-app-{short_sha}.apk");
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(run(run_id, &display_name));
        });
        Self {
            run_id,
            receiver,
            result: None,
        }
    }

    pub(super) fn poll(&mut self) {
        if self.result.is_none() {
            if let Ok(result) = self.receiver.try_recv() {
                self.result = Some(result);
            }
        }
    }

    pub(super) fn finished(&self) -> bool {
        self.result.is_some()
    }

    pub(super) fn error(&self) -> Option<&str> {
        match &self.result {
            Some(Err(error)) => Some(error),
            _ => None,
        }
    }
}

fn run(run_id: u64, display_name: &str) -> Result<(), String> {
    let artifact = fetch_artifact(run_id)?;
    let zip_bytes = get(&artifact.archive_download_url)?;
    let apk_bytes = extract_apk(&zip_bytes)?;
    media_store::install(&apk_bytes, display_name)
}

#[derive(Deserialize)]
struct Artifact {
    name: String,
    archive_download_url: String,
    expired: bool,
}

#[derive(Deserialize)]
struct ArtifactsResponse {
    artifacts: Vec<Artifact>,
}

fn fetch_artifact(run_id: u64) -> Result<Artifact, String> {
    let body = get(&github::artifacts_url(run_id))?;
    let response: ArtifactsResponse = serde_json::from_slice(&body)
        .map_err(|error| format!("failed to parse the artifact list: {error}"))?;
    response
        .artifacts
        .into_iter()
        .find(|artifact| artifact.name == ARTIFACT_NAME)
        .filter(|artifact| !artifact.expired)
        .ok_or_else(|| format!("no unexpired \"{ARTIFACT_NAME}\" artifact on this run"))
}

fn extract_apk(zip_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|error| format!("invalid artifact zip: {error}"))?;
    let mut file = archive
        .by_name(APK_ENTRY_NAME)
        .map_err(|error| format!("\"{APK_ENTRY_NAME}\" was not found in the artifact: {error}"))?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read the apk out of the artifact: {error}"))?;
    Ok(bytes)
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(120))
        .build();
    let mut request = agent.get(url);
    for (key, value) in github::api_headers() {
        request = request.set(key, &value);
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

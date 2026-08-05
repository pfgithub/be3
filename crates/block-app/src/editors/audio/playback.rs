//! Plays an [`Audio`] block's bytes through the platform's audio output.
//!
//! Desktop and Android both go through `rodio`/cpal, which talks to ALSA,
//! CoreAudio, WASAPI or (on Android) Oboe depending on the platform. The web
//! build cannot: cpal's Web Audio backend only targets `wasm32-unknown-unknown`,
//! and this project's browser build targets `wasm32-wasip1` instead (see
//! `scripts/build-block-web.ps1`), so it talks to `<audio>` directly through
//! `web-sys`.

use std::time::Duration;

use block_client::blocks::audio::Audio;

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct AudioPlayer {
    stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
    sink: Option<rodio::Sink>,
    duration: Option<Duration>,
    error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioPlayer {
    pub(super) fn new() -> Self {
        Self {
            stream: None,
            sink: None,
            duration: None,
            error: None,
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        self.sink
            .as_ref()
            .is_some_and(|sink| !sink.is_paused() && !sink.empty())
    }

    pub(super) fn position(&self) -> Duration {
        self.sink
            .as_ref()
            .map_or(Duration::ZERO, rodio::Sink::get_pos)
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Stops playback and drops the sink, so a later `toggle` reloads the
    /// (possibly replaced) audio from scratch.
    pub(super) fn reset(&mut self) {
        self.sink = None;
        self.duration = None;
    }

    fn ensure_stream(&mut self) -> bool {
        if self.stream.is_none() {
            match rodio::OutputStream::try_default() {
                Ok(stream) => self.stream = Some(stream),
                Err(error) => {
                    self.error = Some(format!("Could not open audio output: {error}"));
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn toggle(&mut self, audio: &Audio) {
        if let Some(sink) = &self.sink {
            if sink.is_paused() {
                sink.play();
            } else {
                sink.pause();
            }
            return;
        }
        self.error = None;
        if !self.ensure_stream() {
            return;
        }
        let handle = &self.stream.as_ref().expect("just ensured").1;
        let sink = match rodio::Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(error) => {
                self.error = Some(format!("Could not start playback: {error}"));
                return;
            }
        };
        let cursor = std::io::Cursor::new(audio.data().to_vec());
        let source = match rodio::Decoder::new(cursor) {
            Ok(source) => source,
            Err(error) => {
                self.error = Some(format!("Could not decode audio: {error}"));
                return;
            }
        };
        self.duration = rodio::Source::total_duration(&source);
        sink.append(source);
        self.sink = Some(sink);
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) struct AudioPlayer {
    /// The `<audio>` element playing the current file, and the blob URL it
    /// was created from. Both live for as long as the element does; the URL
    /// is revoked when a new file replaces it or the editor drops.
    element: Option<(web_sys::HtmlAudioElement, String)>,
    error: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl AudioPlayer {
    pub(super) fn new() -> Self {
        Self {
            element: None,
            error: None,
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        self.element
            .as_ref()
            .is_some_and(|(element, _)| !element.paused())
    }

    pub(super) fn position(&self) -> Duration {
        self.element
            .as_ref()
            .map_or(Duration::ZERO, |(element, _)| {
                duration_from_seconds(element.current_time())
            })
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        let (element, _) = self.element.as_ref()?;
        let seconds = element.duration();
        seconds.is_finite().then(|| duration_from_seconds(seconds))
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Stops playback and revokes the blob URL, so a later `toggle` reloads
    /// the (possibly replaced) audio from scratch.
    pub(super) fn reset(&mut self) {
        if let Some((element, url)) = self.element.take() {
            let _ = element.pause();
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    }

    pub(super) fn toggle(&mut self, audio: &Audio) {
        if let Some((element, _)) = &self.element {
            let result = if element.paused() {
                element.play().map(|_| ())
            } else {
                element.pause()
            };
            if result.is_err() {
                self.error = Some("Could not control playback".into());
            }
            return;
        }
        self.error = None;
        match create_element(audio) {
            Ok((element, url)) => {
                let _ = element.play();
                self.element = Some((element, url));
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn duration_from_seconds(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::ZERO)
}

#[cfg(target_arch = "wasm32")]
fn create_element(audio: &Audio) -> Result<(web_sys::HtmlAudioElement, String), String> {
    let bytes = js_sys::Uint8Array::from(audio.data());
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());

    let properties = web_sys::BlobPropertyBag::new();
    properties.set_type(audio.media_type());
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &properties)
        .map_err(|_| "Could not create an audio blob".to_owned())?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Could not create an object URL for the audio".to_owned())?;
    let element = web_sys::HtmlAudioElement::new_with_src(&url).map_err(|_| {
        let _ = web_sys::Url::revoke_object_url(&url);
        "Could not create an audio element".to_owned()
    })?;
    Ok((element, url))
}

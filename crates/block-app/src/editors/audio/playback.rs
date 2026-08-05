//! Plays an [`Audio`] block's bytes through the system's audio output.
//!
//! Android and the web build have no output device wired up yet, so those
//! targets get a stub that always reports itself as unavailable.

use std::time::Duration;

use block_client::blocks::audio::Audio;

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub(super) struct AudioPlayer {
    stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
    sink: Option<rodio::Sink>,
    duration: Option<Duration>,
    error: Option<String>,
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
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

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
pub(super) struct AudioPlayer;

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
impl AudioPlayer {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn is_playing(&self) -> bool {
        false
    }

    pub(super) fn position(&self) -> Duration {
        Duration::ZERO
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        None
    }

    pub(super) fn error(&self) -> Option<&str> {
        Some("Audio playback is not available on this platform yet")
    }

    pub(super) fn reset(&mut self) {}

    pub(super) fn toggle(&mut self, _audio: &Audio) {}

    pub(super) fn stop(&mut self) {}
}

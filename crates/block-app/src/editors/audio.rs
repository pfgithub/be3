mod playback;

use std::time::Duration;

use block_client::{
    blocks::audio::{Audio, AudioOperation},
    BlockClient, BlockHandle,
};
use eframe::egui::{self, Vec2};
use egui_material_icons::{
    icons::{ICON_AUDIO_FILE, ICON_PAUSE, ICON_PLAY_ARROW},
    MaterialIcon,
};

use self::playback::AudioPlayer;

use super::{
    BlockEditor, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};
use crate::platform::{FileFilter, FilePicker, PickedFile};

const AUDIO_FILTER: FileFilter = FileFilter {
    name: "Audio",
    default_file_name: "Audio",
    extensions: &["mp3", "wav", "ogg", "oga", "flac", "m4a"],
    mime_types: &["audio/*"],
};

const DIRECT_EDITOR_SIZE: Vec2 = egui::vec2(320.0, 180.0);

impl EditorKind for AudioEditor {
    type Block = Audio;

    const DISPLAY_NAME: &'static str = "Audio";
    const ICON: MaterialIcon = ICON_AUDIO_FILE;

    fn open(_client: &BlockClient, block: BlockHandle<Audio>) -> Self {
        Self::new(block)
    }
}

/// An audio block is the imported file, so creating one starts by choosing it.
impl ConfigurableEditor for AudioEditor {
    type Options = ChosenAudio;

    fn create(client: &BlockClient, options: ChosenAudio) -> Result<Self, String> {
        let audio = options.audio.ok_or("Choose an audio file first")?;
        Ok(Self::new(client.create_block(audio)))
    }
}

#[derive(Default)]
pub(crate) struct ChosenAudio {
    picker: FilePicker,
    audio: Option<Audio>,
    error: Option<String>,
}

impl CreationOptions for ChosenAudio {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        match self.picker.poll(ui.ctx()).map(|file| file.and_then(decode)) {
            Some(Ok(audio)) => {
                self.audio = Some(audio);
                self.error = None;
            }
            Some(Err(error)) => {
                self.audio = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(ui.ctx(), &AUDIO_FILTER);
            }
            match &self.audio {
                Some(audio) => ui.label(audio.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        self.audio.is_some()
    }
}

pub(crate) struct AudioEditor {
    block: BlockHandle<Audio>,
    player: AudioPlayer,
    picker: FilePicker,
    import_error: Option<String>,
}

impl AudioEditor {
    pub(crate) fn new(block: BlockHandle<Audio>) -> Self {
        Self {
            block,
            player: AudioPlayer::new(),
            picker: FilePicker::default(),
            import_error: None,
        }
    }

    fn poll_picker(&mut self, context: &egui::Context) {
        match self.picker.poll(context).map(|file| file.and_then(decode)) {
            Some(Ok(audio)) => {
                self.block.operate(AudioOperation::Replace { audio });
                self.player.reset();
                self.import_error = None;
            }
            Some(Err(error)) => self.import_error = Some(error),
            None => {}
        }
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn guess_media_type(source_name: &str) -> &'static str {
    match source_name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        _ => "audio/mpeg",
    }
}

fn decode(file: PickedFile) -> Result<Audio, String> {
    let PickedFile { name, data } = file;
    let media_type = guess_media_type(&name);
    Audio::new(name.clone(), media_type, data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}

impl BlockEditor for AudioEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        Some(DIRECT_EDITOR_SIZE)
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        ui.heading("Audio");
        self.poll_picker(ui.ctx());
        if ui
            .add_enabled(
                !self.picker.is_open(),
                egui::Button::new("Replace audio..."),
            )
            .clicked()
        {
            self.picker.open(ui.ctx(), &AUDIO_FILTER);
        }
        if let Some(error) = &self.import_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(audio) = self.block.read().map(|audio| audio.clone()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        if self.player.is_playing() {
            ui.ctx().request_repaint();
        }
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(ICON_AUDIO_FILE.codepoint)
                    .font(egui::FontId::new(48.0, ICON_AUDIO_FILE.font_family())),
            );
            ui.label(audio.source_name());
            ui.add_space(8.0);
            let (icon, hover) = if self.player.is_playing() {
                (ICON_PAUSE, "Pause")
            } else {
                (ICON_PLAY_ARROW, "Play")
            };
            if ui.button(icon).on_hover_text(hover).clicked() {
                self.player.toggle(&audio);
            }
            let duration = self
                .player
                .duration()
                .map_or_else(|| "--:--".to_owned(), format_duration);
            ui.label(format!(
                "{} / {duration}",
                format_duration(self.player.position())
            ));
            if let Some(error) = self.player.error() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        None
    }
}

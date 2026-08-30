use std::sync::Arc;
use std::time::Duration;

use block_client::blocks::audio::{Audio, AudioOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_AUDIO_FILE, ICON_PAUSE, ICON_PLAY_ARROW,
};
use block_editor_plugin::{egui, EditorHost, FileFilter, FilePicker, PickedFile};
use uuid::Uuid;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(320.0, 180.0);

struct Editing {
    host: EditorHost,
    block: BlockHandle<Audio>,
}

struct Creating {
    host: EditorHost,
    client: Arc<BlockClient>,
    chosen: Option<Audio>,
}

#[derive(Default)]
pub struct AudioApp {
    editing: Option<Editing>,
    creation: Option<Creating>,
    picker: FilePicker,
    error: Option<String>,
}

impl block_editor_plugin::App for AudioApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.editing = Some(Editing {
            host,
            block: client.get_block(block_id),
        });
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(Creating {
            host,
            client,
            chosen: None,
        });
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .creation
            .as_mut()
            .ok_or("this editor is not filling in a block")?;
        let audio = creation.chosen.take().ok_or("no file was chosen")?;
        Ok(creation.client.create_block(audio).id())
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        let Some(Creating { host, chosen, .. }) = &mut self.creation else {
            return;
        };
        match self.picker.poll(host).map(|file| file.and_then(decode)) {
            Some(Ok(audio)) => {
                host.set_creation_ready(true);
                *chosen = Some(audio);
                self.error = None;
            }
            Some(Err(error)) => {
                host.set_creation_ready(false);
                *chosen = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(host, filter());
            }
            match chosen {
                Some(audio) => ui.label(audio.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(Editing { host, block }) = &mut self.editing else {
            return;
        };
        ui.heading("Audio");
        match self.picker.poll(host).map(|file| file.and_then(decode)) {
            Some(Ok(audio)) => {
                block.operate(AudioOperation::Replace { audio });
                host.reset_audio(block.id());
                self.error = None;
            }
            Some(Err(error)) => self.error = Some(error),
            None => {}
        }
        if ui
            .add_enabled(
                host.editable() && !self.picker.is_open(),
                egui::Button::new("Replace audio..."),
            )
            .test_id("audio.replace")
            .clicked()
        {
            self.picker.open(host, filter());
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(Editing { host, block }) = &self.editing else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(source_name) = block.read().map(|audio| audio.source_name().to_owned()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let status = host.audio();
        if status.playing {
            ui.ctx().request_repaint();
        }
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(ICON_AUDIO_FILE.codepoint)
                    .font(egui::FontId::new(48.0, ICON_AUDIO_FILE.font_family())),
            );
            ui.label(source_name);
            ui.add_space(8.0);
            let (icon, hover) = match status.playing {
                true => (ICON_PAUSE, "Pause"),
                false => (ICON_PLAY_ARROW, "Play"),
            };
            if ui
                .button(icon)
                .on_hover_text(hover)
                .test_id("audio.play")
                .clicked()
            {
                host.play_audio(block.id());
            }
            let duration = status
                .duration_micros
                .map_or_else(|| "--:--".to_owned(), format_micros);
            ui.label(format!(
                "{} / {duration}",
                format_micros(status.position_micros)
            ));
            if let Some(error) = &status.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
    }
}

fn format_micros(micros: u64) -> String {
    format_duration(Duration::from_micros(micros))
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn filter() -> FileFilter {
    FileFilter {
        name: "Audio".to_owned(),
        default_file_name: "Audio".to_owned(),
        extensions: ["mp3", "wav", "ogg", "oga", "flac", "m4a"]
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: vec!["audio/*".to_owned()],
    }
}

pub(crate) fn guess_media_type(source_name: &str) -> &'static str {
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

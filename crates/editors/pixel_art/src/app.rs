use std::{collections::HashMap, sync::Arc};

use block::Block;
use block_client::{
    blocks::{
        image::Image,
        pixel_art::{PixelArt, PixelArtAnchor, PixelArtOperation, PixelColor},
    },
    BlockClient, BlockHandle,
};
use block_editor_plugin::{egui, Artifact, ArtifactDescription, EditorHost};
use uuid::Uuid;

use crate::{
    artifact,
    canvas::{Pane, View},
    color::format_hex_color,
    drawing::{ActiveDrawing, Brush, BrushShape, CommittedPreview, PixelTool},
};

const MAX_RECENT_COLORS: usize = 12;
const EMBEDDED_LONG_SIDE: f32 = 256.0;
const EMBEDDED_SHORT_SIDE: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneKey {
    Main,
    Preview,
}

pub struct Editing {
    pub host: EditorHost,
    pub client: Arc<BlockClient>,
    pub block: BlockHandle<PixelArt>,
}

struct Exporting {
    client: Arc<BlockClient>,
    block_id: Uuid,
    block_type: Uuid,
    regeneration: Option<artifact::Regeneration>,
    failure: Option<String>,
}

pub struct PixelArtApp {
    pub(crate) editing: Option<Editing>,
    creation: Option<Arc<BlockClient>>,
    exporting: Option<Exporting>,
    pub(crate) tool: PixelTool,
    pub(crate) previous_drawing_tool: PixelTool,
    pub(crate) color: PixelColor,
    pub(crate) color_hex: String,
    pub(crate) recent_colors: Vec<PixelColor>,
    pub(crate) replace_source_hover: Option<PixelColor>,
    pub(crate) brush_size: u16,
    pub(crate) brush_shape: BrushShape,
    pub(crate) shapes_filled: bool,
    pub(crate) mirror_horizontal: bool,
    pub(crate) mirror_vertical: bool,
    pub(crate) show_grid: bool,
    pub(crate) active_drawing: Option<ActiveDrawing>,
    pub(crate) committed_preview: Option<CommittedPreview>,
    pub(crate) view: View,
    panes: HashMap<PaneKey, Pane>,
    pub(crate) resize_open: bool,
    pub(crate) resize_width: u16,
    pub(crate) resize_height: u16,
    pub(crate) resize_anchor: PixelArtAnchor,
    pub(crate) clear_open: bool,
    pub(crate) export_error: Option<String>,
}

impl Default for PixelArtApp {
    fn default() -> Self {
        Self {
            editing: None,
            creation: None,
            exporting: None,
            tool: PixelTool::Pencil,
            previous_drawing_tool: PixelTool::Pencil,
            color: PixelColor::new(0, 0, 0, 255),
            color_hex: "#000000FF".into(),
            recent_colors: vec![PixelColor::new(0, 0, 0, 255)],
            replace_source_hover: None,
            brush_size: 1,
            brush_shape: BrushShape::Square,
            shapes_filled: false,
            mirror_horizontal: false,
            mirror_vertical: false,
            show_grid: true,
            active_drawing: None,
            committed_preview: None,
            view: View::default(),
            panes: HashMap::new(),
            resize_open: false,
            resize_width: 32,
            resize_height: 32,
            resize_anchor: PixelArtAnchor::Center,
            clear_open: false,
            export_error: None,
        }
    }
}

impl block_editor_plugin::App for PixelArtApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        let block = client.get_block::<PixelArt>(block_id);
        self.editing = Some(Editing {
            host,
            client,
            block,
        });
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not filling in a block")?;
        Ok(client.create_block(PixelArt::new()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.canvas_ui(ui);
        if let Some((width, height)) = self.dimensions() {
            self.dialogs_ui(ui, width, height);
        }
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        let dark_mode = ui.visuals().dark_mode;
        if self
            .refresh_pane(ui.ctx(), PaneKey::Preview, dark_mode)
            .is_some()
        {
            self.paint_pane(PaneKey::Preview, ui.painter(), rect);
        }
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some((width, height)) = self.dimensions() else {
            ui.horizontal(|ui| {
                ui.strong("Pixel Art");
                ui.spinner();
            });
            return;
        };
        self.top_bar_ui(ui, width, height);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.tools_ui(ui);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(palette) = self
            .editing
            .as_ref()
            .and_then(|editing| editing.block.read().map(|art| art.palette().to_vec()))
        else {
            return;
        };
        self.colors_ui(ui, &palette);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let (width, height) = self.dimensions()?;
        let width = f32::from(width);
        let height = f32::from(height);
        let pixel_size =
            (EMBEDDED_LONG_SIDE / width.max(height)).max(EMBEDDED_SHORT_SIDE / width.min(height));
        Some(egui::vec2(width * pixel_size, height * pixel_size))
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        let (width, height) = self.dimensions()?;
        Some(f32::from(width) / f32::from(height))
    }

    fn connect_artifact(
        &mut self,
        _host: EditorHost,
        client: Arc<BlockClient>,
        artifact: Artifact,
    ) {
        self.exporting = Some(Exporting {
            client,
            block_id: artifact.block_id,
            block_type: artifact.block_type,
            regeneration: None,
            failure: None,
        });
    }

    fn describe_artifact(&mut self, data: &[u8]) -> Result<ArtifactDescription, String> {
        artifact::describe(data)
    }

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        artifact::settings_ui(ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(exporting) = &mut self.exporting else {
            return;
        };
        match artifact::Regeneration::start(
            &exporting.client,
            exporting.block_id,
            exporting.block_type,
            data,
        ) {
            Ok(regeneration) => {
                exporting.regeneration = Some(regeneration);
                exporting.failure = None;
            }
            Err(error) => {
                exporting.regeneration = None;
                exporting.failure = Some(error);
            }
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        let exporting = self.exporting.as_mut()?;
        if let Some(failure) = exporting.failure.take() {
            return Some(Err(failure));
        }
        let result = exporting.regeneration.as_mut()?.poll()?;
        exporting.regeneration = None;
        Some(result)
    }
}

impl PixelArtApp {
    pub(crate) fn dimensions(&self) -> Option<(u16, u16)> {
        let art = self.editing.as_ref()?.block.read()?;
        Some((art.width(), art.height()))
    }

    pub(crate) fn editable(&self) -> bool {
        self.editing
            .as_ref()
            .is_none_or(|editing| editing.host.editable())
    }

    pub(crate) fn refresh_pane(
        &mut self,
        context: &egui::Context,
        key: PaneKey,
        dark_mode: bool,
    ) -> Option<(u16, u16)> {
        let art = self.editing.as_ref()?.block.read()?;
        let size = (art.width(), art.height());
        self.panes
            .entry(key)
            .or_default()
            .ensure(context, &art, dark_mode);
        Some(size)
    }

    pub(crate) fn preview_pixels(
        &mut self,
        key: PaneKey,
        pixels: &[(u16, u16)],
        color: PixelColor,
    ) {
        if let Some(pane) = self.panes.get_mut(&key) {
            pane.set_preview(pixels, color);
        }
    }

    pub(crate) fn paint_pane(&self, key: PaneKey, painter: &egui::Painter, rect: egui::Rect) {
        if let Some(pane) = self.panes.get(&key) {
            pane.paint(painter, rect);
        }
    }

    pub(crate) fn brush(&self, constrained: bool) -> Brush {
        Brush {
            size: self.brush_size,
            shape: self.brush_shape,
            filled: self.shapes_filled,
            mirror_horizontal: self.mirror_horizontal,
            mirror_vertical: self.mirror_vertical,
            constrained,
        }
    }

    pub(crate) fn operate(&self, operation: PixelArtOperation) {
        if let Some(editing) = &self.editing {
            editing.block.operate(operation);
        }
    }

    pub(crate) fn select_tool(&mut self, tool: PixelTool) {
        self.active_drawing = None;
        self.committed_preview = None;
        self.replace_source_hover = None;
        if self.tool.is_drawing() {
            self.previous_drawing_tool = self.tool;
        }
        if tool.is_drawing() {
            self.previous_drawing_tool = tool;
        }
        self.tool = tool;
    }

    pub(crate) fn remember_color(&mut self, color: PixelColor) {
        self.recent_colors.retain(|recent| *recent != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(MAX_RECENT_COLORS);
    }

    pub(crate) fn set_active_color(&mut self, color: PixelColor, remember: bool) {
        self.color = color;
        self.color_hex = format_hex_color(color);
        if remember {
            self.remember_color(color);
        }
    }

    pub(crate) fn export(&mut self) {
        let Some(editing) = &self.editing else {
            return;
        };
        let name = editing
            .block
            .name()
            .unwrap_or_else(|| "Pixel Art".to_owned());
        let Some(art) = editing.block.read() else {
            return;
        };
        let generated = artifact::generate_initial(&art, &name);
        drop(art);
        match generated {
            Ok(image) => {
                let child = editing
                    .client
                    .create_dynamic_artifact(image, artifact::descriptor(editing.block.id()));
                editing.host.open_block(child.id(), Image::TYPE_ID);
                self.export_error = None;
            }
            Err(error) => self.export_error = Some(error),
        }
    }
}

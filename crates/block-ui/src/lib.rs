pub mod database;
pub mod datetime;
pub mod frame;
pub mod test_id;

use std::collections::{BTreeMap, HashMap};

use block::BlockReference;
use block_client::{presence::PresenceColor, BlockHandleAccess, CachedBlock};
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

pub const EMBEDDED_EDITOR_PADDING: f32 = 12.0;
pub const EMBEDDED_EDITOR_TITLE_HEIGHT: f32 = 28.0;
pub const EMBEDDED_EDITOR_TITLE_GAP: f32 = 8.0;

pub fn embedded_editor_frame_size(intrinsic: egui::Vec2, scale: f32) -> egui::Vec2 {
    egui::vec2(
        (intrinsic.x + EMBEDDED_EDITOR_PADDING * 2.0) * scale,
        (intrinsic.y
            + EMBEDDED_EDITOR_PADDING * 2.0
            + EMBEDDED_EDITOR_TITLE_HEIGHT
            + EMBEDDED_EDITOR_TITLE_GAP)
            * scale,
    )
}

pub fn presence_color(color: PresenceColor) -> egui::Color32 {
    match color {
        PresenceColor::Red => egui::Color32::from_rgb(224, 82, 82),
        PresenceColor::Orange => egui::Color32::from_rgb(230, 140, 50),
        PresenceColor::Yellow => egui::Color32::from_rgb(214, 179, 41),
        PresenceColor::Green => egui::Color32::from_rgb(84, 171, 90),
        PresenceColor::Teal => egui::Color32::from_rgb(46, 173, 168),
        PresenceColor::Blue => egui::Color32::from_rgb(74, 134, 227),
        PresenceColor::Purple => egui::Color32::from_rgb(150, 100, 214),
        PresenceColor::Pink => egui::Color32::from_rgb(224, 104, 168),
    }
}

pub trait BlockTypes {
    fn display_name(&self, block_type: Uuid) -> Option<&str>;
    fn icon(&self, block_type: Uuid) -> Option<MaterialIcon>;
}

pub struct BlockTypeEntry {
    pub display_name: String,
    pub icon: Option<MaterialIcon>,
}

#[derive(Default)]
pub struct BlockCatalog {
    types: HashMap<Uuid, BlockTypeEntry>,
}

impl BlockCatalog {
    pub fn new(types: impl IntoIterator<Item = (Uuid, BlockTypeEntry)>) -> Self {
        Self {
            types: types.into_iter().collect(),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &BlockTypeEntry)> {
        self.types.iter()
    }
}

impl BlockTypes for BlockCatalog {
    fn display_name(&self, block_type: Uuid) -> Option<&str> {
        self.types
            .get(&block_type)
            .map(|entry| entry.display_name.as_str())
    }

    fn icon(&self, block_type: Uuid) -> Option<MaterialIcon> {
        self.types.get(&block_type).and_then(|entry| entry.icon)
    }
}
#[derive(Clone)]

pub struct BlockLabel {
    pub block_type: Uuid,
    pub icon: Option<MaterialIcon>,
    pub name: String,
    pub automatic: bool,
}

impl BlockLabel {
    pub fn new(
        types: &dyn BlockTypes,
        block_type: Uuid,
        name: Option<&block_client::properties::BlockName>,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.value.is_empty()) {
            Some(name) => (name.value.clone(), !name.manual),
            None => (
                types
                    .display_name(block_type)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "Untitled".to_owned()),
                true,
            ),
        };
        Self {
            block_type,
            icon: types.icon(block_type),
            name,
            automatic,
        }
    }

    pub fn for_properties(
        types: &dyn BlockTypes,
        block_type: Uuid,
        properties: &BTreeMap<Uuid, Vec<u8>>,
    ) -> Self {
        Self::new(
            types,
            block_type,
            block_client::properties::read_name(properties).as_ref(),
        )
    }

    pub fn for_reference(types: &dyn BlockTypes, reference: &BlockReference) -> Self {
        Self::for_properties(types, reference.block_type, &reference.properties)
    }

    pub fn for_cached(types: &dyn BlockTypes, cached: &CachedBlock) -> Self {
        Self::for_properties(types, cached.block_type, &cached.properties)
    }

    pub fn for_handle(types: &dyn BlockTypes, handle: &dyn BlockHandleAccess) -> Self {
        Self::new(types, handle.block_type(), handle.block_name().as_ref())
    }

    pub fn rich_text(&self) -> egui::RichText {
        let text = egui::RichText::new(&self.name);
        if self.automatic {
            text.italics()
        } else {
            text
        }
    }

    pub fn widget_text(&self, style: &egui::Style) -> egui::WidgetText {
        let Some(icon) = self.icon else {
            return self.rich_text().into();
        };
        let mut job = egui::text::LayoutJob::default();
        egui::RichText::new(format!("{} ", icon.codepoint)).append_to(
            &mut job,
            style,
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        self.rich_text().append_to(
            &mut job,
            style,
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        job.into()
    }
}

pub fn name_galley(
    painter: &egui::Painter,
    text: &str,
    font_id: egui::FontId,
    color: egui::Color32,
    automatic: bool,
) -> std::sync::Arc<egui::Galley> {
    if !automatic {
        return painter.layout_no_wrap(text.to_owned(), font_id, color);
    }
    painter.layout_job(egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::text::TextFormat {
            font_id,
            color,
            italics: true,
            ..Default::default()
        },
    ))
}

pub fn paint_name(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: &str,
    font_id: egui::FontId,
    color: egui::Color32,
    automatic: bool,
) -> egui::Rect {
    let galley = name_galley(painter, text, font_id, color, automatic);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

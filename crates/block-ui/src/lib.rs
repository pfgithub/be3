use std::collections::{BTreeMap, HashMap};

use block::BlockReference;
use block_client::{BlockHandleAccess, CachedBlock};
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

/// What the app knows about the registered block types: enough to name and
/// illustrate a block whose editor is not open. The host answers from its
/// editor registry; a plugin editor answers from the catalog the host sent it.
pub trait BlockTypes {
    fn display_name(&self, block_type: Uuid) -> Option<&str>;
    fn icon(&self, block_type: Uuid) -> Option<MaterialIcon>;
}

pub struct BlockTypeEntry {
    pub display_name: String,
    pub icon: Option<MaterialIcon>,
}

/// A standalone answer to [`BlockTypes`], for anything that is told about the
/// block types rather than owning their registration.
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

/// A block's icon and display name, along with whether the name was
/// auto-derived from the block's content rather than chosen by the user.
/// Shared by every place that shows a block's name, so an automatic name can
/// be marked as such (e.g. italicized) consistently.
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

    /// For a block type and its raw property map, e.g. from a
    /// [`BlockReference`] or [`CachedBlock`].
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

    /// For a listed [`BlockReference`].
    pub fn for_reference(types: &dyn BlockTypes, reference: &BlockReference) -> Self {
        Self::for_properties(types, reference.block_type, &reference.properties)
    }

    /// For a [`CachedBlock`].
    pub fn for_cached(types: &dyn BlockTypes, cached: &CachedBlock) -> Self {
        Self::for_properties(types, cached.block_type, &cached.properties)
    }

    /// For a block whose editor is open locally.
    pub fn for_handle(types: &dyn BlockTypes, handle: &dyn BlockHandleAccess) -> Self {
        Self::new(types, handle.block_type(), handle.block_name().as_ref())
    }

    /// The name alone, italicized if it was auto-derived rather than chosen
    /// by the user.
    pub fn rich_text(&self) -> egui::RichText {
        let text = egui::RichText::new(&self.name);
        if self.automatic {
            text.italics()
        } else {
            text
        }
    }

    /// Icon and name combined for a widget (button, label, ...), the name
    /// italicized if automatic.
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

/// Lays out `text` for direct painting, matching
/// [`egui::Painter::layout_no_wrap`] but italicizing it when `automatic` -
/// for marking an auto-derived block name in painter-based (non-widget)
/// rendering.
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

/// [`egui::Painter::text`], but italicizing the text when `automatic`.
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

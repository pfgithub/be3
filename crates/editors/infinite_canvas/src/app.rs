mod app_impl;
mod core;

pub use app_impl::CanvasApp;
mod geometry;
mod inspector;
mod interaction;
mod painting;
#[cfg(test)]
mod tests;

use geometry::*;
use painting::*;

use std::collections::{HashMap, HashSet};

use block::{BlockParent, BlockReferenceList, ClientId};
use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::DatabaseValue,
        database_schema::DatabaseSchema,
        image::Image as ImageBlock,
        infinite_canvas::{
            CanvasColor, CanvasComponent, CanvasCursor, CanvasEntity, CanvasEntityKind,
            CanvasEntityStyle, CanvasLayerMove, CanvasPoint, CanvasPreviewRegion, CanvasTextAlign,
            CanvasTextStyle, CanvasTextWeight, CanvasTransform, InfiniteCanvas,
            InfiniteCanvasOperation,
        },
    },
    presence::{PresenceColor, UserActive},
    BlockClient, BlockHandle, ReferenceList,
};
use block_editor_plugin::{
    block_ui::{
        self,
        database::{DatabaseValueEditor, DatabaseValueEditorOutput},
        name_galley, paint_name, BlockLabel, EMBEDDED_EDITOR_PADDING, EMBEDDED_EDITOR_TITLE_GAP,
        EMBEDDED_EDITOR_TITLE_HEIGHT,
    },
    egui::{self, Color32, PointerButton, Pos2, Rect, Stroke, Vec2},
    egui_material_icons::icons::{
        ICON_CIRCLE, ICON_DATA_OBJECT, ICON_DIAGONAL_LINE, ICON_DRAW, ICON_FORMAT_COLOR_RESET,
        ICON_KEYBOARD_ARROW_DOWN, ICON_RECTANGLE, ICON_SELECT, ICON_TEXT_FIELDS, ICON_ZOOM_IN,
        ICON_ZOOM_OUT,
    },
    BlockFilter, BlockPicker, ChildMode, EditorHost, FilePicker, ImagePaster, PastedImage,
    ViewChange,
};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use uuid::Uuid;

use block_client::references;

use crate::access::{Access, Children, DirectEditorInteraction, DirectEditorResize};
use crate::images::{image_filter, imported_image};
use crate::viewport::Viewport as DirectEditorViewport;

pub(crate) enum EditorAction {
    OpenBlock { id: Uuid, block_type: Uuid },
}

const MIN_SIZE: f32 = 4.0;
const HIT_RADIUS: f32 = 7.0;
const HANDLE_RADIUS: f32 = 5.0;
const ROTATE_OFFSET: f32 = 28.0;
const ZOOM_STEP: f32 = 1.25;
const IMPORT_CASCADE_OFFSET: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CommonValue<T> {
    None,
    Mixed,
    Uniform(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Line,
    Rectangle,
    Text,
    Pen,
}

#[derive(Clone, Copy)]
enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

#[derive(Clone, Copy)]
enum CanvasCommand {
    SelectAll,
    InvertSelection,
    Duplicate,
    Delete,
    Lock,
    Unlock,
    Group,
    Ungroup,
    Reorder(CanvasLayerMove),
    Copy,
    Cut,
    Paste,
}

#[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
const CANVAS_CLIPBOARD_PREFIX: &str = "be3-infinite-canvas:";

#[derive(Deserialize, Serialize)]
#[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
struct CanvasClipboardPayload {
    entities: Vec<CanvasEntity>,
}

#[derive(Clone, Copy, Debug)]
struct WorldRect {
    min: CanvasPoint,
    max: CanvasPoint,
}

#[derive(Clone, Copy, Debug)]
struct SelectionFrame {
    center: CanvasPoint,
    size: CanvasPoint,
    rotation: f32,
}

impl SelectionFrame {
    fn from_world_rect(bounds: WorldRect) -> Self {
        Self {
            center: bounds.center(),
            size: bounds.size(),
            rotation: 0.0,
        }
    }

    fn point(self, local: CanvasPoint) -> CanvasPoint {
        local_to_world(
            CanvasTransform::new(self.center, self.size, self.rotation),
            local,
        )
    }

    fn contains(self, point: CanvasPoint) -> bool {
        let local = world_to_local(
            CanvasTransform::new(self.center, self.size, self.rotation),
            point,
        );
        local.x.abs() <= 0.5 && local.y.abs() <= 0.5
    }

    fn local_bounds(self) -> WorldRect {
        WorldRect {
            min: CanvasPoint::new(-self.size.x * 0.5, -self.size.y * 0.5),
            max: CanvasPoint::new(self.size.x * 0.5, self.size.y * 0.5),
        }
    }
}

impl WorldRect {
    fn from_points(a: CanvasPoint, b: CanvasPoint) -> Self {
        Self {
            min: CanvasPoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: CanvasPoint::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    fn center(self) -> CanvasPoint {
        CanvasPoint::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    fn size(self) -> CanvasPoint {
        CanvasPoint::new(self.max.x - self.min.x, self.max.y - self.min.y)
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: CanvasPoint::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: CanvasPoint::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    fn contains_rect(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.max.x >= other.max.x
            && self.min.y <= other.min.y
            && self.max.y >= other.max.y
    }

    fn contains(self, point: CanvasPoint) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

fn gesture_rect(start: CanvasPoint, current: CanvasPoint, from_center: bool) -> WorldRect {
    let opposite = if from_center {
        CanvasPoint::new(start.x * 2.0 - current.x, start.y * 2.0 - current.y)
    } else {
        start
    };
    WorldRect::from_points(opposite, current)
}

fn inspector_text_size(ui: &egui::Ui, text_style: &CanvasTextStyle, text: &str) -> CanvasPoint {
    let font_size = text_style.font_size.clamp(4.0, 256.0);
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        egui::FontId::proportional(font_size),
        Color32::WHITE,
        f32::INFINITY,
    );
    job.sections[0].format.line_height = Some(font_size * text_style.line_height.max(0.5));
    let size = ui.painter().layout_job(job).size();
    CanvasPoint::new(
        (size.x + 8.0).max(16.0),
        (size.y + 8.0).max(font_size * text_style.line_height),
    )
}

#[derive(Clone, Copy, Debug)]
struct ResizeHandle {
    x: i8,
    y: i8,
}

#[derive(Clone, Debug)]
enum Gesture {
    Create {
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
    },
    Pen {
        points: Vec<CanvasPoint>,
    },
    SelectBox {
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
        additive: bool,
    },
    Move {
        start: CanvasPoint,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        duplicate: bool,
    },
    Resize {
        handle: ResizeHandle,
        frame: SelectionFrame,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        default_preserve_aspect_ratio: bool,
        force_preserve_aspect_ratio: bool,
        preserve_aspect_ratio: bool,
        scale_text: bool,
        scale_editors: bool,
    },
    Rotate {
        frame: SelectionFrame,
        start_angle: f32,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        snap_angle: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct TwoFingerTouch {
    start_time: f64,
    start_center: Pos2,
    last_center: Pos2,
    max_touches: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingComponentValuePick {
    schema_id: BlockRef,
    field_id: Uuid,
    entity_ids: Vec<Uuid>,
}

const DEFAULT_VIEW_WIDTH: f32 = 1280.0;

pub(crate) struct InfiniteCanvasEditor {
    access: Access,
    presence_visible: bool,
    block: BlockHandle<InfiniteCanvas>,
    tool: Tool,
    render_scale: f32,
    selection: HashSet<Uuid>,
    gesture: Option<Gesture>,
    two_finger_touch: Option<TwoFingerTouch>,
    picker: BlockPicker,
    component_picker: BlockPicker,
    value_picker: BlockPicker,
    pending_value_target: Option<PendingComponentValuePick>,
    pending_component_entities: Option<Vec<Uuid>>,
    pending_block_center: Option<CanvasPoint>,
    context_menu_position: Option<CanvasPoint>,
    context_menu_for_selection: bool,
    dependencies: ReferenceList,
    editing_text: Option<Uuid>,
    focus_text_requested: bool,
    image_import_error: Option<String>,
    image_picker: FilePicker,
    pending_image_center: Option<CanvasPoint>,
    pending_file_drop_position: Option<CanvasPoint>,
    clipboard_image_paste: ImagePaster,
    focused_editor: Option<Uuid>,
    viewport_center: CanvasPoint,

    pointer_world: Option<CanvasPoint>,
    fit_selection_requested: bool,
    fit_preview_region_requested: bool,
    fit_entity_requested: Option<Uuid>,
    pending_presence_reveal: Option<u64>,
    grouped_inspector_edit_active: bool,
    last_foreground: CanvasColor,
    last_fill: Option<CanvasColor>,
    reference_cache: references::ReferenceResolutionCache,
    pending_entities: references::ReferenceClassificationQueue<(Uuid, CanvasTransform)>,
    pending_components: references::ReferenceClassificationQueue<Vec<Uuid>>,
    pending_values: references::ReferenceClassificationQueue<PendingComponentValuePick>,
    component_editors: HashMap<BlockRef, DatabaseValueEditor>,
    component_editor_selection: Vec<Uuid>,
}

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod codegen;

const EDIT_BURST_DELAY: Duration = Duration::from_millis(750);

pub const MIN_CANVAS_SIZE: f32 = 120.0;
pub const MAX_CANVAS_SIZE: f32 = 4096.0;
pub const MAX_SPACE_HEIGHT: f32 = 512.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiLayout {
    #[default]
    Vertical,
    Horizontal,
}

/// Every widget the builder can place. `Container` is the only kind that can
/// hold children; the other kinds keep any children they were given so that
/// converting a container away and back does not lose them.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "widget", rename_all = "snake_case")]
pub enum GuiWidgetKind {
    Heading {
        text: String,
    },
    Label {
        text: String,
    },
    Button {
        text: String,
    },
    TextField {
        label: String,
        value: String,
        multiline: bool,
    },
    Checkbox {
        label: String,
        checked: bool,
    },
    Slider {
        label: String,
        value: f32,
        min: f32,
        max: f32,
    },
    Separator,
    Space {
        height: f32,
    },
    Container {
        layout: GuiLayout,
        framed: bool,
    },
}

impl GuiWidgetKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Heading { .. } => "Heading",
            Self::Label { .. } => "Label",
            Self::Button { .. } => "Button",
            Self::TextField { .. } => "Text field",
            Self::Checkbox { .. } => "Checkbox",
            Self::Slider { .. } => "Slider",
            Self::Separator => "Separator",
            Self::Space { .. } => "Space",
            Self::Container { .. } => "Container",
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self, Self::Container { .. })
    }

    /// Text the editor shows for the widget in outlines and pickers.
    pub fn summary(&self) -> String {
        match self {
            Self::Heading { text } | Self::Label { text } | Self::Button { text } => text.clone(),
            Self::TextField { label, .. }
            | Self::Checkbox { label, .. }
            | Self::Slider { label, .. } => label.clone(),
            Self::Separator => String::new(),
            Self::Space { height } => format!("{height} px"),
            Self::Container { layout, .. } => match layout {
                GuiLayout::Vertical => "Vertical".into(),
                GuiLayout::Horizontal => "Horizontal".into(),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GuiWidget {
    pub id: Uuid,
    pub kind: GuiWidgetKind,
    pub children: Vec<GuiWidget>,
}

impl GuiWidget {
    pub fn new(kind: GuiWidgetKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            children: Vec::new(),
        }
    }
}

/// Where a widget sits: which container holds it, and its index among that
/// container's children. `None` is the root list.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GuiLocation {
    pub parent: Option<Uuid>,
    pub index: usize,
}

impl GuiLocation {
    pub const fn new(parent: Option<Uuid>, index: usize) -> Self {
        Self { parent, index }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct GuiCanvasSize {
    pub width: f32,
    pub height: f32,
}

impl GuiCanvasSize {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl Default for GuiCanvasSize {
    fn default() -> Self {
        Self::new(420.0, 320.0)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GuiBuilder {
    title: String,
    canvas: GuiCanvasSize,
    widgets: Vec<GuiWidget>,
}

impl Default for GuiBuilder {
    fn default() -> Self {
        Self {
            title: "Window".into(),
            canvas: GuiCanvasSize::default(),
            widgets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GuiBuilderOperation {
    Insert {
        location: GuiLocation,
        widget: GuiWidget,
    },
    Remove {
        id: Uuid,
    },
    Move {
        id: Uuid,
        location: GuiLocation,
    },
    SetKind {
        id: Uuid,
        kind: GuiWidgetKind,
    },
    SetTitle {
        title: String,
    },
    SetCanvasSize {
        canvas: GuiCanvasSize,
    },
}

impl GuiBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn canvas(&self) -> GuiCanvasSize {
        self.canvas
    }

    /// The root-level widgets, in layout order.
    pub fn widgets(&self) -> &[GuiWidget] {
        &self.widgets
    }

    pub fn widget(&self, id: Uuid) -> Option<&GuiWidget> {
        find(&self.widgets, id)
    }

    pub fn location(&self, id: Uuid) -> Option<GuiLocation> {
        locate(&self.widgets, None, id)
    }

    /// The children of `parent`, or the root list for `None`. Returns `None`
    /// when the parent is missing or cannot hold children.
    pub fn children(&self, parent: Option<Uuid>) -> Option<&[GuiWidget]> {
        match parent {
            None => Some(&self.widgets),
            Some(parent) => {
                let widget = self.widget(parent)?;
                widget
                    .kind
                    .is_container()
                    .then_some(widget.children.as_slice())
            }
        }
    }

    /// Rust source for an `egui` window matching the current design.
    pub fn generate_code(&self) -> String {
        codegen::generate(self)
    }

    /// A widget can only be inserted when every id in its subtree is new and
    /// unique, so ids stay usable as stable handles.
    fn can_insert(&self, widget: &GuiWidget) -> bool {
        let mut ids = Vec::new();
        collect_ids(widget, &mut ids);
        let mut seen = HashSet::with_capacity(ids.len());
        ids.iter()
            .all(|id| seen.insert(*id) && self.widget(*id).is_none())
    }

    fn children_mut(&mut self, parent: Option<Uuid>) -> Option<&mut Vec<GuiWidget>> {
        match parent {
            None => Some(&mut self.widgets),
            Some(parent) => {
                let widget = find_mut(&mut self.widgets, parent)?;
                if widget.kind.is_container() {
                    Some(&mut widget.children)
                } else {
                    None
                }
            }
        }
    }

    fn detach(&mut self, id: Uuid) -> Option<GuiWidget> {
        detach_from(&mut self.widgets, id)
    }

    fn insert_at(&mut self, location: GuiLocation, widget: GuiWidget) {
        if let Some(siblings) = self.children_mut(location.parent) {
            let index = location.index.min(siblings.len());
            siblings.insert(index, widget);
        }
    }
}

impl Block for GuiBuilder {
    type Operation = GuiBuilderOperation;
    type History = GuiBuilderHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6775_692d_6275_696c_6465_722d_3030_3031);

    fn apply_operation(builder: &mut Self, operation: &Self::Operation) {
        match operation {
            GuiBuilderOperation::Insert { location, widget } => {
                if !builder.can_insert(widget) || builder.children_mut(location.parent).is_none() {
                    return;
                }
                builder.insert_at(*location, widget.clone());
            }
            GuiBuilderOperation::Remove { id } => {
                builder.detach(*id);
            }
            GuiBuilderOperation::Move { id, location } => {
                // A widget cannot be moved inside itself, and the destination
                // has to survive the detach, so both are checked up front.
                let moves_into_itself = builder.widget(*id).is_some_and(|widget| {
                    location
                        .parent
                        .is_some_and(|parent| contains(widget, parent))
                });
                if moves_into_itself || builder.children_mut(location.parent).is_none() {
                    return;
                }
                let Some(widget) = builder.detach(*id) else {
                    return;
                };
                builder.insert_at(*location, widget);
            }
            GuiBuilderOperation::SetKind { id, kind } => {
                if let Some(widget) = find_mut(&mut builder.widgets, *id) {
                    widget.kind = normalized_kind(kind.clone());
                }
            }
            GuiBuilderOperation::SetTitle { title } => builder.title.clone_from(title),
            GuiBuilderOperation::SetCanvasSize { canvas } => {
                builder.canvas = normalized_canvas(*canvas);
            }
        }
    }

    fn implicit_name(&self) -> String {
        if self.title.trim().is_empty() {
            "GUI".into()
        } else {
            self.title.clone()
        }
    }
}

fn find(widgets: &[GuiWidget], id: Uuid) -> Option<&GuiWidget> {
    widgets.iter().find_map(|widget| {
        if widget.id == id {
            Some(widget)
        } else {
            find(&widget.children, id)
        }
    })
}

fn find_mut(widgets: &mut [GuiWidget], id: Uuid) -> Option<&mut GuiWidget> {
    widgets.iter_mut().find_map(|widget| {
        if widget.id == id {
            Some(widget)
        } else {
            find_mut(&mut widget.children, id)
        }
    })
}

fn locate(widgets: &[GuiWidget], parent: Option<Uuid>, id: Uuid) -> Option<GuiLocation> {
    if let Some(index) = widgets.iter().position(|widget| widget.id == id) {
        return Some(GuiLocation::new(parent, index));
    }
    widgets
        .iter()
        .find_map(|widget| locate(&widget.children, Some(widget.id), id))
}

fn detach_from(widgets: &mut Vec<GuiWidget>, id: Uuid) -> Option<GuiWidget> {
    if let Some(index) = widgets.iter().position(|widget| widget.id == id) {
        return Some(widgets.remove(index));
    }
    widgets
        .iter_mut()
        .find_map(|widget| detach_from(&mut widget.children, id))
}

fn contains(widget: &GuiWidget, id: Uuid) -> bool {
    widget.id == id || find(&widget.children, id).is_some()
}

fn collect_ids(widget: &GuiWidget, ids: &mut Vec<Uuid>) {
    ids.push(widget.id);
    for child in &widget.children {
        collect_ids(child, ids);
    }
}

fn normalized_kind(mut kind: GuiWidgetKind) -> GuiWidgetKind {
    match &mut kind {
        GuiWidgetKind::Slider {
            value, min, max, ..
        } => {
            if !min.is_finite() {
                *min = 0.0;
            }
            if !max.is_finite() || *max <= *min {
                *max = *min + 1.0;
            }
            if !value.is_finite() {
                *value = *min;
            }
            *value = value.clamp(*min, *max);
        }
        GuiWidgetKind::Space { height } => {
            if !height.is_finite() {
                *height = 0.0;
            }
            *height = height.clamp(0.0, MAX_SPACE_HEIGHT);
        }
        _ => {}
    }
    kind
}

fn normalized_canvas(mut canvas: GuiCanvasSize) -> GuiCanvasSize {
    if !canvas.width.is_finite() {
        canvas.width = GuiCanvasSize::default().width;
    }
    if !canvas.height.is_finite() {
        canvas.height = GuiCanvasSize::default().height;
    }
    canvas.width = canvas.width.clamp(MIN_CANVAS_SIZE, MAX_CANVAS_SIZE);
    canvas.height = canvas.height.clamp(MIN_CANVAS_SIZE, MAX_CANVAS_SIZE);
    canvas
}

pub struct GuiBuilderHistory;

pub struct GuiBuilderHistoryAction {
    changes: Vec<GuiBuilderHistoryChange>,
    recorded_at: Instant,
}

/// Each change carries the operation that reverses it and the one that
/// replays it, which keeps undo and redo symmetric for every operation kind.
struct GuiBuilderHistoryChange {
    undo: GuiBuilderOperation,
    redo: GuiBuilderOperation,
}

impl BlockHistory<GuiBuilder> for GuiBuilderHistory {
    type Action = GuiBuilderHistoryAction;
    type Snapshot = GuiBuilder;

    fn snapshot(block: &GuiBuilder) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: GuiBuilder,
        _after: &GuiBuilder,
        operations: &[GuiBuilderOperation],
    ) -> Option<Self::Action> {
        let mut current = before;
        let mut changes = Vec::new();
        for operation in operations {
            let previous = current.clone();
            let change = reversal(&previous, operation);
            GuiBuilder::apply_operation(&mut current, operation);
            if current == previous {
                continue;
            }
            if let Some(undo) = change {
                changes.push(GuiBuilderHistoryChange {
                    undo,
                    redo: operation.clone(),
                });
            }
        }
        (!changes.is_empty()).then_some(GuiBuilderHistoryAction {
            changes,
            recorded_at: Instant::now(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action.changes.len() * 512
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        if next.recorded_at.duration_since(previous.recorded_at) > EDIT_BURST_DELAY {
            return Err(next);
        }
        let can_merge = match (previous.changes.as_slice(), next.changes.as_slice()) {
            ([previous], [next]) => mergeable(previous, next),
            _ => false,
        };
        if !can_merge {
            return Err(next);
        }
        let recorded_at = next.recorded_at;
        for (target, source) in previous.changes.iter_mut().zip(next.changes) {
            target.redo = source.redo;
        }
        previous.recorded_at = recorded_at;
        Ok(())
    }

    fn operations(
        _current: &GuiBuilder,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<GuiBuilderOperation> {
        match direction {
            HistoryDirection::Undo => action
                .changes
                .iter()
                .rev()
                .map(|change| change.undo.clone())
                .collect(),
            HistoryDirection::Redo => action
                .changes
                .iter()
                .map(|change| change.redo.clone())
                .collect(),
        }
    }
}

/// The operation that puts `builder` back the way it was before `operation`,
/// or `None` when the operation cannot be reversed from this state.
fn reversal(builder: &GuiBuilder, operation: &GuiBuilderOperation) -> Option<GuiBuilderOperation> {
    match operation {
        GuiBuilderOperation::Insert { widget, .. } => {
            Some(GuiBuilderOperation::Remove { id: widget.id })
        }
        GuiBuilderOperation::Remove { id } => Some(GuiBuilderOperation::Insert {
            location: builder.location(*id)?,
            widget: builder.widget(*id)?.clone(),
        }),
        GuiBuilderOperation::Move { id, .. } => Some(GuiBuilderOperation::Move {
            id: *id,
            location: builder.location(*id)?,
        }),
        GuiBuilderOperation::SetKind { id, .. } => Some(GuiBuilderOperation::SetKind {
            id: *id,
            kind: builder.widget(*id)?.kind.clone(),
        }),
        GuiBuilderOperation::SetTitle { .. } => Some(GuiBuilderOperation::SetTitle {
            title: builder.title.clone(),
        }),
        GuiBuilderOperation::SetCanvasSize { .. } => Some(GuiBuilderOperation::SetCanvasSize {
            canvas: builder.canvas,
        }),
    }
}

/// Typing and dragging produce a stream of edits to one property; those
/// collapse into a single history entry.
fn mergeable(previous: &GuiBuilderHistoryChange, next: &GuiBuilderHistoryChange) -> bool {
    match (&previous.redo, &next.redo) {
        (
            GuiBuilderOperation::SetKind { id: previous, .. },
            GuiBuilderOperation::SetKind { id: next, .. },
        ) => previous == next,
        (GuiBuilderOperation::SetTitle { .. }, GuiBuilderOperation::SetTitle { .. })
        | (GuiBuilderOperation::SetCanvasSize { .. }, GuiBuilderOperation::SetCanvasSize { .. }) => {
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;

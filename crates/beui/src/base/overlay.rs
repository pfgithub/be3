use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::geometry::{pos2, Pos2, Rect, Vec2};
use crate::input::{CursorIcon, PointerPress};
use crate::painter::Painter;

use beui_macros::{component, view};

use crate::document::Document;
use crate::node::{ClickHandler, Element, InteractInput, NodeId};
use crate::reactive::{
    with_document, with_reactive_scope, Children, ClickCallback, ClickCatcherBuilder,
};

#[derive(Clone, Copy)]
pub(crate) enum OverlayAnchor {
    Node(NodeId),
    Point(Pos2),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    BelowStart,
    RightStart,
}

pub(crate) struct OverlayNode {
    content: Option<NodeId>,
    scrim: NodeId,
    open: bool,
    anchor: OverlayAnchor,
    placement: Placement,
    on_dismiss: Option<ClickHandler>,
}

impl OverlayNode {
    fn new(scrim: NodeId, anchor: OverlayAnchor, placement: Placement) -> Self {
        Self {
            content: None,
            scrim,
            open: false,
            anchor,
            placement,
            on_dismiss: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
}

fn resolve_rect(
    viewport: Rect,
    anchor_rect: Rect,
    placement: Placement,
    content_size: Vec2,
) -> Rect {
    let mut origin = match placement {
        Placement::BelowStart => pos2(anchor_rect.left(), anchor_rect.bottom()),
        Placement::RightStart => pos2(anchor_rect.right(), anchor_rect.top()),
    };
    if origin.x + content_size.x > viewport.right() {
        origin.x = match placement {
            Placement::RightStart => anchor_rect.left() - content_size.x,
            Placement::BelowStart => (viewport.right() - content_size.x).max(viewport.left()),
        };
    }
    if origin.y + content_size.y > viewport.bottom() {
        origin.y = match placement {
            Placement::BelowStart => anchor_rect.top() - content_size.y,
            Placement::RightStart => (viewport.bottom() - content_size.y).max(viewport.top()),
        };
    }
    origin.x = origin.x.max(viewport.left());
    origin.y = origin.y.max(viewport.top());
    Rect::from_min_size(origin, content_size)
}

impl Element for OverlayNode {
    fn measure(&self, _doc: &Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        _rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if !self.open {
            return;
        }
        let Some(content) = self.content else {
            return;
        };
        let viewport = doc.viewport_rect();
        crate::layout::layout(doc, painter, self.scrim, viewport, out);
        let content_size = crate::layout::measure(doc, painter, content, viewport.size());
        let anchor_rect = match self.anchor {
            OverlayAnchor::Node(id) => out.get(&id).copied().unwrap_or(viewport),
            OverlayAnchor::Point(pos) => Rect::from_min_size(pos, Vec2::ZERO),
        };
        let rect = resolve_rect(viewport, anchor_rect, self.placement, content_size);
        crate::layout::layout(doc, painter, content, rect, out);
    }

    fn paint(
        &self,
        _doc: &Document,
        _painter: &Painter,
        _rects: &HashMap<NodeId, Rect>,
        _rect: Rect,
    ) {
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        if !self.open {
            return Vec::new();
        }
        let mut children = vec![self.scrim];
        children.extend(self.content);
        children
    }

    fn children(&self) -> Vec<NodeId> {
        let mut children = vec![self.scrim];
        children.extend(self.content);
        children
    }

    fn kind(&self) -> &'static str {
        "overlay"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.open { "open" } else { "closed" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[component(base)]
pub(crate) fn overlay(
    anchor: OverlayAnchor,
    placement: Placement,
    on_dismiss: ClickCallback,
    children: Children,
) -> NodeId {
    let content = children.into_first();
    with_document(|document| {
        let overlay = document.create_overlay(anchor, placement);
        if let Some(content) = content {
            document.set_overlay_content(overlay, content);
        }
        if !on_dismiss.is_empty() {
            document.set_overlay_on_dismiss(overlay, move || on_dismiss.call());
        }
        overlay
    })
}

pub(crate) fn open_overlay(overlay: NodeId) {
    with_document(|document| document.open_overlay(overlay));
}

pub(crate) fn close_overlay(overlay: NodeId) {
    with_document(|document| document.close_overlay(overlay));
}

pub(crate) fn overlay_is_open(overlay: NodeId) -> bool {
    with_document(|document| document.is_overlay_open(overlay))
}

pub(crate) fn move_overlay_to(overlay: NodeId, pos: Pos2) {
    with_document(|document| document.set_overlay_anchor(overlay, OverlayAnchor::Point(pos)));
}

pub(crate) fn replace_overlay_content(overlay: NodeId, content: NodeId) {
    with_document(|document| {
        if let Some(previous) = document.overlay_content(overlay) {
            document.remove_node(previous);
        }
        document.set_overlay_content(overlay, content);
    });
}

impl Document {
    pub(crate) fn create_overlay(&mut self, anchor: OverlayAnchor, placement: Placement) -> NodeId {
        let overlay_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let press_cell = overlay_cell.clone();
        let scrim = with_reactive_scope(self, || {
            view! {
                <click_catcher
                    cursor={CursorIcon::Default}
                    on_press={move |press: PointerPress| {
                        let id = press_cell.get().expect("overlay not yet initialized");
                        with_document(|document| document.dismiss_overlay_if_outside(id, press.pos));
                    }}
                ></click_catcher>
            }
        });
        let id = self
            .arena
            .insert(OverlayNode::new(scrim, anchor, placement));
        overlay_cell.set(Some(id));
        id
    }

    pub(crate) fn set_overlay_content(&mut self, overlay: NodeId, content: NodeId) {
        self.arena.get_mut_as::<OverlayNode>(overlay).content = Some(content);
    }

    pub(crate) fn set_overlay_anchor(&mut self, overlay: NodeId, anchor: OverlayAnchor) {
        self.arena.get_mut_as::<OverlayNode>(overlay).anchor = anchor;
    }

    pub(crate) fn set_overlay_on_dismiss(
        &mut self,
        overlay: NodeId,
        handler: impl FnMut() + 'static,
    ) {
        self.arena.get_mut_as::<OverlayNode>(overlay).on_dismiss = Some(Box::new(handler));
    }

    pub(crate) fn is_overlay_open(&self, overlay: NodeId) -> bool {
        self.arena.get_as::<OverlayNode>(overlay).open
    }

    pub(crate) fn overlay_content(&self, overlay: NodeId) -> Option<NodeId> {
        self.arena.get_as::<OverlayNode>(overlay).content
    }

    pub(crate) fn open_overlay(&mut self, overlay: NodeId) {
        if self.arena.get_as::<OverlayNode>(overlay).open {
            return;
        }
        self.arena.get_mut_as::<OverlayNode>(overlay).open = true;
        self.overlay_stack.push(overlay);
        self.arena.invalidate();
    }

    pub(crate) fn close_overlay(&mut self, overlay: NodeId) {
        if let Some(level) = self.overlay_stack.iter().position(|&id| id == overlay) {
            self.close_overlay_at(level);
        }
    }

    pub(crate) fn close_topmost_overlay(&mut self) {
        if !self.overlay_stack.is_empty() {
            self.close_overlay_at(self.overlay_stack.len() - 1);
        }
    }

    fn close_overlay_at(&mut self, level: usize) {
        let closing: Vec<NodeId> = self.overlay_stack.split_off(level);
        for id in closing {
            if self.contains(id) {
                self.arena.get_mut_as::<OverlayNode>(id).open = false;
            }
            self.call_overlay_dismiss(id);
        }
        self.arena.invalidate();
    }

    fn call_overlay_dismiss(&mut self, id: NodeId) {
        if !self.contains(id) {
            return;
        }
        let mut element = self.arena.take(id);
        let handler = element
            .as_any_mut()
            .downcast_mut::<OverlayNode>()
            .and_then(|node| node.on_dismiss.take());
        self.arena.put_back(id, element);
        if let Some(mut handler) = handler {
            handler();
            if self.contains(id) {
                let node = self.arena.get_mut_as::<OverlayNode>(id);
                if node.on_dismiss.is_none() {
                    node.on_dismiss = Some(handler);
                }
            }
        }
    }

    fn dismiss_overlay_if_outside(&mut self, overlay: NodeId, pos: Pos2) {
        let Some(level) = self.overlay_stack.iter().position(|&id| id == overlay) else {
            return;
        };
        let inside_any = self.overlay_stack[level..].iter().any(|&id| {
            self.overlay_content(id)
                .and_then(|content| self.node_rect(content))
                .is_some_and(|rect| rect.contains(pos))
        });
        if !inside_any {
            self.close_overlay_at(level);
        }
    }
}

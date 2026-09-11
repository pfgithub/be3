use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::with_document;

pub(crate) struct ShadowNode {
    pub(crate) name: &'static str,
    pub(crate) shadow_root: NodeId,
    pub(crate) slots: Vec<NodeId>,
    pub(crate) state: Option<Box<dyn Any>>,
    pub(crate) detail: Option<String>,
}

impl Element for ShadowNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        crate::layout::measure(doc, painter, self.shadow_root, available)
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        crate::layout::layout(doc, painter, self.shadow_root, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        crate::paint::paint(doc, painter, rects, self.shadow_root);
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
        vec![self.shadow_root]
    }

    fn children(&self) -> Vec<NodeId> {
        vec![self.shadow_root]
    }

    fn kind(&self) -> &'static str {
        self.name
    }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) struct SlotNode {
    pub(crate) name: &'static str,
    pub(crate) content: Option<NodeId>,
}

impl Element for SlotNode {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.content {
            Some(content) => crate::layout::measure(doc, painter, content, available),
            None => Vec2::ZERO,
        }
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(content) = self.content {
            crate::layout::layout(doc, painter, content, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, _rect: Rect) {
        if let Some(content) = self.content {
            crate::paint::paint(doc, painter, rects, content);
        }
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
        self.content.into_iter().collect()
    }

    fn children(&self) -> Vec<NodeId> {
        self.content.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) fn shadow_root(shadow: NodeId) -> NodeId {
    with_document(|document| document.shadow_root(shadow))
}

impl Document {
    pub fn create_slot(&mut self, name: &'static str) -> NodeId {
        self.arena.insert(SlotNode {
            name,
            content: None,
        })
    }

    pub fn create_shadow(
        &mut self,
        name: &'static str,
        shadow_root: NodeId,
        slots: Vec<NodeId>,
    ) -> NodeId {
        self.arena.insert(ShadowNode {
            name,
            shadow_root,
            slots,
            state: None,
            detail: None,
        })
    }

    pub(crate) fn reserve_shadow(&mut self) -> NodeId {
        self.arena.reserve()
    }

    pub(crate) fn finish_shadow(
        &mut self,
        shadow: NodeId,
        name: &'static str,
        shadow_root: NodeId,
        slots: Vec<NodeId>,
    ) {
        self.arena.fill(
            shadow,
            ShadowNode {
                name,
                shadow_root,
                slots,
                state: None,
                detail: None,
            },
        );
    }

    pub fn set_slot_child(&mut self, slot: NodeId, child: NodeId) {
        if self.arena.get_as::<SlotNode>(slot).content != Some(child) {
            self.arena.get_mut_as::<SlotNode>(slot).content = Some(child);
        }
    }

    pub fn set_shadow_child(&mut self, shadow: NodeId, child: NodeId) {
        let slots = &self.arena.get_as::<ShadowNode>(shadow).slots;
        let [slot] = slots[..] else {
            panic!("shadow has more than one slot, use set_slot_child");
        };
        self.set_slot_child(slot, child);
    }

    pub fn shadow_root(&self, shadow: NodeId) -> NodeId {
        self.arena.get_as::<ShadowNode>(shadow).shadow_root
    }

    pub fn shadow_slots(&self, shadow: NodeId) -> Vec<NodeId> {
        self.arena.get_as::<ShadowNode>(shadow).slots.clone()
    }

    pub fn set_component_state<T: 'static>(&mut self, shadow: NodeId, state: T) {
        self.arena.get_mut_as::<ShadowNode>(shadow).state = Some(Box::new(state));
    }

    pub(crate) fn set_component_state_dyn(&mut self, shadow: NodeId, state: Box<dyn Any>) {
        self.arena.get_mut_as::<ShadowNode>(shadow).state = Some(state);
    }

    pub fn set_component_detail(&mut self, shadow: NodeId, detail: impl Into<String>) {
        let value = Some(detail.into());
        if self.arena.get_as::<ShadowNode>(shadow).detail != value {
            self.arena.get_mut_as::<ShadowNode>(shadow).detail = value;
        }
    }

    pub fn component_state<T: 'static>(&self, shadow: NodeId) -> &T {
        self.arena
            .get_as::<ShadowNode>(shadow)
            .state
            .as_ref()
            .and_then(|state| state.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("component has no {} state", std::any::type_name::<T>()))
    }

    pub fn component_state_mut<T: 'static>(&mut self, shadow: NodeId) -> &mut T {
        self.arena
            .get_mut_as::<ShadowNode>(shadow)
            .state
            .as_mut()
            .and_then(|state| state.downcast_mut::<T>())
            .unwrap_or_else(|| panic!("component has no {} state", std::any::type_name::<T>()))
    }

    pub(crate) fn as_shadow(&self, id: NodeId) -> Option<&ShadowNode> {
        self.arena.get(id).as_any().downcast_ref::<ShadowNode>()
    }

    pub(crate) fn as_slot(&self, id: NodeId) -> Option<&SlotNode> {
        self.arena.get(id).as_any().downcast_ref::<SlotNode>()
    }
}

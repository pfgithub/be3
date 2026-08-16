use std::any::Any;
use std::collections::HashMap;

use egui::{Painter, Pos2, Rect, Vec2};

use crate::document::Document;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(u32);

pub(crate) struct InteractInput {
    pub(crate) pointer_pos: Option<Pos2>,
    pub(crate) pressed_this_frame: bool,
    pub(crate) released_this_frame: bool,
    pub(crate) scroll_delta: f32,
}

pub(crate) trait Element: Any {
    fn measure(&self, doc: &Document, painter: &Painter) -> Vec2;

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    );

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect);

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId>;

    fn children(&self) -> Vec<NodeId>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Default)]
pub(crate) struct Arena {
    nodes: Vec<Option<Box<dyn Element>>>,
}

impl Arena {
    pub(crate) fn insert<T: Element>(&mut self, element: T) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Some(Box::new(element)));
        id
    }

    pub(crate) fn get(&self, id: NodeId) -> &dyn Element {
        self.nodes[id.0 as usize]
            .as_deref()
            .expect("node was removed")
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.nodes[id.0 as usize]
            .as_deref_mut()
            .expect("node was removed")
    }

    pub(crate) fn get_as<T: Element>(&self, id: NodeId) -> &T {
        self.get(id)
            .as_any()
            .downcast_ref::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub(crate) fn get_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.get_mut(id)
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub(crate) fn take(&mut self, id: NodeId) -> Box<dyn Element> {
        self.nodes[id.0 as usize].take().expect("node was removed")
    }

    pub(crate) fn put_back(&mut self, id: NodeId, element: Box<dyn Element>) {
        self.nodes[id.0 as usize] = Some(element);
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        self.nodes[id.0 as usize] = None;
    }
}

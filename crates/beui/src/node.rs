use std::any::Any;
use std::collections::HashMap;

use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::Modifiers;
use crate::painter::Painter;

use crate::document::Document;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(u32);

pub(crate) struct InteractInput {
    pub(crate) pointer_pos: Option<Pos2>,
    pub(crate) pointer_down: bool,
    pub(crate) pressed_this_frame: bool,
    pub(crate) released_this_frame: bool,
    pub(crate) scroll_delta: f32,
    pub(crate) clicks: u32,
    pub(crate) modifiers: Modifiers,
}

pub type Handler<V> = Box<dyn FnMut(&mut Document, V)>;
pub type ClickHandler = Box<dyn FnMut(&mut Document)>;
pub(crate) type ChangeHandler = Handler<bool>;

pub(crate) trait Element: Any {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2;

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

    fn kind(&self) -> &'static str;

    fn detail(&self) -> Option<String> {
        None
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Default)]
pub(crate) struct Arena {
    nodes: Vec<Option<Box<dyn Element>>>,
    pub(crate) revision: u64,
}

impl Arena {
    pub(crate) fn insert<T: Element>(&mut self, element: T) -> NodeId {
        self.invalidate();
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Some(Box::new(element)));
        id
    }

    pub(crate) fn contains(&self, id: NodeId) -> bool {
        self.nodes
            .get(id.0 as usize)
            .is_some_and(std::option::Option::is_some)
    }

    pub(crate) fn get(&self, id: NodeId) -> &dyn Element {
        self.nodes[id.0 as usize]
            .as_deref()
            .expect("node was removed")
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.invalidate();
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

    pub(crate) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        self.invalidate();
        self.nodes[id.0 as usize] = None;
    }
}

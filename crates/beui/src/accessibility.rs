use std::sync::atomic::{AtomicU32, Ordering};

use accesskit::{
    Action, ActionData, ActionRequest, Affine, Node, NodeId as AccessNodeId, Rect as AccessRect,
    Role, Tree, TreeId, TreeUpdate,
};

use crate::base::focusable::FocusableNode;
use crate::base::overlay::OverlayNode;
use crate::base::scroll::ScrollNode;
use crate::base::text::TextNode;
use crate::base::visibility::VisibilityNode;
use crate::geometry::{Rect, Vec2};
use crate::input::{Key, KeyPress, Modifiers};
use crate::node::NodeId;
use crate::Document;

pub(crate) const WINDOW_NODE: AccessNodeId = AccessNodeId(0);
const DOCUMENT_SHIFT: u32 = 32;
static NEXT_DOCUMENT_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub(crate) struct Fragment {
    pub(crate) nodes: Vec<(AccessNodeId, Node)>,
    pub(crate) root: AccessNodeId,
    pub(crate) focus: Option<AccessNodeId>,
}

pub(crate) fn next_document_id() -> u32 {
    NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn tree_update(
    title: &str,
    viewport: Vec2,
    scale: f32,
    fragments: Vec<Fragment>,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_bounds(AccessRect::new(
        0.0,
        0.0,
        viewport.x.into(),
        viewport.y.into(),
    ));
    root.set_transform(Affine::scale(scale.into()));
    root.set_children(
        fragments
            .iter()
            .map(|fragment| fragment.root)
            .collect::<Vec<_>>(),
    );
    let focus = fragments
        .iter()
        .find_map(|fragment| fragment.focus)
        .unwrap_or(WINDOW_NODE);
    let mut nodes = vec![(WINDOW_NODE, root)];
    nodes.extend(fragments.into_iter().flat_map(|fragment| fragment.nodes));
    TreeUpdate {
        nodes,
        tree: Some(Tree {
            root: WINDOW_NODE,
            toolkit_name: Some("beui".to_owned()),
            toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        tree_id: TreeId::ROOT,
        focus,
    }
}

impl Document {
    pub(crate) fn accessibility_fragment(&self) -> Option<Fragment> {
        let root = self.root?;
        let mut nodes = Vec::new();
        let mut focused_accessible = None;
        let roots = self.accessibility_subtree(root, true, &mut nodes, &mut focused_accessible);
        let root = roots.into_iter().next()?;
        Some(Fragment {
            nodes,
            root,
            focus: focused_accessible,
        })
    }

    fn accessibility_subtree(
        &self,
        id: NodeId,
        force: bool,
        out: &mut Vec<(AccessNodeId, Node)>,
        focused_accessible: &mut Option<AccessNodeId>,
    ) -> Vec<AccessNodeId> {
        let Some(rect) = self.rects.get(&id).copied() else {
            return Vec::new();
        };
        let element = self.arena.get(id);
        if element
            .as_any()
            .downcast_ref::<VisibilityNode>()
            .is_some_and(|node| !node.visible)
            || element
                .as_any()
                .downcast_ref::<OverlayNode>()
                .is_some_and(|node| !node.is_open())
        {
            return Vec::new();
        }

        let mut children = Vec::new();
        for child in element.children() {
            children.extend(self.accessibility_subtree(child, false, out, focused_accessible));
        }

        let explicit = self.accessibility.get(&id);
        let text = element
            .as_any()
            .downcast_ref::<TextNode>()
            .and_then(TextNode::accessible_text);
        let scroll = element.as_any().downcast_ref::<ScrollNode>();
        if explicit.is_none() && text.is_none() && scroll.is_none() && !force {
            return children;
        }

        let mut node = if let Some(node) = explicit {
            node.clone()
        } else if let Some(text) = text {
            let mut node = Node::new(Role::Label);
            node.set_value(text);
            node
        } else if let Some(scroll) = scroll {
            let mut node = Node::new(Role::ScrollView);
            if let Some(position) = scroll.position {
                node.set_scroll_y(position.offset.into());
                node.set_scroll_y_min(0.0);
                node.set_scroll_y_max(position.max_offset().into());
                node.add_action(Action::ScrollUp);
                node.add_action(Action::ScrollDown);
                node.add_action(Action::SetScrollOffset);
            }
            node
        } else {
            Node::new(Role::GenericContainer)
        };
        node.set_bounds(access_rect(rect));
        node.set_children(children);

        if explicit.is_some() {
            if node.is_disabled() {
                node.clear_actions();
            } else {
                if matches!(node.role(), Role::Slider | Role::SpinButton) {
                    node.add_action(Action::Increment);
                    node.add_action(Action::Decrement);
                    node.add_action(Action::SetValue);
                }
                if is_text_input(node.role()) {
                    node.add_action(Action::ReplaceSelectedText);
                    node.add_action(Action::SetValue);
                }
                let bridges_focus = node.supports_action(Action::Focus)
                    || node.supports_action(Action::Click)
                    || is_focusable_control(node.role());
                if bridges_focus {
                    if let Some(focusable) = self.first_focusable_within(id) {
                        node.add_action(Action::Focus);
                        if self.focusable_can_activate(focusable) {
                            node.add_action(Action::Click);
                        }
                        if self.focused == Some(focusable) {
                            *focused_accessible = Some(self.access_node_id(id));
                        }
                    }
                }
            }
        } else if self.focused == Some(id) {
            node.add_action(Action::Focus);
            *focused_accessible = Some(self.access_node_id(id));
        }

        let access_id = self.access_node_id(id);
        out.push((access_id, node));
        vec![access_id]
    }

    pub(crate) fn handle_accessibility_action(&mut self, request: ActionRequest) -> bool {
        let Some(target) = self.local_node_id(request.target_node) else {
            return false;
        };
        if !self.contains(target) {
            return true;
        }
        if self
            .accessibility
            .get(&target)
            .is_some_and(Node::is_disabled)
        {
            return true;
        }
        let focusable = self.first_focusable_within(target);
        match request.action {
            Action::Focus => {
                if let Some(focusable) = focusable {
                    self.update_focus(Some(focusable));
                }
            }
            Action::Blur => {
                if focusable == self.focused {
                    self.update_focus(None);
                }
            }
            Action::Click => {
                if let Some(focusable) = focusable {
                    self.update_focus(Some(focusable));
                    self.activate_focusable(focusable);
                }
            }
            Action::Increment => {
                if let Some(focusable) = focusable {
                    self.step_focusable(focusable, 1.0);
                }
            }
            Action::Decrement => {
                if let Some(focusable) = focusable {
                    self.step_focusable(focusable, -1.0);
                }
            }
            Action::ReplaceSelectedText => {
                if let (Some(focusable), Some(ActionData::Value(value))) = (focusable, request.data)
                {
                    self.update_focus(Some(focusable));
                    self.text_focused(&value);
                }
            }
            Action::SetValue => match (focusable, request.data) {
                (Some(focusable), Some(ActionData::Value(value))) => {
                    self.update_focus(Some(focusable));
                    self.key_focused(KeyPress {
                        key: Key::A,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::CTRL,
                    });
                    self.text_focused(&value);
                }
                (Some(focusable), Some(ActionData::NumericValue(value))) => {
                    let current = self
                        .accessibility
                        .get(&target)
                        .and_then(Node::numeric_value)
                        .unwrap_or(value);
                    self.step_focusable(focusable, ((value - current) / 0.05) as f32);
                }
                _ => {}
            },
            Action::ScrollUp | Action::ScrollDown | Action::SetScrollOffset => {
                if let Some(scroll) = self.first_scroll_within(target) {
                    let position = self.arena.get_as::<ScrollNode>(scroll).position;
                    if let Some(position) = position {
                        let offset = match (request.action, request.data) {
                            (Action::ScrollUp, _) => position.offset - position.viewport,
                            (Action::ScrollDown, _) => position.offset + position.viewport,
                            (Action::SetScrollOffset, Some(ActionData::SetScrollOffset(point))) => {
                                point.y as f32
                            }
                            _ => position.offset,
                        };
                        self.set_scroll_offset(scroll, offset.clamp(0.0, position.max_offset()));
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn access_node_id(&self, id: NodeId) -> AccessNodeId {
        AccessNodeId(((self.accessibility_id as u64) << DOCUMENT_SHIFT) | id.index() as u64)
    }

    fn local_node_id(&self, id: AccessNodeId) -> Option<NodeId> {
        ((id.0 >> DOCUMENT_SHIFT) == self.accessibility_id as u64)
            .then(|| NodeId::from_index(id.0 as u32))
    }

    fn first_focusable_within(&self, id: NodeId) -> Option<NodeId> {
        let element = self.arena.get(id);
        if element.as_any().is::<FocusableNode>() || element.as_any().is::<ScrollNode>() {
            return Some(id);
        }
        element
            .children()
            .into_iter()
            .find_map(|child| self.first_focusable_within(child))
    }

    fn first_scroll_within(&self, id: NodeId) -> Option<NodeId> {
        let element = self.arena.get(id);
        if element.as_any().is::<ScrollNode>() {
            return Some(id);
        }
        element
            .children()
            .into_iter()
            .find_map(|child| self.first_scroll_within(child))
    }

    fn focusable_can_activate(&self, id: NodeId) -> bool {
        self.arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .is_some_and(|node| !node.on_activate.is_empty())
    }
}

fn access_rect(rect: Rect) -> AccessRect {
    AccessRect::new(
        rect.left().into(),
        rect.top().into(),
        rect.right().into(),
        rect.bottom().into(),
    )
}

fn is_text_input(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
    )
}

fn is_focusable_control(role: Role) -> bool {
    matches!(
        role,
        Role::TreeItem
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::CheckBox
            | Role::RadioButton
            | Role::Button
            | Role::DefaultButton
            | Role::Switch
            | Role::ComboBox
            | Role::EditableComboBox
            | Role::DisclosureTriangle
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::Slider
            | Role::SpinButton
            | Role::Tab
            | Role::Link
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
    )
}

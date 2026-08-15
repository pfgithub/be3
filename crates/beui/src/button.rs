use crate::document::Document;
use crate::node::NodeId;

pub(crate) type ChangeHandler = Box<dyn FnMut(&mut Document, bool)>;
pub(crate) type ClickHandler = Box<dyn FnMut(&mut Document)>;

pub(crate) struct ButtonNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) armed: bool,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) on_click: Option<ClickHandler>,
    pub(crate) on_hover_change: Option<ChangeHandler>,
    pub(crate) on_active_change: Option<ChangeHandler>,
    pub(crate) on_focus_change: Option<ChangeHandler>,
}

impl ButtonNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            armed: false,
            hovered: false,
            active: false,
            focused: false,
            on_click: None,
            on_hover_change: None,
            on_active_change: None,
            on_focus_change: None,
        }
    }
}

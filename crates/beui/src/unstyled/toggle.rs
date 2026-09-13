use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, clone, component_accessibility, create_effect, create_memo, create_signal,
    set_component_state, untrack, Callback, ClickCatcher, Focusable, Prop, ReadSignal, Render,
};

pub struct ToggleHandle {
    pub checked: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn Toggle(
    checked: Prop<bool>,
    #[prop(children)] content: Option<Render<ToggleHandle>>,
    on_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let (checked_read, set_checked) = create_signal(checked.peek());
    create_effect(clone!(set_checked -> move || set_checked.set(checked.get())));
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::CheckBox)));
    component_accessibility(create_memo(clone!(checked_read -> move || {
        let mut node = accessibility.get();
        node.set_toggled(Toggled::from(checked_read.get()));
        node
    })));

    let content_node = content.map(|build| {
        build.call(ToggleHandle {
            checked: checked_read.clone(),
            hovered: hovered.clone(),
            active: active.clone(),
            focused: focused.clone(),
        })
    });

    let toggle_checked = {
        let checked = checked_read.clone();
        move || {
            let next = !untrack(|| checked.get());
            set_checked.set(next);
            on_change.call(next);
        }
    };
    let key_toggle = toggle_checked.clone();

    set_component_state(checked_read.clone());

    view! {
        <Focusable
            on_focus_change={move |focused: bool| set_focused.set(focused)}
            on_activate_change={move |pressed: bool| set_key_active.set(pressed)}
            on_activate={key_toggle}
        >
            <ClickCatcher
                cursor=CursorIcon::PointingHand
                key_active
                on_click={toggle_checked}
                on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
                on_active_change={move |active: bool| set_active.set(active)}
                children={content_node.map(reactive::intrinsic)}
            />
        </Focusable>
    }
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<ReadSignal<bool>>(toggle).clone()
}

use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayAnchor, Placement};
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    create_memo, create_signal, set_component_state, Callback, Child, ClickCatcher, Column,
    Dynamic, NodeRef, Prop, RenderFn,
};
use crate::unstyled::menu::{MenuItem, MenuList, MenuRowHandle};

struct State {
    overlay: NodeRef,
    content: NodeRef,
}

#[component]
pub fn context_menu(
    children: Child,
    items: Prop<Vec<MenuItem>>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.unwrap_or_else(|| RenderFn::new(|_| view! { <Column spacing=0.0 /> }));
    let panel = panel.unwrap_or_else(|| RenderFn::new(|content| content));
    let (open, set_open) = create_signal(false);
    let (position, set_position) = create_signal(Pos2::ZERO);
    let anchor = create_memo(move || OverlayAnchor::Point(position.get()));
    let (overlay, content) = (NodeRef::new(), NodeRef::new());
    set_component_state(State {
        overlay: overlay.clone(),
        content: content.clone(),
    });

    let dismiss = set_open.clone();
    let (active, close) = (open.clone(), set_open.clone());
    view! {
        <ClickCatcher
            cursor=CursorIcon::Default
            on_secondary_press={move |press: PointerPress| {
                set_position.set(press.pos);
                set_open.set(true);
            }}
        >
            <Column spacing=0.0>
                {children}
                <Overlay
                    @node_ref=&overlay
                    anchor
                    placement=Placement::BelowStart
                    open
                    on_dismiss={move || dismiss.set(false)}
                >
                    <Dynamic value={items}>
                        {move |items: Vec<MenuItem>| {
                            let (row, panel, close) = (row.clone(), panel.clone(), close.clone());
                            let content = content.clone();
                            let on_select = on_select.clone();
                            panel.call(view! {
                                <MenuList
                                    @node_ref=&content
                                    items
                                    row
                                    panel={panel.clone()}
                                    active={active.clone()}
                                    on_select={move |path: Vec<usize>| {
                                        on_select.call(path);
                                        close.set(false);
                                    }}
                                />
                            })
                        }}
                    </Dynamic>
                </Overlay>
            </Column>
        </ClickCatcher>
    }
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .content
        .get()
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .overlay
        .get()
}

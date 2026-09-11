use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::base::overlay::{
    close_overlay, move_overlay_to, open_overlay, replace_overlay_content, OverlayAnchor,
    OverlayBuilder, Placement,
};
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    in_new_scope, set_component_state, Callback, ClickCatcherBuilder, ColumnBuilder, NodeRef, Prop,
};
use crate::unstyled::menu::{self, MenuItem, MenuListBuilder, MenuPanel, MenuRow};

struct State {
    overlay: NodeId,
    content: Rc<Cell<Option<NodeId>>>,
}

#[component]
pub fn context_menu(
    region: NodeId,
    items: Prop<Vec<MenuItem>>,
    row: Option<MenuRow>,
    panel: Option<MenuPanel>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.unwrap_or_else(|| Rc::new(|_| view! { <column spacing={0.0} /> }));
    let panel = panel.unwrap_or_else(|| Rc::new(|content| content));
    let overlay = NodeRef::new();
    let content: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));

    let catcher = view! {
        <click_catcher
            cursor={CursorIcon::Default}
            on_secondary_press={{
                let (overlay, content) = (overlay.clone(), content.clone());
                move |press: PointerPress| {
                    move_overlay_to(overlay.get(), press.pos);
                    open_overlay(overlay.get());
                    if let Some(content) = content.get() {
                        menu::focus_menu_list_root(content);
                    }
                }
            }}
        >
            <column spacing={0.0}>
                {region}
                <overlay
                    node_ref={&overlay}
                    anchor={OverlayAnchor::Point(Pos2::ZERO)}
                    placement={Placement::BelowStart}
                />
            </column>
        </click_catcher>
    };

    let overlay = overlay.get();
    set_component_state(State {
        overlay,
        content: content.clone(),
    });
    items.apply(move |items| {
        let menu = NodeRef::new();
        let replacement = in_new_scope({
            let (menu, row, panel) = (menu.clone(), row.clone(), panel.clone());
            move || {
                panel(view! {
                    <menu_list node_ref={&menu} items={items} row={row} panel={panel.clone()} />
                })
            }
        });
        replace_overlay_content(overlay, replacement);
        let menu = menu.get();
        content.set(Some(menu));
        let on_select = on_select.clone();
        menu::menu_list_on_select(menu).set(move |path: Vec<usize>| {
            on_select.call(path);
            close_overlay(overlay);
        });
    });

    catcher
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .content
        .get()
        .expect("context menu has no items yet")
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).overlay
}

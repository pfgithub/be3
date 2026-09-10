use beui_macros::{component, view};

use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    current_component, set_component_state, with_document, Callback, ClickCatcherBuilder,
};
use crate::unstyled;
use crate::unstyled::menu::{self, MenuItem, MenuPanel, MenuRow};

struct State {
    overlay: NodeId,
    content: NodeId,
    row: MenuRow,
    panel: MenuPanel,
    on_select: Callback<Vec<usize>>,
}

#[component]
pub fn context_menu(
    region: NodeId,
    items: Vec<MenuItem>,
    row: Option<MenuRow>,
    panel: Option<MenuPanel>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let context_menu = current_component();
    let row = row.unwrap_or_else(|| std::rc::Rc::new(|_| unstyled::column(0.0)));
    let panel = panel.unwrap_or_else(|| std::rc::Rc::new(|content| content));
    let (overlay, content) = with_document(|document| {
        let overlay =
            document.create_overlay(OverlayAnchor::Point(Pos2::ZERO), Placement::BelowStart);
        let content = menu::menu_list(document, &items, &row, &panel);
        document.set_overlay_content(overlay, panel(content));
        (overlay, content)
    });

    let root = unstyled::column(0.0);
    with_document(|document| {
        document.append_child(root, region, ItemSize::Intrinsic);
        document.append_child(root, overlay, ItemSize::Intrinsic);
    });

    let catcher = view! {
        <click_catcher
            cursor={CursorIcon::Default}
            on_secondary_press={move |press: PointerPress| {
                with_document(|document| {
                    document.set_overlay_anchor(overlay, OverlayAnchor::Point(press.pos));
                    document.open_overlay(overlay);
                    let content = document.component_state::<State>(context_menu).content;
                    menu::focus_menu_list_root(document, content);
                });
            }}
        >
            {root}
        </click_catcher>
    };

    with_document(|document| {
        wire_on_select(document, content, overlay, &on_select);
        set_component_state(
            document,
            context_menu,
            State {
                overlay,
                content,
                row,
                panel,
                on_select,
            },
        );
    });

    catcher
}

pub fn set_context_menu_items(context_menu: NodeId, items: Vec<MenuItem>) {
    with_document(|document| {
        let overlay = document.component_state::<State>(context_menu).overlay;
        let state = document.component_state::<State>(context_menu);
        let old_content = state.content;
        let row = state.row.clone();
        let panel = state.panel.clone();
        document.remove_node(document.overlay_content(overlay).unwrap_or(old_content));
        let content = menu::menu_list(document, &items, &row, &panel);
        document.set_overlay_content(overlay, panel(content));
        document.component_state_mut::<State>(context_menu).content = content;
        let on_select = document
            .component_state::<State>(context_menu)
            .on_select
            .clone();
        wire_on_select(document, content, overlay, &on_select);
    });
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).content
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).overlay
}

fn wire_on_select(
    document: &mut Document,
    content: NodeId,
    overlay: NodeId,
    on_select: &Callback<Vec<usize>>,
) {
    let on_select = on_select.clone();
    menu::menu_list_on_select(document, content).set(move |path: Vec<usize>| {
        on_select.call(path);
        with_document(|document| document.close_overlay(overlay));
    });
}

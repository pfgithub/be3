use beui_macros::{component, view};

use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, PointerPress};
use crate::node::{Handler, NodeId};
use crate::reactive::{current_component, set_component_state, with_document, ClickCatcherBuilder};
use crate::unstyled;
use crate::unstyled::menu::{self, MenuItem};

struct State {
    overlay: NodeId,
    content: NodeId,
    on_select: Option<Handler<Vec<usize>>>,
}

#[component]
pub fn context_menu(region: NodeId, items: Vec<MenuItem>) -> NodeId {
    let context_menu = current_component();
    let (overlay, content) = with_document(|document| {
        let overlay =
            document.create_overlay(OverlayAnchor::Point(Pos2::ZERO), Placement::BelowStart);
        let content = menu::menu_list(document, &items);
        document.set_overlay_content(overlay, content);
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
            on_secondary_press={Box::new(
                move |document: &mut Document, press: PointerPress| {
                    document.set_overlay_anchor(overlay, OverlayAnchor::Point(press.pos));
                    document.open_overlay(overlay);
                    let content = document.component_state::<State>(context_menu).content;
                    menu::focus_menu_list_root(document, content);
                },
            )}
        >
            {root}
        </click_catcher>
    };

    with_document(|document| {
        set_component_state(
            document,
            context_menu,
            State {
                overlay,
                content,
                on_select: None,
            },
        );
        wire_on_select(document, context_menu, overlay, content);
    });

    catcher
}

pub fn set_context_menu_items(context_menu: NodeId, items: Vec<MenuItem>) {
    with_document(|document| {
        let overlay = document.component_state::<State>(context_menu).overlay;
        let old_content = document.component_state::<State>(context_menu).content;
        document.remove_node(old_content);
        let content = menu::menu_list(document, &items);
        document.set_overlay_content(overlay, content);
        document.component_state_mut::<State>(context_menu).content = content;
        wire_on_select(document, context_menu, overlay, content);
    });
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).content
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).overlay
}

pub fn set_context_menu_on_select(
    context_menu: NodeId,
    handler: impl FnMut(&mut Document, Vec<usize>) + 'static,
) {
    with_document(|document| {
        document
            .component_state_mut::<State>(context_menu)
            .on_select = Some(Box::new(handler));
    });
}

fn wire_on_select(document: &mut Document, context_menu: NodeId, overlay: NodeId, content: NodeId) {
    menu::set_menu_list_on_select(document, content, move |document, path| {
        document
            .call_component_handler(context_menu, path, |state: &mut State| &mut state.on_select);
        document.close_overlay(overlay);
    });
}

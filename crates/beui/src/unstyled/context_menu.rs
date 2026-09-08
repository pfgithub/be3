use crate::base::overlay::{OverlayAnchor, Placement};
use crate::base::ItemSize;
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::CursorIcon;
use crate::node::{Handler, NodeId};
use crate::unstyled;
use crate::unstyled::menu::{self, MenuItem};

struct State {
    overlay: NodeId,
    content: NodeId,
    on_select: Option<Handler<Vec<usize>>>,
}

pub fn context_menu(document: &mut Document, region: NodeId, items: Vec<MenuItem>) -> NodeId {
    let overlay = document.create_overlay(OverlayAnchor::Point(Pos2::ZERO), Placement::BelowStart);
    let content = menu::menu_list(document, &items);
    document.set_overlay_content(overlay, content);

    let root = unstyled::column(document, 0.0);
    document.append_child(root, region, ItemSize::Intrinsic);
    document.append_child(root, overlay, ItemSize::Intrinsic);

    let catcher = document.create_click_catcher(CursorIcon::Default);
    document.set_click_catcher_child(catcher, root);

    let context_menu = document.create_shadow("context-menu", catcher, Vec::new());
    document.set_component_state(
        context_menu,
        State {
            overlay,
            content,
            on_select: None,
        },
    );

    wire_on_select(document, context_menu, overlay, content);

    document.set_click_catcher_on_secondary_press(catcher, move |document, press| {
        document.set_overlay_anchor(overlay, OverlayAnchor::Point(press.pos));
        document.open_overlay(overlay);
        let content = document.component_state::<State>(context_menu).content;
        menu::focus_menu_list_root(document, content);
    });

    context_menu
}

pub fn set_context_menu_items(document: &mut Document, context_menu: NodeId, items: Vec<MenuItem>) {
    let overlay = document.component_state::<State>(context_menu).overlay;
    let old_content = document.component_state::<State>(context_menu).content;
    document.remove_node(old_content);
    let content = menu::menu_list(document, &items);
    document.set_overlay_content(overlay, content);
    document.component_state_mut::<State>(context_menu).content = content;
    wire_on_select(document, context_menu, overlay, content);
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).content
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).overlay
}

pub fn set_context_menu_on_select(
    document: &mut Document,
    context_menu: NodeId,
    handler: impl FnMut(&mut Document, Vec<usize>) + 'static,
) {
    document
        .component_state_mut::<State>(context_menu)
        .on_select = Some(Box::new(handler));
}

fn wire_on_select(document: &mut Document, context_menu: NodeId, overlay: NodeId, content: NodeId) {
    menu::set_menu_list_on_select(document, content, move |document, path| {
        document
            .call_component_handler(context_menu, path, |state: &mut State| &mut state.on_select);
        document.close_overlay(overlay);
    });
}

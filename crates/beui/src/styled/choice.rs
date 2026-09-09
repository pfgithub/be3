use crate::base::ItemSize;
use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, intrinsic, with_document, CenteredRowBuilder, FillBuilder, OutlineBuilder,
    PaddingBuilder, SizedBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

pub(super) use crate::unstyled::ChoiceKind as Kind;

struct State {
    on_change: Option<Handler<Option<usize>>>,
}

pub(super) fn choice(
    document: &mut Document,
    labels: &[&str],
    selected: Option<usize>,
    kind: Kind,
) -> NodeId {
    let inner = unstyled::choice(document, labels, selected, kind);
    let selected = unstyled::choice_selected(document, inner);
    let label_strings: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();

    let mut fills = Vec::new();
    let mut label_nodes = Vec::new();
    let mut marks: Vec<Option<NodeId>> = Vec::new();

    for index in 0..unstyled::choice_option_count(document, inner) {
        let button = unstyled::choice_option_button(document, inner, index);
        let label = unstyled::choice_option_label_node(document, inner, index);
        let active = unstyled::choice_selected(document, inner) == Some(index);

        document.set_text_font_size(label, FONT_BODY);
        document.set_text_color(label, if active { TEXT } else { TEXT_MUTED });
        if kind == Kind::Tabs {
            document.set_text_align(label, TextAlign::Center, TextAlign::Center);
        }

        let mut mark = None;
        let content = if kind == Kind::Radio {
            let dot = FillBuilder::default()
                .color(ACCENT)
                .radius(9)
                .children([])
                .build();
            let dot_size = SizedBuilder::default()
                .width(8.0)
                .height(8.0)
                .children([intrinsic(dot)])
                .build();
            let visibility = VisibilityBuilder::default()
                .visible(active)
                .children([intrinsic(dot_size)])
                .build();
            mark = Some(visibility);
            let before = unstyled::spacer(document);
            let after = unstyled::spacer(document);
            let center = CenteredRowBuilder::default()
                .spacing(0.0)
                .children([
                    (before, ItemSize::Percent(100.0)),
                    (visibility, ItemSize::Intrinsic),
                    (after, ItemSize::Percent(100.0)),
                ])
                .build();
            let circle = OutlineBuilder::default()
                .color(BORDER)
                .width(2.0)
                .radius(9)
                .offset(0.0)
                .visible(true)
                .children([intrinsic(center)])
                .build();
            let size = SizedBuilder::default()
                .width(18.0)
                .height(18.0)
                .children([intrinsic(circle)])
                .build();
            CenteredRowBuilder::default()
                .spacing(10.0)
                .children([
                    (size, ItemSize::Intrinsic),
                    (label, ItemSize::Percent(100.0)),
                ])
                .build()
        } else {
            label
        };
        let padding = PaddingBuilder::default()
            .horizontal(14.0)
            .vertical(6.0)
            .children([intrinsic(content)])
            .build();
        let fill = FillBuilder::default()
            .color(background(active, false))
            .radius(RADIUS)
            .children([intrinsic(padding)])
            .build();
        let ring = OutlineBuilder::default()
            .color(ACCENT)
            .width(2.0)
            .radius(RADIUS)
            .offset(1.0)
            .children([intrinsic(fill)])
            .build();
        unstyled::set_button_child(button, ring);

        let hovered = unstyled::button_hovered(document, button);
        create_effect(move || {
            let is_hovered = hovered.get();
            with_document(|document| {
                let active = unstyled::choice_selected(document, inner) == Some(index);
                document.set_fill_color(fill, background(active, is_hovered));
            });
        });
        let focused = unstyled::button_focused(document, button);
        create_effect(move || {
            let visible = focused.get();
            with_document(|document| document.set_outline_visible(ring, visible));
        });

        fills.push(fill);
        label_nodes.push(label);
        marks.push(mark);
    }

    let choice = document.create_shadow(kind_name(kind), inner, Vec::new());
    document.set_component_detail(choice, selected.map_or("", |index| labels[index]));
    document.set_component_state(choice, State { on_change: None });

    unstyled::set_choice_on_change(document, inner, move |document, selected| {
        for (index, &fill) in fills.iter().enumerate() {
            let active = selected == Some(index);
            let button = unstyled::choice_option_button(document, inner, index);
            let hovered = unstyled::button_hovered(document, button).get();
            document.set_fill_color(fill, background(active, hovered));
            document.set_text_color(label_nodes[index], if active { TEXT } else { TEXT_MUTED });
            if let Some(mark) = marks[index] {
                document.set_visible(mark, active);
            }
        }
        let detail = selected
            .map(|index| label_strings[index].clone())
            .unwrap_or_default();
        document.set_component_detail(choice, detail);
        document.call_component_handler(choice, selected, |state: &mut State| &mut state.on_change);
    });

    choice
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    let inner = document.shadow_root(choice);
    unstyled::choice_selected(document, inner)
}

pub(super) fn set_selected(document: &mut Document, choice: NodeId, selected: Option<usize>) {
    let inner = document.shadow_root(choice);
    unstyled::set_choice_selected(document, inner, selected);
}

pub(super) fn set_on_change(
    document: &mut Document,
    choice: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    document.component_state_mut::<State>(choice).on_change = Some(Box::new(handler));
}

pub(super) fn focus(document: &mut Document, choice: NodeId) {
    let inner = document.shadow_root(choice);
    unstyled::focus_choice(document, inner);
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Tabs => "tabs",
        Kind::Radio => "radio-group",
        Kind::Listbox => "listbox",
    }
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

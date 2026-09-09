use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::view;

use crate::reactive::{
    create_effect, intrinsic, with_document, CenteredRowBuilder, FillBuilder, OutlineBuilder,
    PaddingBuilder, SizedBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

pub(super) use crate::unstyled::ChoiceKind as Kind;

pub(super) fn choice(
    document: &mut Document,
    labels: &[&str],
    selected: Option<usize>,
    kind: Kind,
    mut on_change: impl FnMut(&mut Document, Option<usize>) + 'static,
) -> NodeId {
    let inner = unstyled::choice(document, labels, selected, kind);

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
            let visibility = view! {
                <visibility visible={active}>
                    <sized width={8.0} height={8.0}>{dot}</sized>
                </visibility>
            };
            mark = Some(visibility);
            let before = unstyled::spacer(document);
            let after = unstyled::spacer(document);
            view! {
                <centered_row spacing={10.0}>
                    <sized width={18.0} height={18.0}>
                        <outline color={BORDER} width={2.0} radius={9} offset={0.0} visible={true}>
                            <centered_row spacing={0.0}>
                                @percent(100.0) {before}
                                {visibility}
                                @percent(100.0) {after}
                            </centered_row>
                        </outline>
                    </sized>
                    @percent(100.0) {label}
                </centered_row>
            }
        } else {
            label
        };
        let padding = view! { <padding horizontal={14.0} vertical={6.0}>{content}</padding> };
        let fill = FillBuilder::default()
            .color(background(active, false))
            .radius(RADIUS)
            .children([intrinsic(padding)])
            .build();
        let ring = view! {
            <outline color={ACCENT} width={2.0} radius={RADIUS} offset={1.0}>
                {fill}
            </outline>
        };
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
        on_change(document, selected);
    });

    inner
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    let inner = document.shadow_root(choice);
    unstyled::choice_selected(document, inner)
}

pub(super) fn focus(document: &mut Document, choice: NodeId) {
    let inner = document.shadow_root(choice);
    unstyled::focus_choice(document, inner);
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

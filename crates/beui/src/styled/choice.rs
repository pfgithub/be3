use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::view;

use crate::reactive::{
    create_effect, with_document, CenteredRowBuilder, FillBuilder, OutlineBuilder, PaddingBuilder,
    Prop, SizedBuilder, SpacerBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;

pub(super) use crate::unstyled::ChoiceKind as Kind;

pub(super) fn choice(
    labels: &[&str],
    selected: Prop<Option<usize>>,
    kind: Kind,
    on_change: impl FnMut(&mut Document, Option<usize>) + 'static,
) -> NodeId {
    let inner = unstyled::choice(labels, selected, kind, Some(Box::new(on_change)));
    let selected_signal =
        with_document(|document| unstyled::choice_selected_signal(document, inner));

    with_document(|document| {
        for index in 0..unstyled::choice_option_count(document, inner) {
            let button = unstyled::choice_option_button(document, inner, index);
            let label = unstyled::choice_option_label_node(document, inner, index);

            document.set_text_font_size(label, FONT_BODY);
            if kind == Kind::Tabs {
                document.set_text_align(label, TextAlign::Center, TextAlign::Center);
            }
            create_effect({
                let selected_signal = selected_signal.clone();
                move || {
                    let active = selected_signal.get() == Some(index);
                    with_document(|document| {
                        document.set_text_color(label, if active { TEXT } else { TEXT_MUTED });
                    });
                }
            });

            let content = if kind == Kind::Radio {
                let mark_visible = {
                    let selected_signal = selected_signal.clone();
                    Prop::Dynamic(Box::new(move || selected_signal.get() == Some(index)))
                };
                let before = view! { <spacer /> };
                let after = view! { <spacer /> };
                view! {
                    <centered_row spacing={10.0}>
                        <sized width={18.0} height={18.0}>
                            <outline color={BORDER} width={2.0} radius={9} offset={0.0} visible={true}>
                                <centered_row spacing={0.0}>
                                    @percent(100.0) {before}
                                    <visibility visible={mark_visible}>
                                        <sized width={8.0} height={8.0}>
                                            <fill color={ACCENT} radius={9}></fill>
                                        </sized>
                                    </visibility>
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
            let hovered = unstyled::button_hovered(document, button);
            let focused = unstyled::button_focused(document, button);
            let fill_color = {
                let selected_signal = selected_signal.clone();
                Prop::Dynamic(Box::new(move || {
                    let active = selected_signal.get() == Some(index);
                    background(active, hovered.get())
                }))
            };
            let ring = view! {
                <outline color={ACCENT} width={2.0} radius={RADIUS} offset={1.0} visible={focused}>
                    <fill color={fill_color} radius={RADIUS}>
                        <padding horizontal={14.0} vertical={6.0}>{content}</padding>
                    </fill>
                </outline>
            };
            unstyled::set_button_child(button, ring);
        }
    });

    inner
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    let inner = document.shadow_root(choice);
    unstyled::choice_selected(document, inner)
}

pub(super) fn focus(choice: NodeId) {
    with_document(|document| {
        let inner = document.shadow_root(choice);
        unstyled::focus_choice(inner);
    });
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

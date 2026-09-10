use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::view;

use crate::reactive::{
    with_document, Callback, CenteredRowBuilder, FillBuilder, OutlineBuilder, PaddingBuilder, Prop,
    SizedBuilder, SpacerBuilder, TextBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::ChoiceOptionHandle;

pub(super) use crate::unstyled::ChoiceKind as Kind;

pub(super) fn choice(
    labels: &[&str],
    selected: Prop<Option<usize>>,
    kind: Kind,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    unstyled::choice(
        labels,
        selected,
        kind,
        on_change,
        Box::new(move |option| option_view(kind, option)),
    )
}

fn option_view(kind: Kind, option: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = option;
    let label_color = {
        let selected = selected.clone();
        Prop::Dynamic(Box::new(
            move || {
                if selected.get() {
                    TEXT
                } else {
                    TEXT_MUTED
                }
            },
        ))
    };
    let align = if kind == Kind::Tabs {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    let label = view! {
        <text string={label} font_size={FONT_BODY} color={label_color} align={align} />
    };

    let content = if kind == Kind::Radio {
        let mark_visible = selected.clone();
        view! {
            <centered_row spacing={10.0}>
                <sized width={18.0} height={18.0}>
                    <outline color={BORDER} width={2.0} radius={9} offset={0.0} visible={true}>
                        <centered_row spacing={0.0}>
                            @percent(100.0) <spacer />
                            <visibility visible={mark_visible}>
                                <sized width={8.0} height={8.0}>
                                    <fill color={ACCENT} radius={9}></fill>
                                </sized>
                            </visibility>
                            @percent(100.0) <spacer />
                        </centered_row>
                    </outline>
                </sized>
                @percent(100.0) {label}
            </centered_row>
        }
    } else {
        label
    };

    let fill_color = Prop::Dynamic(Box::new(move || background(selected.get(), hovered.get())));
    view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={1.0} visible={focused}>
            <fill color={fill_color} radius={RADIUS}>
                <padding horizontal={14.0} vertical={6.0}>{content}</padding>
            </fill>
        </outline>
    }
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

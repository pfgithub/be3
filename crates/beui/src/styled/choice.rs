use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::Memo;
use crate::reactive::{
    clone, create_memo, CenteredRowBuilder, FillBuilder, ItemSize, OutlineBuilder, PaddingBuilder,
    Prop, SizedBuilder, SpacerBuilder, TextBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::ChoiceOptionHandle;

pub(super) use crate::unstyled::ChoiceKind as Kind;

const MARK_SPACING: f32 = 10.0;
const MARK_BOX: f32 = 18.0;
const MARK_DOT: f32 = 8.0;
const MARK_RADIUS: u8 = 9;

#[component]
pub(super) fn choice_option(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let label_color =
        create_memo(clone!(selected -> move || if selected.get() { TEXT } else { TEXT_MUTED }));
    let checked = selected.clone();
    let fill_color = create_memo(move || background(selected.get(), hovered.get()));
    view! {
        <outline color=ACCENT width=2.0 radius=RADIUS offset=1.0 visible={focused}>
            <fill color={fill_color} radius=RADIUS>
                <padding horizontal=14.0 vertical=6.0>
                    <choice_label
                        kind
                        label
                        color={label_color}
                        checked
                    />
                </padding>
            </fill>
        </outline>
    }
}

#[component]
fn choice_label(kind: Kind, label: String, color: Prop<Color32>, checked: Memo<bool>) -> NodeId {
    let align = if kind == Kind::Tabs {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    if kind != Kind::Radio {
        return view! { <text string={label} font_size=FONT_BODY color align /> };
    }
    view! {
        <centered_row spacing=MARK_SPACING>
            <radio_mark checked />
            <text @sizing=ItemSize::Percent(100.0) string={label} font_size=FONT_BODY color align />
        </centered_row>
    }
}

#[component]
fn radio_mark(checked: Memo<bool>) -> NodeId {
    view! {
        <sized width=MARK_BOX height=MARK_BOX>
            <outline color=BORDER width=2.0 radius=MARK_RADIUS offset=0.0 visible=true>
                <centered_row spacing=0.0>
                    <spacer @sizing=ItemSize::Percent(100.0) />
                    <visibility visible={checked}>
                        <sized width=MARK_DOT height=MARK_DOT>
                            <fill color=ACCENT radius=MARK_RADIUS />
                        </sized>
                    </visibility>
                    <spacer @sizing=ItemSize::Percent(100.0) />
                </centered_row>
            </outline>
        </sized>
    }
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    let inner = document.shadow_root(choice);
    unstyled::choice_selected(document, inner)
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

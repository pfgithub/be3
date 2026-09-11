use crate::base::shadow::shadow_root;
use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::Memo;
use crate::reactive::{
    Callback, CenteredRowBuilder, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
    SpacerBuilder, TextBuilder, VisibilityBuilder,
};
use crate::styled::theme::{
    ACCENT, ACCENT_SOFT, BORDER, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::ChoiceBuilder;
use crate::unstyled::ChoiceOptionHandle;

pub(super) use crate::unstyled::ChoiceKind as Kind;

const MARK_SPACING: f32 = 10.0;
const MARK_BOX: f32 = 18.0;
const MARK_DOT: f32 = 8.0;
const MARK_RADIUS: u8 = 9;

pub(super) fn choice(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    kind: Kind,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    view! {
        <choice
            labels={labels}
            selected={selected}
            kind={kind}
            on_change={move |selected| on_change.call(selected)}
            option={Box::new(move |handle| view! { <choice_option kind={kind} handle={handle} /> })}
        />
    }
}

#[component]
fn choice_option(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
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
    let radio = kind == Kind::Radio;
    let checked = selected.clone();
    let fill_color = Prop::Dynamic(Box::new(move || background(selected.get(), hovered.get())));
    view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={1.0} visible={focused}>
            <fill color={fill_color} radius={RADIUS}>
                <padding horizontal={14.0} vertical={6.0}>
                    {{
                        let label = view! {
                            <text string={label} font_size={FONT_BODY} color={label_color} align={align} />
                        };
                        if !radio {
                            label
                        } else {
                            view! {
                                <centered_row spacing={MARK_SPACING}>
                                    <radio_mark checked={checked} />
                                    @percent(100.0) {label}
                                </centered_row>
                            }
                        }
                    }}
                </padding>
            </fill>
        </outline>
    }
}

#[component]
fn radio_mark(checked: Memo<bool>) -> NodeId {
    view! {
        <sized width={MARK_BOX} height={MARK_BOX}>
            <outline color={BORDER} width={2.0} radius={MARK_RADIUS} offset={0.0} visible={true}>
                <centered_row spacing={0.0}>
                    @percent(100.0) <spacer />
                    <visibility visible={checked}>
                        <sized width={MARK_DOT} height={MARK_DOT}>
                            <fill color={ACCENT} radius={MARK_RADIUS} />
                        </sized>
                    </visibility>
                    @percent(100.0) <spacer />
                </centered_row>
            </outline>
        </sized>
    }
}

pub(super) fn selected_index(document: &Document, choice: NodeId) -> Option<usize> {
    let inner = document.shadow_root(choice);
    unstyled::choice_selected(document, inner)
}

pub(super) fn focus(choice: NodeId) {
    unstyled::focus_choice(shadow_root(choice));
}

fn background(active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        _ => Color32::TRANSPARENT,
    }
}

use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_effect, create_signal, current_component, set_component_detail, with_document, Callback,
    CenteredRowBuilder, Children, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder,
    TextBuilder,
};
use crate::styled::theme::{
    ACCENT, FONT_HEADING, FONT_SMALL, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::DisclosureHandle;

const SPACING: f32 = 10.0;
const MARKER_WIDTH: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 6.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn accordion(
    title: Prop<String>,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
    children: Children,
) -> NodeId {
    let child = children
        .into_first()
        .expect("accordion requires a child, e.g. <accordion>{content}</accordion>");
    let shadow = current_component();

    let (title_text, set_title_text) = create_signal(String::new());
    title.apply(move |value| set_title_text.set(value));
    create_effect({
        let title_text = title_text.clone();
        move || {
            let value = title_text.get();
            with_document(|document| set_component_detail(document, shadow, value));
        }
    });

    view! {
        <unstyled::disclosure
            spacing={SPACING}
            on_toggle={move |open| on_toggle.call(open)}
            header={Box::new(move |handle: DisclosureHandle| {
                let header_color = Prop::Dynamic(Box::new(move || header_fill(handle.hovered.get())));
                let marker_glyph = Prop::Dynamic(Box::new(move || glyph(handle.open.get()).to_owned()));
                view! {
                    <outline color={ACCENT} width={2.0} radius={RADIUS} offset={2.0} visible={handle.focused}>
                        <fill color={header_color} radius={RADIUS}>
                            <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                                <centered_row spacing={SPACING}>
                                    <sized width={MARKER_WIDTH}>
                                        <text
                                            string={marker_glyph}
                                            font_size={FONT_SMALL}
                                            color={TEXT_MUTED}
                                            monospace={true}
                                            align={TextAlign::Center}
                                        />
                                    </sized>
                                    @percent(100.0) <text string={title_text} font_size={FONT_HEADING} color={TEXT} align={TextAlign::Start} />
                                </centered_row>
                            </padding>
                        </fill>
                    </outline>
                }
            })}
            open={open}
        >
            {child}
        </unstyled::disclosure>
    }
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, document.shadow_root(accordion))
}

fn glyph(open: bool) -> &'static str {
    if open {
        "-"
    } else {
        "+"
    }
}

fn header_fill(hovered: bool) -> Color32 {
    if hovered {
        SURFACE_RAISED
    } else {
        Color32::TRANSPARENT
    }
}

use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_memo, Callback, CenteredRowBuilder, Children, FillBuilder, Memo,
    OutlineBuilder, PaddingBuilder, Prop, SizedBuilder, TextBuilder,
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

    let title_text = title.memo();
    component_detail(title_text.clone());

    view! {
        <unstyled::disclosure
            spacing={SPACING}
            on_toggle={move |open| on_toggle.call(open)}
            header={move |handle| view! { <accordion_header handle={handle} title={title_text} /> }}
            open={open}
        >
            {child}
        </unstyled::disclosure>
    }
}

#[component]
fn accordion_header(handle: DisclosureHandle, title: Memo<String>) -> NodeId {
    let DisclosureHandle {
        hovered,
        open,
        focused,
        ..
    } = handle;
    let header_color = create_memo(move || header_fill(hovered.get()));
    let marker_glyph = create_memo(move || glyph(open.get()).to_owned());
    view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={2.0} visible={focused}>
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
                        @percent(100.0) <text
                            string={title}
                            font_size={FONT_HEADING}
                            color={TEXT}
                            align={TextAlign::Start}
                        />
                    </centered_row>
                </padding>
            </fill>
        </outline>
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

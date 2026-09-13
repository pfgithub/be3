use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_memo, Callback, CenteredRow, Child, Fill, ItemSize, Memo, Outline, Padding, Prop, Sized,
    Text,
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
pub fn Accordion(
    title: Prop<String>,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
    children: Child,
) -> NodeId {
    let title_text = create_memo(move || title.get());
    view! {
        <unstyled::Disclosure
            spacing=SPACING
            on_toggle={move |open| on_toggle.call(open)}
            header={move |handle| view! { <AccordionHeader handle title={title_text} /> }}
            open
        >
            {children}
        </unstyled::Disclosure>
    }
}

#[component]
fn AccordionHeader(handle: DisclosureHandle, title: Memo<String>) -> NodeId {
    let DisclosureHandle {
        hovered,
        open,
        focused,
        ..
    } = handle;
    let header_color = create_memo(move || header_fill(hovered.get()));
    let marker_glyph = create_memo(move || glyph(open.get()).to_owned());
    view! {
        <Outline color=ACCENT width=2.0 radius=RADIUS offset=2.0 visible={focused}>
            <Fill color={header_color} radius=RADIUS>
                <Padding horizontal=PADDING_HORIZONTAL vertical=PADDING_VERTICAL>
                    <CenteredRow spacing=SPACING>
                        <Sized width=MARKER_WIDTH>
                            <Text
                                string={marker_glyph}
                                font_size=FONT_SMALL
                                color=TEXT_MUTED
                                monospace=true
                                align=TextAlign::Center
                            />
                        </Sized>
                        <Text @sizing=ItemSize::Percent(100.0)
                            string={title}
                            font_size=FONT_HEADING
                            color=TEXT
                            align=TextAlign::Start
                        />
                    </CenteredRow>
                </Padding>
            </Fill>
        </Outline>
    }
}

pub fn accordion_open(document: &Document, accordion: NodeId) -> bool {
    unstyled::disclosure_open(document, accordion)
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

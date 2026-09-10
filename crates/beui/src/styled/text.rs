use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{
    create_effect, create_signal, current_component, set_component_detail, with_document, Prop,
    TextBuilder,
};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

#[component]
pub fn code(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! {
        <text
            string={content}
            font_size={FONT_SMALL}
            color={color}
            align={align.unwrap_or(TextAlign::Start)}
            monospace={true}
        />
    }
}

#[component]
pub fn icon(glyph: String, #[prop(default = TEXT)] color: Prop<Color32>) -> NodeId {
    view! { <icon_sized glyph={glyph} font_size={ICON_SIZE} color={color} /> }
}

#[component]
pub fn icon_sized(glyph: String, font_size: f32, color: Prop<Color32>) -> NodeId {
    view! {
        <text string={glyph} font_size={font_size} color={color} align={TextAlign::Center} icon={true} />
    }
}

fn reactive_line(
    content: Prop<String>,
    font_size: f32,
    color: Prop<Color32>,
    align: Option<TextAlign>,
) -> NodeId {
    let shadow = current_component();
    let (text, set_text) = create_signal(String::new());
    content.apply(move |value| set_text.set(value));
    create_effect({
        let text = text.clone();
        move || {
            let value = text.get();
            with_document(|document| set_component_detail(document, shadow, format!("{value:?}")));
        }
    });
    view! {
        <text string={text} font_size={font_size} color={color} align={align.unwrap_or(TextAlign::Start)} />
    }
}

#[component]
pub fn display(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    reactive_line(content, FONT_DISPLAY, color, align)
}

#[component]
pub fn title(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    reactive_line(content, FONT_TITLE, color, align)
}

#[component]
pub fn heading(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    reactive_line(content, FONT_HEADING, color, align)
}

#[component]
pub fn body(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    reactive_line(content, FONT_BODY, color, align)
}

#[component]
pub fn caption(
    content: Prop<String>,
    align: Option<TextAlign>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    reactive_line(content, FONT_SMALL, color, align)
}

#[component]
pub fn paragraph(
    content: Prop<String>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    let shadow = current_component();
    let (text, set_text) = create_signal(String::new());
    content.apply(move |value| set_text.set(value));
    create_effect({
        let text = text.clone();
        move || {
            let value = text.get();
            with_document(|document| set_component_detail(document, shadow, format!("{value:?}")));
        }
    });
    view! {
        <text string={text} font_size={FONT_BODY} color={color} wrap={true} />
    }
}

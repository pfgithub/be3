use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{component_detail, create_signal, Prop, TextBuilder};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

#[component]
pub fn code(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! {
        <text
            string={content}
            font_size={FONT_SMALL}
            color={color}
            align={align}
            monospace={true}
        />
    }
}

#[component]
pub fn icon(glyph: String, #[prop(default = TEXT)] color: Prop<Color32>) -> NodeId {
    view! { <icon_sized glyph={glyph} font_size={ICON_SIZE} color={color} /> }
}

#[component]
pub fn icon_sized(glyph: String, font_size: Prop<f32>, color: Prop<Color32>) -> NodeId {
    view! {
        <text string={glyph} font_size={font_size} color={color} align={TextAlign::Center} icon={true} />
    }
}

#[component(base)]
fn line(
    content: Prop<String>,
    font_size: Prop<f32>,
    color: Prop<Color32>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
) -> NodeId {
    let (text, set_text) = create_signal(String::new());
    content.apply(move |value| set_text.set(value));
    component_detail({
        let text = text.clone();
        move || format!("{:?}", text.get())
    });
    view! {
        <text string={text} font_size={font_size} color={color} align={align} />
    }
}

#[component]
pub fn display(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content={content} font_size={FONT_DISPLAY} color={color} align={align} /> }
}

#[component]
pub fn title(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content={content} font_size={FONT_TITLE} color={color} align={align} /> }
}

#[component]
pub fn heading(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content={content} font_size={FONT_HEADING} color={color} align={align} /> }
}

#[component]
pub fn body(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content={content} font_size={FONT_BODY} color={color} align={align} /> }
}

#[component]
pub fn caption(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content={content} font_size={FONT_SMALL} color={color} align={align} /> }
}

#[component]
pub fn paragraph(
    content: Prop<String>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    let (text, set_text) = create_signal(String::new());
    content.apply(move |value| set_text.set(value));
    component_detail({
        let text = text.clone();
        move || format!("{:?}", text.get())
    });
    view! {
        <text string={text} font_size={FONT_BODY} color={color} wrap={true} />
    }
}

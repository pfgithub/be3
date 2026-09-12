use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{clone, component_detail, create_memo, Prop, TextBuilder};
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
            font_size=FONT_SMALL
            color
            align
            monospace=true
        />
    }
}

#[component]
pub fn icon(glyph: String, #[prop(default = TEXT)] color: Prop<Color32>) -> NodeId {
    view! { <icon_sized glyph font_size=ICON_SIZE color /> }
}

#[component]
pub fn icon_sized(glyph: String, font_size: Prop<f32>, color: Prop<Color32>) -> NodeId {
    view! {
        <text string={glyph} font_size color align=TextAlign::Center icon=true />
    }
}

#[component(base)]
fn line(
    content: Prop<String>,
    font_size: Prop<f32>,
    color: Prop<Color32>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
) -> NodeId {
    let text = create_memo(move || content.get());
    component_detail(create_memo(
        clone!(text -> move || format!("{:?}", text.get())),
    ));
    view! {
        <text string={text} font_size color align />
    }
}

#[component]
pub fn display(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content font_size=FONT_DISPLAY color align /> }
}

#[component]
pub fn title(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content font_size=FONT_TITLE color align /> }
}

#[component]
pub fn heading(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content font_size=FONT_HEADING color align /> }
}

#[component]
pub fn body(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content font_size=FONT_BODY color align /> }
}

#[component]
pub fn caption(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    view! { <line content font_size=FONT_SMALL color align /> }
}

#[component]
pub fn paragraph(
    content: Prop<String>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    let text = create_memo(move || content.get());
    component_detail(create_memo(
        clone!(text -> move || format!("{:?}", text.get())),
    ));
    view! {
        <text string={text} font_size=FONT_BODY color wrap=true />
    }
}

use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{clone, component_detail, create_memo, Prop, Text};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, TEXT, TEXT_MUTED,
};

#[component]
pub fn Code(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Text
            string={content}
            font_size=FONT_SMALL
            color
            align
            monospace=true
        />
    }
}

#[component]
pub fn Icon(glyph: String, #[prop(default = TEXT)] color: Prop<Color32>) -> NodeId {
    view! { <IconSized glyph font_size=ICON_SIZE color /> }
}

#[component]
pub fn IconSized(glyph: String, font_size: Prop<f32>, color: Prop<Color32>) -> NodeId {
    view! {
        <Text string={glyph} font_size color align=TextAlign::Center icon=true />
    }
}

#[component(base)]
fn Line(
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
        <Text string={text} font_size color align />
    }
}

#[component]
pub fn Display(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <Line content font_size=FONT_DISPLAY color align /> }
}

#[component]
pub fn Title(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <Line content font_size=FONT_TITLE color align /> }
}

#[component]
pub fn Heading(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <Line content font_size=FONT_HEADING color align /> }
}

#[component]
pub fn Body(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT)] color: Prop<Color32>,
) -> NodeId {
    view! { <Line content font_size=FONT_BODY color align /> }
}

#[component]
pub fn Caption(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    view! { <Line content font_size=FONT_SMALL color align /> }
}

#[component]
pub fn Paragraph(
    content: Prop<String>,
    #[prop(default = TEXT_MUTED)] color: Prop<Color32>,
) -> NodeId {
    let text = create_memo(move || content.get());
    component_detail(create_memo(
        clone!(text -> move || format!("{:?}", text.get())),
    ));
    view! {
        <Text string={text} font_size=FONT_BODY color wrap=true />
    }
}

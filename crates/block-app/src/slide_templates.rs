use block::Block;
use block_client::blocks::infinite_canvas::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasPreviewRegion,
    CanvasTextAlign, CanvasTextStyle, CanvasTextWeight, CanvasTransform, InfiniteCanvas,
    InfiniteCanvasOperation,
};
use eframe::egui::Vec2;
use egui_material_icons::{
    icons::{ICON_CROP_SQUARE, ICON_SUBJECT, ICON_TITLE},
    MaterialIcon,
};
use uuid::Uuid;

pub const DEFAULT_SLIDE_SIZE: Vec2 = eframe::egui::vec2(960.0, 540.0);

const TITLE_FONT_SIZE: f32 = 54.0;
const SUBTITLE_FONT_SIZE: f32 = 26.0;
const HEADER_FONT_SIZE: f32 = 40.0;
const BODY_FONT_SIZE: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlideTemplate {
    Title,
    Regular,
    Blank,
}

impl SlideTemplate {
    pub const ALL: [SlideTemplate; 3] = [Self::Title, Self::Regular, Self::Blank];

    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title page",
            Self::Regular => "Regular page",
            Self::Blank => "Blank page",
        }
    }

    pub fn icon(self) -> MaterialIcon {
        match self {
            Self::Title => ICON_TITLE,
            Self::Regular => ICON_SUBJECT,
            Self::Blank => ICON_CROP_SQUARE,
        }
    }
}

fn template_text_entity(
    center: CanvasPoint,
    size: CanvasPoint,
    placeholder: &str,
    text_style: CanvasTextStyle,
) -> CanvasEntity {
    CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(center, size, 0.0),
        kind: CanvasEntityKind::Text {
            text: String::new(),
            text_style,
            placeholder: placeholder.into(),
        },
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
    }
}

fn template_entities(template: SlideTemplate) -> Vec<CanvasEntity> {
    match template {
        SlideTemplate::Blank => Vec::new(),
        SlideTemplate::Title => vec![
            template_text_entity(
                CanvasPoint::new(0.0, -40.0),
                CanvasPoint::new(820.0, 110.0),
                "Title",
                CanvasTextStyle {
                    font_size: TITLE_FONT_SIZE,
                    weight: CanvasTextWeight::Bold,
                    alignment: CanvasTextAlign::Center,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
            template_text_entity(
                CanvasPoint::new(0.0, 70.0),
                CanvasPoint::new(700.0, 60.0),
                "Subtitle",
                CanvasTextStyle {
                    font_size: SUBTITLE_FONT_SIZE,
                    weight: CanvasTextWeight::Regular,
                    alignment: CanvasTextAlign::Center,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
        ],
        SlideTemplate::Regular => vec![
            template_text_entity(
                CanvasPoint::new(0.0, -220.0),
                CanvasPoint::new(860.0, 80.0),
                "Header",
                CanvasTextStyle {
                    font_size: HEADER_FONT_SIZE,
                    weight: CanvasTextWeight::Bold,
                    alignment: CanvasTextAlign::Left,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
            template_text_entity(
                CanvasPoint::new(0.0, 40.0),
                CanvasPoint::new(860.0, 380.0),
                "Body",
                CanvasTextStyle {
                    font_size: BODY_FONT_SIZE,
                    weight: CanvasTextWeight::Regular,
                    alignment: CanvasTextAlign::Left,
                    line_height: 1.3,
                    wrap: true,
                },
            ),
        ],
    }
}

pub fn build_template_canvas(template: SlideTemplate) -> InfiniteCanvas {
    let mut canvas = InfiniteCanvas::new();
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(CanvasPreviewRegion::new(
                CanvasPoint::default(),
                CanvasPoint::new(DEFAULT_SLIDE_SIZE.x, DEFAULT_SLIDE_SIZE.y),
            )),
        },
    );
    for entity in template_entities(template) {
        InfiniteCanvas::apply_operation(&mut canvas, &InfiniteCanvasOperation::Add { entity });
    }
    canvas
}

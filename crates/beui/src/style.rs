use egui::Color32;

#[derive(Clone)]
pub struct Style {
    pub spacing: f32,
    pub padding: f32,
    pub font_size: f32,
    pub text_color: Color32,
    pub button_fill: Color32,
    pub button_hover_fill: Color32,
    pub button_press_fill: Color32,
    pub button_text_color: Color32,
    pub corner_radius: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            spacing: 8.0,
            padding: 8.0,
            font_size: 14.0,
            text_color: Color32::from_gray(20),
            button_fill: Color32::from_rgb(70, 120, 220),
            button_hover_fill: Color32::from_rgb(90, 140, 235),
            button_press_fill: Color32::from_rgb(50, 95, 190),
            button_text_color: Color32::WHITE,
            corner_radius: 4,
        }
    }
}

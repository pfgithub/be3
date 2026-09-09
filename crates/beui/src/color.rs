#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Color32([u8; 4]);

impl Color32 {
    pub const TRANSPARENT: Self = Self::from_rgba_unmultiplied(0, 0, 0, 0);
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    pub const fn from_rgba_unmultiplied(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([red, green, blue, alpha])
    }

    pub const fn from_gray(level: u8) -> Self {
        Self([level, level, level, 255])
    }

    pub const fn to_array(self) -> [u8; 4] {
        self.0
    }

    pub const fn alpha(self) -> u8 {
        self.0[3]
    }

    pub fn to_normalized_gamma_f32(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.0;
        [
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            alpha as f32 / 255.0,
        ]
    }

    pub fn to_linear_f32(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.0;
        [
            linear_from_gamma(red),
            linear_from_gamma(green),
            linear_from_gamma(blue),
            alpha as f32 / 255.0,
        ]
    }
}

fn linear_from_gamma(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

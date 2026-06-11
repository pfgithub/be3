#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeRecommendation {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl SizeRecommendation {
    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self { width, height }
    }

    pub const fn exact(width: f32, height: f32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }

    pub(crate) fn main(self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    pub(crate) fn with_main(self, axis: Axis, value: Option<f32>) -> Self {
        match axis {
            Axis::Horizontal => Self::new(value, self.height),
            Axis::Vertical => Self::new(self.width, value),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub(crate) fn main(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    pub(crate) fn cross(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.height,
            Axis::Vertical => self.width,
        }
    }

    pub(crate) fn from_axes(axis: Axis, main: f32, cross: f32) -> Self {
        match axis {
            Axis::Horizontal => Self::new(main, cross),
            Axis::Vertical => Self::new(cross, main),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn size(self) -> Size {
        Size::new(self.width, self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    Intrinsic,
    Fr(f32),
}

impl Sizing {
    pub const fn fr(value: f32) -> Self {
        Self::Fr(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeSource {
    Parent,
    Child,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(u32);

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }

    pub(crate) fn as_f32(self) -> [f32; 4] {
        [
            ((self.0 >> 16) & 0xff) as f32 / 255.0,
            ((self.0 >> 8) & 0xff) as f32 / 255.0,
            (self.0 & 0xff) as f32 / 255.0,
            1.0,
        ]
    }
}

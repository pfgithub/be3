use std::ops::{Add, Index, IndexMut};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector<const N: usize, T> {
    pub values: [T; N],
}

impl<const N: usize, T> Vector<N, T> {
    pub const fn from_array(values: [T; N]) -> Self {
        Self { values }
    }

    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Vector<N, U> {
        Vector::from_array(self.values.map(|value| map(value)))
    }
}

impl<T> Vector<2, T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { values: [x, y] }
    }
}

impl<const N: usize, T: Default + Copy> Default for Vector<N, T> {
    fn default() -> Self {
        Self::from_array([T::default(); N])
    }
}

impl<const N: usize, T> Index<usize> for Vector<N, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl<const N: usize, T> IndexMut<usize> for Vector<N, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.values[index]
    }
}

impl<const N: usize, T: Add<Output = T> + Copy> Add for Vector<N, T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from_array(std::array::from_fn(|index| self[index] + rhs[index]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeRecommendation {
    pub size: Vector<2, Option<f32>>,
}

impl SizeRecommendation {
    pub const fn new(size: Vector<2, Option<f32>>) -> Self {
        Self { size }
    }

    pub const fn exact(size: Vector<2, f32>) -> Self {
        Self {
            size: Vector::new(Some(size.values[0]), Some(size.values[1])),
        }
    }

    pub(crate) fn main(self, axis: Axis) -> Option<f32> {
        self.size[axis.index()]
    }

    pub(crate) fn with_main(self, axis: Axis, value: Option<f32>) -> Self {
        let mut size = self.size;
        size[axis.index()] = value;
        Self::new(size)
    }
}

pub type Size = Vector<2, f32>;

impl Vector<2, f32> {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub(crate) fn main(self, axis: Axis) -> f32 {
        self[axis.index()]
    }

    pub(crate) fn cross(self, axis: Axis) -> f32 {
        self[axis.cross().index()]
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
    pub position: Vector<2, f32>,
    pub size: Vector<2, f32>,
}

impl Rect {
    pub const fn new(position: Vector<2, f32>, size: Vector<2, f32>) -> Self {
        Self { position, size }
    }

    pub const fn size(self) -> Size {
        self.size
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub const fn index(self) -> usize {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }

    pub const fn cross(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
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

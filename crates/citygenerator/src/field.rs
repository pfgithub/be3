use glam::Vec2;

use crate::Polygon;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Eigenvector {
    Major,
    Minor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tensor {
    magnitude: f32,
    matrix: Vec2,
}

impl Tensor {
    pub const ZERO: Self = Self {
        magnitude: 0.0,
        matrix: Vec2::ZERO,
    };

    pub fn new(magnitude: f32, matrix: Vec2) -> Self {
        Self { magnitude, matrix }
    }

    pub fn from_angle(angle: f32) -> Self {
        Self::new(1.0, Vec2::from_angle(angle * 4.0))
    }

    pub fn from_vector(vector: Vec2) -> Self {
        let t1 = vector.x * vector.x - vector.y * vector.y;
        let t2 = 2.0 * vector.x * vector.y;
        Self::new(1.0, Vec2::new(t1 * t1 - t2 * t2, 2.0 * t1 * t2))
    }

    pub fn theta(self) -> f32 {
        if self.magnitude == 0.0 {
            0.0
        } else {
            (self.matrix / self.magnitude).to_angle() * 0.5
        }
    }

    pub fn add(&mut self, other: Self, smooth: bool) {
        self.matrix = self.matrix * self.magnitude + other.matrix * other.magnitude;
        if smooth {
            self.magnitude = self.matrix.length();
            if self.magnitude > 0.0 {
                self.matrix /= self.magnitude;
            }
        } else {
            self.magnitude = 2.0;
        }
    }

    pub fn scaled(mut self, scale: f32) -> Self {
        self.magnitude *= scale;
        self
    }

    pub fn rotated(mut self, angle: f32) -> Self {
        if angle != 0.0 && self.magnitude != 0.0 {
            let theta = (self.theta() + angle).rem_euclid(std::f32::consts::PI);
            self.matrix = Vec2::from_angle(theta * 2.0) * self.magnitude;
        }
        self
    }

    pub fn major(self) -> Vec2 {
        if self.magnitude == 0.0 {
            Vec2::ZERO
        } else {
            Vec2::from_angle(self.theta())
        }
    }

    pub fn minor(self) -> Vec2 {
        if self.magnitude == 0.0 {
            Vec2::ZERO
        } else {
            Vec2::from_angle(self.theta() + std::f32::consts::FRAC_PI_2)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BasisKind {
    Grid { angle: f32 },
    Radial,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BasisField {
    pub center: Vec2,
    pub size: f32,
    pub decay: f32,
    pub kind: BasisKind,
}

impl BasisField {
    pub fn tensor(self, point: Vec2) -> Tensor {
        match self.kind {
            BasisKind::Grid { angle } => Tensor::new(1.0, Vec2::from_angle(angle * 2.0)),
            BasisKind::Radial => {
                let offset = point - self.center;
                Tensor::new(
                    1.0,
                    Vec2::new(
                        offset.y * offset.y - offset.x * offset.x,
                        -2.0 * offset.x * offset.y,
                    ),
                )
            }
        }
    }

    fn weight(self, point: Vec2, smooth: bool) -> f32 {
        let normalized = point.distance(self.center) / self.size.max(f32::EPSILON);
        if smooth {
            if normalized == 0.0 {
                return 1.0;
            }
            normalized.powf(-self.decay).min(1.0e6)
        } else if self.decay == 0.0 && normalized >= 1.0 {
            0.0
        } else {
            (1.0 - normalized).max(0.0).powf(self.decay)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseParams {
    pub park_size: f32,
    pub park_angle_degrees: f32,
    pub global_size: f32,
    pub global_angle_degrees: f32,
    pub global: bool,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            park_size: 20.0,
            park_angle_degrees: 90.0,
            global_size: 30.0,
            global_angle_degrees: 20.0,
            global: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TensorField {
    seed: u64,
    basis_fields: Vec<BasisField>,
    pub noise: NoiseParams,
    pub parks: Vec<Polygon>,
    pub sea: Polygon,
    pub river: Polygon,
    pub ignore_river: bool,
    pub smooth: bool,
}

impl TensorField {
    pub fn new(seed: u64, noise: NoiseParams) -> Self {
        Self {
            seed,
            basis_fields: Vec::new(),
            noise,
            parks: Vec::new(),
            sea: Vec::new(),
            river: Vec::new(),
            ignore_river: false,
            smooth: false,
        }
    }

    pub fn basis_fields(&self) -> &[BasisField] {
        &self.basis_fields
    }

    pub fn add_grid(&mut self, center: Vec2, size: f32, decay: f32, angle: f32) {
        self.basis_fields.push(BasisField {
            center,
            size,
            decay,
            kind: BasisKind::Grid { angle },
        });
    }

    pub fn add_radial(&mut self, center: Vec2, size: f32, decay: f32) {
        self.basis_fields.push(BasisField {
            center,
            size,
            decay,
            kind: BasisKind::Radial,
        });
    }

    pub fn clear_basis_fields(&mut self) {
        self.basis_fields.clear();
    }

    pub fn sample(&self, point: Vec2) -> Tensor {
        if !self.on_land(point) {
            return Tensor::ZERO;
        }
        if self.basis_fields.is_empty() {
            return Tensor::new(1.0, Vec2::X);
        }
        let mut tensor = Tensor::ZERO;
        for basis in &self.basis_fields {
            tensor.add(
                basis.tensor(point).scaled(basis.weight(point, self.smooth)),
                self.smooth,
            );
        }
        if self
            .parks
            .iter()
            .any(|park| crate::geometry::point_in_polygon(point, park))
        {
            tensor = tensor.rotated(self.rotational_noise(
                point,
                self.noise.park_size,
                self.noise.park_angle_degrees,
            ));
        }
        if self.noise.global {
            tensor = tensor.rotated(self.rotational_noise(
                point,
                self.noise.global_size,
                self.noise.global_angle_degrees,
            ));
        }
        tensor
    }

    pub fn direction(&self, point: Vec2, eigenvector: Eigenvector) -> Vec2 {
        match eigenvector {
            Eigenvector::Major => self.sample(point).major(),
            Eigenvector::Minor => self.sample(point).minor(),
        }
    }

    pub fn on_land(&self, point: Vec2) -> bool {
        !crate::geometry::point_in_polygon(point, &self.sea)
            && (self.ignore_river || !crate::geometry::point_in_polygon(point, &self.river))
    }

    fn rotational_noise(&self, point: Vec2, size: f32, angle_degrees: f32) -> f32 {
        let scale = size.max(f32::EPSILON);
        let phase = self.seed as f32 * 0.000_001_7;
        let x = point.x / scale;
        let y = point.y / scale;
        let noise = (x * 1.71 + phase).sin() * (y * 1.37 - phase).cos() * 0.7
            + (x * 3.11 - y * 2.73 + phase).sin() * 0.3;
        noise * angle_degrees.to_radians()
    }
}

#[cfg(test)]
mod tests;

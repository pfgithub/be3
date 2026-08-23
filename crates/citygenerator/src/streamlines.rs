use std::collections::HashMap;

use glam::Vec2;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::field::{Eigenvector, TensorField};
use crate::geometry::simplify_polyline;
use crate::Polyline;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoadClass {
    Main,
    Major,
    Minor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamlineParams {
    pub dsep: f32,
    pub dtest: f32,
    pub dstep: f32,
    pub dcirclejoin: f32,
    pub dlookahead: f32,
    pub join_angle: f32,
    pub path_iterations: usize,
    pub seed_tries: usize,
    pub simplify_tolerance: f32,
    pub collide_early: f32,
}

impl Default for StreamlineParams {
    fn default() -> Self {
        Self {
            dsep: 20.0,
            dtest: 15.0,
            dstep: 1.0,
            dcirclejoin: 5.0,
            dlookahead: 40.0,
            join_angle: 0.1,
            path_iterations: 1_000,
            seed_tries: 300,
            simplify_tolerance: 0.5,
            collide_early: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StreamlineGenerator {
    field: TensorField,
    origin: Vec2,
    dimensions: Vec2,
    params: StreamlineParams,
    rng: ChaCha8Rng,
    existing: PointGrid,
    major: PointGrid,
    minor: PointGrid,
}

impl StreamlineGenerator {
    pub fn new(
        field: TensorField,
        origin: Vec2,
        dimensions: Vec2,
        mut params: StreamlineParams,
        seed: u64,
    ) -> Self {
        params.dtest = params.dtest.min(params.dsep);
        Self {
            field,
            origin,
            dimensions,
            params,
            rng: ChaCha8Rng::seed_from_u64(seed),
            existing: PointGrid::new(params.dsep),
            major: PointGrid::new(params.dsep),
            minor: PointGrid::new(params.dsep),
        }
    }

    pub fn add_existing(&mut self, lines: &[Polyline]) {
        for line in lines {
            self.existing.add_polyline(line);
        }
    }

    pub fn generate(&mut self) -> Vec<Polyline> {
        let mut output = Vec::new();
        let mut major = true;
        while let Some(seed) = self.seed(major) {
            let line = self.integrate_streamline(seed, major);
            if line.len() > 5 {
                self.grid_mut(major).add_polyline(&line);
                output.push(simplify_polyline(&line, self.params.simplify_tolerance));
            }
            major = !major;
        }
        output
    }

    pub fn integrate_from(&self, seed: Vec2, eigenvector: Eigenvector) -> Polyline {
        self.integrate_streamline(seed, eigenvector == Eigenvector::Major)
    }

    fn seed(&mut self, major: bool) -> Option<Vec2> {
        for _ in 0..=self.params.seed_tries {
            let point = self.origin
                + Vec2::new(
                    self.rng.random::<f32>() * self.dimensions.x,
                    self.rng.random::<f32>() * self.dimensions.y,
                );
            if self.field.on_land(point)
                && self
                    .grid(major)
                    .is_valid(point, self.params.dsep * self.params.dsep)
                && self
                    .existing
                    .is_valid(point, self.params.dsep * self.params.dsep)
            {
                return Some(point);
            }
        }
        None
    }

    fn integrate_streamline(&self, seed: Vec2, major: bool) -> Polyline {
        let initial = self.integrate(seed, major);
        if initial.length_squared() < 0.01 {
            return Vec::new();
        }
        let mut forward = Integration::new(seed, initial);
        forward.valid = self.in_bounds(forward.previous_point);
        let mut backward = Integration::new(seed, -initial);
        backward.valid = self.in_bounds(backward.previous_point);
        let mut escaped = false;
        let circle_distance_sq = self.params.dcirclejoin * self.params.dcirclejoin;

        for _ in 0..self.params.path_iterations {
            self.integration_step(&mut forward, major);
            self.integration_step(&mut backward, major);
            let distance = forward
                .previous_point
                .distance_squared(backward.previous_point);
            if !escaped && distance > circle_distance_sq {
                escaped = true;
            }
            if escaped && distance <= circle_distance_sq {
                forward.points.push(forward.previous_point);
                forward.points.push(backward.previous_point);
                backward.points.push(backward.previous_point);
                break;
            }
            if !forward.valid && !backward.valid {
                break;
            }
        }
        backward.points.reverse();
        backward.points.extend(forward.points);
        backward.points
    }

    fn integration_step(&self, integration: &mut Integration, major: bool) {
        if !integration.valid {
            return;
        }
        integration.points.push(integration.previous_point);
        let mut direction = self.integrate(integration.previous_point, major);
        if direction.length_squared() < 0.01 {
            integration.valid = false;
            return;
        }
        if direction.dot(integration.previous_direction) < 0.0 {
            direction = -direction;
        }
        let next = integration.previous_point + direction;
        let valid = self.in_bounds(next)
            && self.field.on_land(next)
            && self
                .grid(major)
                .is_valid(next, self.params.dtest * self.params.dtest)
            && self
                .existing
                .is_valid(next, self.params.dtest * self.params.dtest)
            && !streamline_turned(
                integration.seed,
                integration.original_direction,
                next,
                direction,
            );
        if valid {
            integration.previous_point = next;
            integration.previous_direction = direction;
        } else {
            integration.points.push(next);
            integration.valid = false;
        }
    }

    fn integrate(&self, point: Vec2, major: bool) -> Vec2 {
        let eigenvector = if major {
            Eigenvector::Major
        } else {
            Eigenvector::Minor
        };

        let k1 = self.field.direction(point, eigenvector);
        let diagonal = Vec2::splat(self.params.dstep * 0.5);
        let k23 = self.field.direction(point + diagonal, eigenvector);
        let k4 = self
            .field
            .direction(point + Vec2::splat(self.params.dstep), eigenvector);
        (k1 + k23 * 4.0 + k4) * (self.params.dstep / 6.0)
    }

    fn in_bounds(&self, point: Vec2) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.origin.x + self.dimensions.x
            && point.y < self.origin.y + self.dimensions.y
    }

    fn grid(&self, major: bool) -> &PointGrid {
        if major {
            &self.major
        } else {
            &self.minor
        }
    }

    fn grid_mut(&mut self, major: bool) -> &mut PointGrid {
        if major {
            &mut self.major
        } else {
            &mut self.minor
        }
    }
}

#[derive(Clone, Debug)]
struct Integration {
    seed: Vec2,
    original_direction: Vec2,
    points: Polyline,
    previous_direction: Vec2,
    previous_point: Vec2,
    valid: bool,
}

impl Integration {
    fn new(seed: Vec2, direction: Vec2) -> Self {
        Self {
            seed,
            original_direction: direction,
            points: vec![seed],
            previous_direction: direction,
            previous_point: seed + direction,
            valid: true,
        }
    }
}

fn streamline_turned(seed: Vec2, original: Vec2, point: Vec2, direction: Vec2) -> bool {
    if original.dot(direction) >= 0.0 {
        return false;
    }
    let perpendicular = Vec2::new(original.y, -original.x);
    let is_left = (point - seed).dot(perpendicular) < 0.0;
    let direction_up = direction.dot(perpendicular) > 0.0;
    is_left == direction_up
}

#[derive(Clone, Debug)]
struct PointGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<Vec2>>,
}

impl PointGrid {
    fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    fn add_polyline(&mut self, line: &[Vec2]) {
        for &point in line {
            self.cells.entry(self.cell(point)).or_default().push(point);
        }
    }

    fn is_valid(&self, point: Vec2, distance_squared: f32) -> bool {
        let cell = self.cell(point);
        for x in cell.0 - 1..=cell.0 + 1 {
            for y in cell.1 - 1..=cell.1 + 1 {
                if self.cells.get(&(x, y)).is_some_and(|points| {
                    points
                        .iter()
                        .any(|sample| sample.distance_squared(point) < distance_squared)
                }) {
                    return false;
                }
            }
        }
        true
    }

    fn cell(&self, point: Vec2) -> (i32, i32) {
        (
            (point.x / self.cell_size).floor() as i32,
            (point.y / self.cell_size).floor() as i32,
        )
    }
}

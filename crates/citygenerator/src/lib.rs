//! Procedural city generation ported from ProbableTrain/MapGenerator.
//!
//! The crate contains no rendering code. All output is world-space geometry and
//! all random choices are driven by the seed passed to [`CityGenerator`].

mod field;
mod geometry;
mod graph;
mod streamlines;

pub use field::{BasisField, BasisKind, Eigenvector, NoiseParams, Tensor, TensorField};
pub use graph::{Graph, Node};
pub use streamlines::{RoadClass, StreamlineGenerator, StreamlineParams};

use glam::Vec2;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use geometry::{average_point, inset_polygon, point_in_polygon, polygon_area, subdivide_polygon};

pub type Polyline = Vec<Vec2>;
pub type Polygon = Vec<Vec2>;

#[derive(Clone, Debug, PartialEq)]
pub struct Road {
    pub centerline: Polyline,
    pub class: RoadClass,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    pub footprint: Polygon,
    pub height: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Water {
    pub coastline: Polyline,
    pub sea: Polygon,
    pub river: Polygon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct City {
    pub origin: Vec2,
    pub dimensions: Vec2,
    pub roads: Vec<Road>,
    pub parks: Vec<Polygon>,
    pub blocks: Vec<Polygon>,
    pub buildings: Vec<Building>,
    pub water: Water,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratorConfig {
    pub origin: Vec2,
    pub dimensions: Vec2,
    pub smooth_field: bool,
    pub generate_water: bool,
    pub big_parks: usize,
    pub small_parks: usize,
    pub building_min_area: f32,
    pub building_setback: f32,
    pub chance_no_divide: f32,
    pub main_roads: StreamlineParams,
    pub major_roads: StreamlineParams,
    pub minor_roads: StreamlineParams,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        let minor = StreamlineParams::default();
        Self {
            origin: Vec2::ZERO,
            dimensions: Vec2::new(1_440.0, 900.0),
            smooth_field: false,
            generate_water: true,
            big_parks: 2,
            small_parks: 0,
            building_min_area: 50.0,
            building_setback: 4.0,
            chance_no_divide: 0.05,
            main_roads: StreamlineParams {
                dsep: 400.0,
                dtest: 200.0,
                dlookahead: 500.0,
                ..minor
            },
            major_roads: StreamlineParams {
                dsep: 100.0,
                dtest: 30.0,
                dlookahead: 200.0,
                ..minor
            },
            minor_roads: minor,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CityGenerator {
    config: GeneratorConfig,
    field: TensorField,
}

impl CityGenerator {
    pub fn new(seed: u64, config: GeneratorConfig) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut field = TensorField::new(seed, NoiseParams::default());
        field.smooth = config.smooth_field;
        add_recommended_fields(&mut field, &config, &mut rng);
        Self { config, field }
    }

    pub fn with_field(config: GeneratorConfig, field: TensorField) -> Self {
        Self { config, field }
    }

    pub fn field(&self) -> &TensorField {
        &self.field
    }

    pub fn field_mut(&mut self) -> &mut TensorField {
        &mut self.field
    }

    pub fn generate(&self, seed: u64) -> City {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut field = self.field.clone();
        let water = if self.config.generate_water {
            generate_water(&mut field, &self.config, &mut rng)
        } else {
            Water::default()
        };

        let mut occupied = Vec::new();
        if !water.coastline.is_empty() {
            occupied.push(water.coastline.clone());
        }
        if !water.river.is_empty() {
            occupied.push(water.river.clone());
        }
        let mut roads = Vec::new();
        for (class, params, width) in [
            (RoadClass::Main, self.config.main_roads, 5.0),
            (RoadClass::Major, self.config.major_roads, 4.0),
            (RoadClass::Minor, self.config.minor_roads, 2.0),
        ] {
            let mut generator = StreamlineGenerator::new(
                field.clone(),
                self.config.origin,
                self.config.dimensions,
                params,
                rng.random(),
            );
            generator.add_existing(&occupied);
            let lines = generator.generate();
            occupied.extend(lines.iter().cloned());
            roads.extend(lines.into_iter().map(|centerline| Road {
                centerline,
                class,
                width,
            }));

            if class == RoadClass::Major && self.config.big_parks > 0 {
                let candidates = find_blocks(&occupied, &field, params.dstep);
                field.parks = choose_polygons(candidates, self.config.big_parks, &mut rng);
            }
        }

        let blocks = find_blocks(&occupied, &field, self.config.minor_roads.dstep);
        let mut parks = field.parks.clone();
        if self.config.small_parks > 0 {
            parks.extend(choose_polygons(
                blocks.clone(),
                self.config.small_parks,
                &mut rng,
            ));
        }
        field.parks = parks.clone();

        let mut buildings = Vec::new();
        for block in &blocks {
            let Some(inset) = inset_polygon(block, self.config.building_setback) else {
                continue;
            };
            if parks
                .iter()
                .any(|park| point_in_polygon(average_point(&inset), park))
            {
                continue;
            }
            let lots = if rng.random::<f32>() < self.config.chance_no_divide {
                vec![inset]
            } else {
                subdivide_polygon(inset, self.config.building_min_area, &mut rng)
            };
            for footprint in lots {
                if polygon_area(&footprint) >= self.config.building_min_area * 0.5 {
                    buildings.push(Building {
                        height: rng.random_range(20.0..40.0),
                        footprint,
                    });
                }
            }
        }
        buildings.sort_by(|a, b| a.height.total_cmp(&b.height));

        City {
            origin: self.config.origin,
            dimensions: self.config.dimensions,
            roads,
            parks,
            blocks,
            buildings,
            water,
        }
    }
}

fn add_recommended_fields(field: &mut TensorField, config: &GeneratorConfig, rng: &mut ChaCha8Rng) {
    let size = config.dimensions * 0.7;
    let origin = config.origin + config.dimensions * 0.15;
    for center in [
        origin,
        origin + size,
        origin + Vec2::new(size.x, 0.0),
        origin + Vec2::new(0.0, size.y),
    ] {
        field.add_grid(
            center,
            rng.random_range(config.dimensions.x / 4.0..config.dimensions.x),
            rng.random_range(0.0..50.0),
            rng.random_range(0.0..std::f32::consts::FRAC_PI_2),
        );
    }
    field.add_radial(
        config.origin
            + Vec2::new(
                rng.random_range(0.15..0.85) * config.dimensions.x,
                rng.random_range(0.15..0.85) * config.dimensions.y,
            ),
        rng.random_range(config.dimensions.x / 10.0..config.dimensions.x / 5.0),
        rng.random_range(0.0..50.0),
    );
}

fn choose_polygons(mut polygons: Vec<Polygon>, count: usize, rng: &mut ChaCha8Rng) -> Vec<Polygon> {
    let mut selected = Vec::new();
    for _ in 0..count.min(polygons.len()) {
        selected.push(polygons.swap_remove(rng.random_range(0..polygons.len())));
    }
    selected
}

fn find_blocks(lines: &[Polyline], field: &TensorField, dstep: f32) -> Vec<Polygon> {
    Graph::from_streamlines(lines, dstep, true)
        .polygons(20)
        .into_iter()
        .filter(|polygon| field.on_land(average_point(polygon)))
        .collect()
}

fn generate_water(
    field: &mut TensorField,
    config: &GeneratorConfig,
    rng: &mut ChaCha8Rng,
) -> Water {
    let horizontal = rng.random_bool(0.5);
    let phase = rng.random_range(0.0..std::f32::consts::TAU);
    let mut coastline = Vec::new();
    let samples = 80;
    for index in 0..=samples {
        let t = index as f32 / samples as f32;
        let wave = ((t * 3.0 * std::f32::consts::TAU) + phase).sin() * 35.0
            + ((t * 7.0 * std::f32::consts::TAU) - phase).sin() * 10.0;
        let point = if horizontal {
            config.origin + Vec2::new(t * config.dimensions.x, config.dimensions.y * 0.14 + wave)
        } else {
            config.origin + Vec2::new(config.dimensions.x * 0.14 + wave, t * config.dimensions.y)
        };
        coastline.push(point);
    }
    let mut sea = coastline.clone();
    if horizontal {
        sea.push(config.origin + Vec2::new(config.dimensions.x, 0.0));
        sea.push(config.origin);
    } else {
        sea.push(config.origin + Vec2::new(0.0, config.dimensions.y));
        sea.push(config.origin);
    }
    field.sea = sea.clone();

    let river_phase = rng.random_range(0.0..std::f32::consts::TAU);
    let mut river_centerline = Vec::new();
    for index in 0..=samples {
        let t = index as f32 / samples as f32;
        let wave = ((t * 2.0 * std::f32::consts::TAU) + river_phase).sin() * 28.0
            + ((t * 5.0 * std::f32::consts::TAU) - river_phase).sin() * 7.0;
        let point = if horizontal {
            config.origin + Vec2::new(config.dimensions.x * 0.62 + wave, t * config.dimensions.y)
        } else {
            config.origin + Vec2::new(t * config.dimensions.x, config.dimensions.y * 0.62 + wave)
        };
        river_centerline.push(point);
    }
    let river = geometry::polyline_band(&river_centerline, 20.0);
    field.river = river.clone();
    Water {
        coastline,
        sea,
        river,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let generator = CityGenerator::new(7, GeneratorConfig::default());
        assert_eq!(generator.generate(11), generator.generate(11));
        assert_ne!(generator.generate(11), generator.generate(12));
    }

    #[test]
    fn generated_geometry_stays_in_the_domain() {
        let config = GeneratorConfig {
            generate_water: false,
            ..GeneratorConfig::default()
        };
        let city = CityGenerator::new(4, config.clone()).generate(9);
        assert!(!city.roads.is_empty());
        assert!(!city.blocks.is_empty());
        assert!(!city.buildings.is_empty());
        let margin = config
            .main_roads
            .dstep
            .max(config.major_roads.dstep)
            .max(config.minor_roads.dstep);
        for point in city.roads.iter().flat_map(|road| &road.centerline) {
            assert!(
                point.x >= config.origin.x - margin - 1.0e-3,
                "x below domain: {point:?}"
            );
            assert!(
                point.y >= config.origin.y - margin - 1.0e-3,
                "y below domain: {point:?}"
            );
            assert!(
                point.x < config.origin.x + config.dimensions.x + margin + 1.0e-3,
                "x above domain: {point:?}"
            );
            assert!(
                point.y < config.origin.y + config.dimensions.y + margin + 1.0e-3,
                "y above domain: {point:?}"
            );
        }
    }

    #[test]
    fn generated_sea_uses_the_smaller_side_of_the_coastline() {
        let config = GeneratorConfig::default();
        let domain_area = config.dimensions.x * config.dimensions.y;
        let mut saw_horizontal = false;
        let mut saw_vertical = false;

        for seed in 0..100 {
            let mut field = TensorField::new(seed, NoiseParams::default());
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let water = generate_water(&mut field, &config, &mut rng);
            let coast_span = *water.coastline.last().unwrap() - *water.coastline.first().unwrap();
            let horizontal = coast_span.x.abs() > coast_span.y.abs();
            saw_horizontal |= horizontal;
            saw_vertical |= !horizontal;
            assert!(
                polygon_area(&water.sea) < domain_area * 0.5,
                "seed {seed} generated sea over most of the domain"
            );

            if saw_horizontal && saw_vertical {
                return;
            }
        }

        panic!("test seeds did not cover both coastline orientations");
    }
}

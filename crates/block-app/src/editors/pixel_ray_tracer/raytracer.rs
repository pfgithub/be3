use block_client::blocks::pixel_ray_tracer::{
    Point, RayEntity, RaySettings, PIXEL_RAY_TRACER_PALETTE, PIXEL_RAY_TRACER_SIZE,
};

const AMBIENT_LIGHT: f32 = 0.15;
const LIGHT_FALLOFF_SOFTENING_SQUARED: f32 = 64.0;
const MINIMUM_POWER: f32 = 0.002;
const MAXIMUM_BRANCHES: usize = 48;

#[derive(Clone, Copy)]
struct Branch {
    position: Point,
    direction: Point,
    power: [f32; 3],
    traveled: f32,
    water: Option<u64>,
    random: u32,
}

#[derive(Clone, Copy)]
struct Hit {
    entity: usize,
    distance: f32,
    normal: Point,
}

pub(super) struct RayTraceResult {
    pub pixels: Vec<[u8; 4]>,
    pub debug_positions: Vec<Point>,
}

pub(super) fn trace_lighting(
    pixels: &[u8],
    entities: &[RayEntity],
    settings: RaySettings,
) -> Vec<[u8; 4]> {
    let size = usize::from(PIXEL_RAY_TRACER_SIZE);
    let mut illumination = vec![[0.0_f32; 3]; size * size];
    for light in entities {
        let RayEntity::Light {
            position,
            color_index,
            intensity,
            ..
        } = light
        else {
            continue;
        };
        let color = PIXEL_RAY_TRACER_PALETTE[usize::from(*color_index)];
        let mut accumulated = vec![[0.0_f32; 3]; size * size];
        let mut counts = vec![0_u32; size * size];
        for ray in 0..settings.ray_count {
            let angle = f32::from(ray) / f32::from(settings.ray_count) * std::f32::consts::TAU;
            let mut branches = vec![create_branch(
                *position,
                angle,
                entities,
                u32::from(ray) + 1,
            )];
            for _ in 0..settings.maximum_steps {
                if branches.is_empty() {
                    break;
                }
                branches = move_branches(branches, settings.step_distance, entities);
                for branch in &branches {
                    let x = branch
                        .position
                        .x
                        .floor()
                        .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1))
                        as usize;
                    let y = branch
                        .position
                        .y
                        .floor()
                        .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1))
                        as usize;
                    let index = y * size + x;
                    let attenuation = *intensity * LIGHT_FALLOFF_SOFTENING_SQUARED
                        / (branch.traveled * branch.traveled + LIGHT_FALLOFF_SOFTENING_SQUARED);
                    for channel in 0..3 {
                        accumulated[index][channel] += attenuation * branch.power[channel];
                    }
                    counts[index] += 1;
                }
            }
        }
        for index in 0..illumination.len() {
            if counts[index] == 0 {
                continue;
            }
            for channel in 0..3 {
                illumination[index][channel] += f32::from(color[channel]) / 255.0
                    * accumulated[index][channel]
                    / counts[index] as f32;
            }
        }
    }
    pixels
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let base = PIXEL_RAY_TRACER_PALETTE[usize::from(*color)];
            let mut output = [0, 0, 0, 255];
            for channel in 0..3 {
                output[channel] = (f32::from(base[channel])
                    * (AMBIENT_LIGHT + illumination[index][channel]))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            output
        })
        .collect()
}

pub(super) fn trace_rays(
    source: &[[u8; 4]],
    entities: &[RayEntity],
    origin: Point,
    settings: RaySettings,
    include_debug: bool,
) -> RayTraceResult {
    let size = usize::from(PIXEL_RAY_TRACER_SIZE);
    let mut accumulated = vec![[0.0_f32; 3]; size * size];
    let mut counts = vec![0_u32; size * size];
    let mut debug_positions = Vec::new();
    for ray in 0..settings.ray_count {
        let angle = f32::from(ray) / f32::from(settings.ray_count) * std::f32::consts::TAU;
        let direction = Point::new(angle.cos(), angle.sin());
        let mut render = origin;
        let mut branches = vec![create_branch(origin, angle, entities, u32::from(ray) + 1)];
        for _ in 0..settings.maximum_steps {
            render.x += direction.x * settings.step_distance;
            render.y += direction.y * settings.step_distance;
            if !inside(render) {
                break;
            }
            if include_debug {
                debug_positions.push(render);
            }
            branches = move_branches(branches, settings.step_distance, entities);
            if branches.is_empty() {
                break;
            }
            let target = render.y.floor() as usize * size + render.x.floor() as usize;
            for branch in &branches {
                let x = branch
                    .position
                    .x
                    .floor()
                    .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1))
                    as usize;
                let y = branch
                    .position
                    .y
                    .floor()
                    .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1))
                    as usize;
                let color = source[y * size + x];
                for channel in 0..3 {
                    accumulated[target][channel] +=
                        f32::from(color[channel]) * branch.power[channel];
                }
            }
            counts[target] += 1;
        }
    }
    let pixels = accumulated
        .into_iter()
        .zip(counts)
        .map(|(color, count)| {
            let mut result = [0, 0, 0, 255];
            if count > 0 {
                for channel in 0..3 {
                    result[channel] =
                        (color[channel] / count as f32).round().clamp(0.0, 255.0) as u8;
                }
            }
            result
        })
        .collect();
    RayTraceResult {
        pixels,
        debug_positions,
    }
}

fn create_branch(position: Point, angle: f32, entities: &[RayEntity], random: u32) -> Branch {
    Branch {
        position,
        direction: Point::new(angle.cos(), angle.sin()),
        power: [1.0; 3],
        traveled: 0.0,
        water: entities.iter().rev().find_map(|entity| match entity {
            RayEntity::Water { id, start, end } if rectangle_contains(position, *start, *end) => {
                Some(*id)
            }
            _ => None,
        }),
        random,
    }
}

fn move_branches(branches: Vec<Branch>, distance: f32, entities: &[RayEntity]) -> Vec<Branch> {
    let mut pending: Vec<(Branch, f32, u8)> = branches
        .into_iter()
        .map(|branch| (branch, distance, 0))
        .collect();
    let mut moved = Vec::new();
    while let Some((mut branch, remaining, collisions)) = pending.pop() {
        if branch.power.into_iter().fold(0.0, f32::max) < MINIMUM_POWER || collisions > 32 {
            continue;
        }
        let movement = Point::new(
            branch.direction.x * remaining,
            branch.direction.y * remaining,
        );
        let Some(hit) = closest_hit(branch.position, movement, entities) else {
            branch.position.x += movement.x;
            branch.position.y += movement.y;
            branch.traveled += remaining;
            if inside(branch.position) {
                moved.push(branch);
            }
            continue;
        };
        let traveled = remaining * hit.distance;
        branch.position.x += movement.x * hit.distance;
        branch.position.y += movement.y * hit.distance;
        branch.traveled += traveled;
        let remaining = remaining - traveled;
        let mut normal = hit.normal;
        let mut cosine = -(branch.direction.x * normal.x + branch.direction.y * normal.y);
        if cosine < 0.0 {
            normal.x = -normal.x;
            normal.y = -normal.y;
            cosine = -cosine;
        }
        let reflected_direction = Point::new(
            branch.direction.x + 2.0 * cosine * normal.x,
            branch.direction.y + 2.0 * cosine * normal.y,
        );
        match &entities[hit.entity] {
            RayEntity::Surface {
                color_index,
                roughness,
                metalness,
                transmission,
                refractive_index,
                ..
            } => {
                let base = PIXEL_RAY_TRACER_PALETTE[usize::from(*color_index)]
                    .map(|value| f32::from(value) / 255.0);
                let dielectric = ((*refractive_index - 1.0) / (*refractive_index + 1.0)).powi(2);
                let grazing = (1.0 - cosine).powi(5);
                let mut reflected = branch;
                reflected.direction =
                    rough_direction(reflected_direction, *roughness, &mut reflected.random);
                for channel in 0..3 {
                    let reflectance = dielectric + (base[channel] - dielectric) * *metalness;
                    reflected.power[channel] *= reflectance + (1.0 - reflectance) * grazing;
                }
                separate(&mut reflected);
                pending.push((reflected, remaining, collisions + 1));
                if *transmission > 0.0 && *metalness < 1.0 {
                    let mut transmitted = branch;
                    transmitted.direction =
                        rough_direction(branch.direction, *roughness, &mut transmitted.random);
                    for channel in 0..3 {
                        transmitted.power[channel] *=
                            *transmission * (1.0 - *metalness) * base[channel] * (1.0 - grazing);
                    }
                    separate(&mut transmitted);
                    pending.push((transmitted, remaining, collisions + 1));
                }
            }
            RayEntity::Water { id, .. } => {
                let exiting = branch.water == Some(*id);
                let (source, target) = if exiting { (1.333, 1.0) } else { (1.0, 1.333) };
                let ratio = source / target;
                let term = 1.0 - ratio * ratio * (1.0 - cosine * cosine);
                let base = ((source - target) / (source + target)).powi(2);
                let reflectance = if term < 0.0 {
                    1.0
                } else {
                    base + (1.0 - base) * (1.0 - cosine).powi(5)
                };
                let mut reflected = branch;
                reflected.direction = reflected_direction;
                for power in &mut reflected.power {
                    *power *= reflectance;
                }
                separate(&mut reflected);
                pending.push((reflected, remaining, collisions + 1));
                if term >= 0.0 {
                    let mut refracted = branch;
                    refracted.direction = Point::new(
                        ratio * branch.direction.x + (ratio * cosine - term.sqrt()) * normal.x,
                        ratio * branch.direction.y + (ratio * cosine - term.sqrt()) * normal.y,
                    );
                    refracted.water = if exiting { None } else { Some(*id) };
                    for power in &mut refracted.power {
                        *power *= 1.0 - reflectance;
                    }
                    separate(&mut refracted);
                    pending.push((refracted, remaining, collisions + 1));
                }
            }
            RayEntity::Light { .. } => unreachable!(),
        }
    }
    moved.sort_by(|left, right| {
        right
            .power
            .into_iter()
            .fold(0.0, f32::max)
            .total_cmp(&left.power.into_iter().fold(0.0, f32::max))
    });
    moved.truncate(MAXIMUM_BRANCHES);
    moved
}

fn separate(branch: &mut Branch) {
    branch.position.x += branch.direction.x * 0.000_001;
    branch.position.y += branch.direction.y * 0.000_001;
    branch.traveled += 0.000_001;
}

fn rough_direction(direction: Point, roughness: f32, random: &mut u32) -> Point {
    *random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let offset = (*random as f32 / 4_294_967_296.0 * 2.0 - 1.0)
        * roughness
        * roughness
        * std::f32::consts::FRAC_PI_2;
    Point::new(
        direction.x * offset.cos() - direction.y * offset.sin(),
        direction.x * offset.sin() + direction.y * offset.cos(),
    )
}

fn closest_hit(position: Point, movement: Point, entities: &[RayEntity]) -> Option<Hit> {
    let mut closest: Option<Hit> = None;
    let mut consider = |entity, start, end| {
        if let Some((distance, normal)) = segment_hit(position, movement, start, end) {
            if closest.is_none_or(|hit| distance < hit.distance) {
                closest = Some(Hit {
                    entity,
                    distance,
                    normal,
                });
            }
        }
    };
    for (index, entity) in entities.iter().enumerate() {
        match entity {
            RayEntity::Surface { start, end, .. } => consider(index, *start, *end),
            RayEntity::Water { start, end, .. } => {
                let left = start.x.min(end.x);
                let right = start.x.max(end.x);
                let top = start.y.min(end.y);
                let bottom = start.y.max(end.y);
                consider(index, Point::new(left, top), Point::new(right, top));
                consider(index, Point::new(right, top), Point::new(right, bottom));
                consider(index, Point::new(right, bottom), Point::new(left, bottom));
                consider(index, Point::new(left, bottom), Point::new(left, top));
            }
            RayEntity::Light { .. } => {}
        }
    }
    closest
}

fn segment_hit(position: Point, movement: Point, start: Point, end: Point) -> Option<(f32, Point)> {
    let entity = Point::new(end.x - start.x, end.y - start.y);
    let denominator = movement.x * entity.y - movement.y * entity.x;
    if denominator.abs() < 1e-9 {
        return None;
    }
    let offset = Point::new(start.x - position.x, start.y - position.y);
    let distance = (offset.x * entity.y - offset.y * entity.x) / denominator;
    let entity_distance = (offset.x * movement.y - offset.y * movement.x) / denominator;
    if distance <= 1e-7 || distance > 1.0 || !(0.0..=1.0).contains(&entity_distance) {
        return None;
    }
    let length = entity.x.hypot(entity.y);
    (length > 0.0).then(|| (distance, Point::new(-entity.y / length, entity.x / length)))
}

fn rectangle_contains(point: Point, start: Point, end: Point) -> bool {
    point.x > start.x.min(end.x)
        && point.x < start.x.max(end.x)
        && point.y > start.y.min(end.y)
        && point.y < start.y.max(end.y)
}

fn inside(point: Point) -> bool {
    point.x >= 0.0
        && point.y >= 0.0
        && point.x < f32::from(PIXEL_RAY_TRACER_SIZE)
        && point.y < f32::from(PIXEL_RAY_TRACER_SIZE)
}

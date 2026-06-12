use glam::Vec2;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::{Polygon, Polyline};

pub fn polygon_signed_area(polygon: &[Vec2]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(a, b)| a.perp_dot(*b))
        .sum::<f32>()
        * 0.5
}

pub fn polygon_area(polygon: &[Vec2]) -> f32 {
    polygon_signed_area(polygon).abs()
}

pub fn average_point(polygon: &[Vec2]) -> Vec2 {
    if polygon.is_empty() {
        Vec2::ZERO
    } else {
        polygon.iter().copied().sum::<Vec2>() / polygon.len() as f32
    }
}

pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

pub fn simplify_polyline(points: &[Vec2], tolerance: f32) -> Polyline {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let mut furthest = 0.0;
    let mut furthest_index = 0;
    for (index, point) in points[1..points.len() - 1].iter().enumerate() {
        let distance = distance_to_segment(*point, first, last);
        if distance > furthest {
            furthest = distance;
            furthest_index = index + 1;
        }
    }
    if furthest <= tolerance {
        return vec![first, last];
    }
    let mut left = simplify_polyline(&points[..=furthest_index], tolerance);
    let right = simplify_polyline(&points[furthest_index..], tolerance);
    left.pop();
    left.extend(right);
    left
}

pub fn inset_polygon(polygon: &[Vec2], spacing: f32) -> Option<Polygon> {
    if polygon.len() < 3 {
        return None;
    }
    let orientation = polygon_signed_area(polygon).signum();
    if orientation == 0.0 {
        return None;
    }
    let mut lines = Vec::with_capacity(polygon.len());
    for (&a, &b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        let edge = b - a;
        if edge.length_squared() < 1.0e-8 {
            continue;
        }
        let inward = Vec2::new(-edge.y, edge.x).normalize() * orientation * spacing;
        lines.push((a + inward, b + inward));
    }
    if lines.len() < 3 {
        return None;
    }
    let mut result = Vec::with_capacity(lines.len());
    for index in 0..lines.len() {
        let previous = lines[(index + lines.len() - 1) % lines.len()];
        let current = lines[index];
        result.push(line_intersection(
            previous.0, previous.1, current.0, current.1,
        )?);
    }
    (polygon_area(&result) > 1.0 && result.iter().all(|point| point.is_finite())).then_some(result)
}

pub fn polyline_band(line: &[Vec2], half_width: f32) -> Polygon {
    if line.len() < 2 {
        return Vec::new();
    }
    let normal_at = |index: usize| {
        let direction = if index == 0 {
            line[1] - line[0]
        } else if index == line.len() - 1 {
            line[index] - line[index - 1]
        } else {
            (line[index] - line[index - 1]).normalize()
                + (line[index + 1] - line[index]).normalize()
        };
        Vec2::new(-direction.y, direction.x).normalize_or_zero() * half_width
    };
    let mut left = Vec::with_capacity(line.len());
    let mut right = Vec::with_capacity(line.len());
    for (index, &point) in line.iter().enumerate() {
        let normal = normal_at(index);
        left.push(point + normal);
        right.push(point - normal);
    }
    right.reverse();
    left.extend(right);
    left
}

pub fn subdivide_polygon(polygon: Polygon, min_area: f32, rng: &mut ChaCha8Rng) -> Vec<Polygon> {
    subdivide_polygon_inner(polygon, min_area, rng, 0)
}

fn subdivide_polygon_inner(
    polygon: Polygon,
    min_area: f32,
    rng: &mut ChaCha8Rng,
    depth: usize,
) -> Vec<Polygon> {
    let area = polygon_area(&polygon);
    if area < min_area * 0.5 || polygon.len() < 3 {
        return Vec::new();
    }
    let perimeter = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(a, b)| a.distance(*b))
        .sum::<f32>();
    if perimeter == 0.0 || area / (perimeter * perimeter) < 0.04 {
        return Vec::new();
    }
    if area < min_area * 2.0 || depth >= 32 {
        return vec![polygon];
    }

    let (&a, &b) = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .max_by(|(a1, b1), (a2, b2)| {
            a1.distance_squared(**b1)
                .total_cmp(&a2.distance_squared(**b2))
        })
        .expect("polygon has at least three points");
    let split_point = a.lerp(b, rng.random_range(0.4..0.6));
    let split_direction = Vec2::new(-(b - a).y, (b - a).x).normalize();
    let first = clip_half_plane(&polygon, split_point, split_direction, true);
    let second = clip_half_plane(&polygon, split_point, split_direction, false);
    if first.len() < 3 || second.len() < 3 {
        return vec![polygon];
    }
    let first_area = polygon_area(&first);
    let second_area = polygon_area(&second);
    if first_area >= area * 0.999 || second_area >= area * 0.999 {
        return vec![polygon];
    }
    let mut result = subdivide_polygon_inner(first, min_area, rng, depth + 1);
    result.extend(subdivide_polygon_inner(second, min_area, rng, depth + 1));
    result
}

fn clip_half_plane(polygon: &[Vec2], point: Vec2, normal: Vec2, positive: bool) -> Polygon {
    let inside = |candidate: Vec2| {
        let value = (candidate - point).dot(normal);
        if positive {
            value >= -1.0e-5
        } else {
            value <= 1.0e-5
        }
    };
    let mut output = Vec::new();
    let mut previous = polygon[polygon.len() - 1];
    let mut previous_inside = inside(previous);
    for &current in polygon {
        let current_inside = inside(current);
        if current_inside != previous_inside {
            let delta = current - previous;
            let denominator = delta.dot(normal);
            if denominator.abs() > 1.0e-7 {
                let t = (point - previous).dot(normal) / denominator;
                output.push(previous + delta * t);
            }
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    deduplicate_polygon(output)
}

fn deduplicate_polygon(mut polygon: Polygon) -> Polygon {
    polygon.dedup_by(|a, b| a.distance_squared(*b) < 1.0e-8);
    if polygon.len() > 1 && polygon[0].distance_squared(*polygon.last().unwrap()) < 1.0e-8 {
        polygon.pop();
    }
    polygon
}

pub fn segment_intersection(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denominator = r.perp_dot(s);
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    let t = (c - a).perp_dot(s) / denominator;
    let u = (c - a).perp_dot(r) / denominator;
    if (-1.0e-5..=1.0 + 1.0e-5).contains(&t) && (-1.0e-5..=1.0 + 1.0e-5).contains(&u) {
        Some(a + r * t)
    } else {
        None
    }
}

fn line_intersection(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denominator = r.perp_dot(s);
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    Some(a + r * ((c - a).perp_dot(s) / denominator))
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    if segment.length_squared() == 0.0 {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

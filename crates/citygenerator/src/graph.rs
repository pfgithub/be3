use std::collections::{HashMap, HashSet, VecDeque};

use glam::Vec2;

use crate::geometry::{polygon_area, polygon_signed_area, segment_intersection};
use crate::{Polygon, Polyline};

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub position: Vec2,
    pub neighbors: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub intersections: Vec<Vec2>,
}

impl Graph {
    pub fn from_streamlines(streamlines: &[Polyline], _dstep: f32, delete_dangling: bool) -> Self {
        let segments: Vec<_> = streamlines
            .iter()
            .flat_map(|line| line.windows(2))
            .filter(|edge| edge[0].distance_squared(edge[1]) > 1.0e-8)
            .map(|edge| Segment {
                start: edge[0],
                end: edge[1],
                splits: vec![edge[0], edge[1]],
            })
            .collect();
        let mut segments = segments;
        let mut intersections = Vec::new();
        for first in 0..segments.len() {
            for second in first + 1..segments.len() {
                let Some(point) = segment_intersection(
                    segments[first].start,
                    segments[first].end,
                    segments[second].start,
                    segments[second].end,
                ) else {
                    continue;
                };
                segments[first].splits.push(point);
                segments[second].splits.push(point);
                if !is_endpoint(point, &segments[first]) && !is_endpoint(point, &segments[second]) {
                    intersections.push(point);
                }
            }
        }

        let mut builder = GraphBuilder::default();
        for segment in &mut segments {
            let direction = segment.end - segment.start;
            segment.splits.sort_by(|a, b| {
                (*a - segment.start)
                    .dot(direction)
                    .total_cmp(&(*b - segment.start).dot(direction))
            });
            segment
                .splits
                .dedup_by(|a, b| a.distance_squared(*b) < 1.0e-6);
            for edge in segment.splits.windows(2) {
                let a = builder.node(edge[0]);
                let b = builder.node(edge[1]);
                builder.connect(a, b);
            }
        }
        let mut graph = Self {
            nodes: builder.nodes,
            intersections,
        };
        if delete_dangling {
            graph.delete_dangling();
        }
        graph
    }

    pub fn polygons(&self, max_length: usize) -> Vec<Polygon> {
        let mut visited = HashSet::new();
        let mut polygons = Vec::new();
        for from in 0..self.nodes.len() {
            for &to in &self.nodes[from].neighbors {
                if visited.contains(&(from, to)) {
                    continue;
                }
                let mut polygon = Vec::new();
                let (start_from, start_to) = (from, to);
                let (mut previous, mut current) = (from, to);
                for _ in 0..max_length {
                    if !visited.insert((previous, current)) {
                        break;
                    }
                    polygon.push(self.nodes[previous].position);
                    let Some(next) = self.rightmost_neighbor(previous, current) else {
                        polygon.clear();
                        break;
                    };
                    previous = current;
                    current = next;
                    if previous == start_from && current == start_to {
                        break;
                    }
                }
                if polygon.len() >= 3
                    && current == start_to
                    && previous == start_from
                    && polygon_area(&polygon) > 1.0e-4
                {
                    polygons.push(polygon);
                }
            }
        }

        let positive = polygons
            .iter()
            .filter(|polygon| polygon_signed_area(polygon) > 0.0)
            .count();
        let keep_positive = positive >= polygons.len().saturating_sub(positive);
        polygons.retain(|polygon| (polygon_signed_area(polygon) > 0.0) == keep_positive);
        polygons
    }

    fn rightmost_neighbor(&self, from: usize, at: usize) -> Option<usize> {
        let backwards = self.nodes[from].position - self.nodes[at].position;
        let base = backwards.to_angle();
        self.nodes[at]
            .neighbors
            .iter()
            .copied()
            .filter(|candidate| *candidate != from)
            .min_by(|a, b| {
                let angle_a = (self.nodes[*a].position - self.nodes[at].position).to_angle() - base;
                let angle_b = (self.nodes[*b].position - self.nodes[at].position).to_angle() - base;
                angle_a
                    .rem_euclid(std::f32::consts::TAU)
                    .total_cmp(&angle_b.rem_euclid(std::f32::consts::TAU))
            })
    }

    fn delete_dangling(&mut self) {
        let mut removed = vec![false; self.nodes.len()];
        let mut degree: Vec<_> = self.nodes.iter().map(|node| node.neighbors.len()).collect();
        let mut queue: VecDeque<_> = degree
            .iter()
            .enumerate()
            .filter_map(|(index, degree)| (*degree <= 1).then_some(index))
            .collect();
        while let Some(index) = queue.pop_front() {
            if removed[index] {
                continue;
            }
            removed[index] = true;
            for &neighbor in &self.nodes[index].neighbors {
                if !removed[neighbor] {
                    degree[neighbor] = degree[neighbor].saturating_sub(1);
                    if degree[neighbor] == 1 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        let mut remap = vec![usize::MAX; self.nodes.len()];
        let mut nodes = Vec::new();
        for (old, node) in self.nodes.iter().enumerate() {
            if !removed[old] {
                remap[old] = nodes.len();
                nodes.push(Node {
                    position: node.position,
                    neighbors: Vec::new(),
                });
            }
        }
        for (old, node) in self.nodes.iter().enumerate() {
            if removed[old] {
                continue;
            }
            nodes[remap[old]].neighbors = node
                .neighbors
                .iter()
                .filter(|neighbor| !removed[**neighbor])
                .map(|neighbor| remap[*neighbor])
                .collect();
        }
        self.nodes = nodes;
    }
}

#[derive(Clone, Debug)]
struct Segment {
    start: Vec2,
    end: Vec2,
    splits: Vec<Vec2>,
}

fn is_endpoint(point: Vec2, segment: &Segment) -> bool {
    point.distance_squared(segment.start) < 1.0e-6 || point.distance_squared(segment.end) < 1.0e-6
}

#[derive(Default)]
struct GraphBuilder {
    nodes: Vec<Node>,
    buckets: HashMap<(i64, i64), usize>,
}

impl GraphBuilder {
    fn node(&mut self, position: Vec2) -> usize {
        let key = (
            (position.x * 1_000.0).round() as i64,
            (position.y * 1_000.0).round() as i64,
        );
        if let Some(&index) = self.buckets.get(&key) {
            return index;
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            position,
            neighbors: Vec::new(),
        });
        self.buckets.insert(key, index);
        index
    }

    fn connect(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        if !self.nodes[a].neighbors.contains(&b) {
            self.nodes[a].neighbors.push(b);
        }
        if !self.nodes[b].neighbors.contains(&a) {
            self.nodes[b].neighbors.push(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_block_in_a_square_graph() {
        let square = vec![
            vec![Vec2::ZERO, Vec2::X],
            vec![Vec2::X, Vec2::ONE],
            vec![Vec2::ONE, Vec2::Y],
            vec![Vec2::Y, Vec2::ZERO],
        ];
        let graph = Graph::from_streamlines(&square, 1.0, true);
        let polygons = graph.polygons(20);
        assert_eq!(polygons.len(), 1);
        assert!((polygon_area(&polygons[0]) - 1.0).abs() < 1.0e-5);
    }
}

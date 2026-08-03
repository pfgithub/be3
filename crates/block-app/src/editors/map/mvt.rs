//! Minimal decoder for the Mapbox Vector Tile format, covering the subset
//! needed to render OpenStreetMap Shortbread tiles.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GeometryKind {
    Point,
    Line,
    Polygon,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TagValue {
    String(String),
    Float(f64),
    Int(i64),
    Bool(bool),
}

impl TagValue {
    pub(super) fn as_str(&self) -> Option<&str> {
        match self {
            TagValue::String(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn as_i64(&self) -> Option<i64> {
        match self {
            TagValue::Int(value) => Some(*value),
            TagValue::Float(value) => Some(*value as i64),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Feature {
    pub kind: GeometryKind,
    /// Decoded coordinate paths in tile-local units: rings for polygons,
    /// polylines for lines, and single-point paths for points.
    pub paths: Vec<Vec<[f32; 2]>>,
    /// Key and value indices into the owning layer's tables.
    pub tags: Vec<(u32, u32)>,
}

#[derive(Clone, Debug)]
pub(super) struct Layer {
    pub name: String,
    pub extent: u32,
    pub keys: Vec<String>,
    pub values: Vec<TagValue>,
    pub features: Vec<Feature>,
}

impl Layer {
    pub(super) fn tag<'a>(&'a self, feature: &Feature, key: &str) -> Option<&'a TagValue> {
        let key_index = self.keys.iter().position(|candidate| candidate == key)? as u32;
        feature
            .tags
            .iter()
            .find(|(index, _)| *index == key_index)
            .and_then(|(_, value)| self.values.get(*value as usize))
    }
}

#[derive(Clone, Debug)]
pub(super) struct Tile {
    pub layers: Vec<Layer>,
}

impl Tile {
    pub(super) fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.name == name)
    }
}

pub(super) fn decode(data: &[u8]) -> Result<Tile, String> {
    let mut reader = Reader::new(data);
    let mut layers = Vec::new();
    while let Some((field, wire)) = reader.field()? {
        if (field, wire) == (3, 2) {
            layers.push(decode_layer(reader.bytes()?)?);
        } else {
            reader.skip(wire)?;
        }
    }
    Ok(Tile { layers })
}

fn decode_layer(data: &[u8]) -> Result<Layer, String> {
    let mut reader = Reader::new(data);
    let mut layer = Layer {
        name: String::new(),
        extent: 4096,
        keys: Vec::new(),
        values: Vec::new(),
        features: Vec::new(),
    };
    while let Some((field, wire)) = reader.field()? {
        match (field, wire) {
            (1, 2) => layer.name = String::from_utf8_lossy(reader.bytes()?).into_owned(),
            (2, 2) => {
                if let Some(feature) = decode_feature(reader.bytes()?)? {
                    layer.features.push(feature);
                }
            }
            (3, 2) => layer
                .keys
                .push(String::from_utf8_lossy(reader.bytes()?).into_owned()),
            (4, 2) => layer.values.push(decode_value(reader.bytes()?)?),
            (5, 0) => layer.extent = (reader.varint()? as u32).max(1),
            _ => reader.skip(wire)?,
        }
    }
    Ok(layer)
}

fn decode_feature(data: &[u8]) -> Result<Option<Feature>, String> {
    let mut reader = Reader::new(data);
    let mut kind = None;
    let mut tags = Vec::new();
    let mut geometry = Vec::new();
    while let Some((field, wire)) = reader.field()? {
        match (field, wire) {
            (2, 2) => {
                let mut packed = Reader::new(reader.bytes()?);
                while !packed.done() {
                    tags.push(packed.varint()? as u32);
                }
            }
            (3, 0) => {
                kind = match reader.varint()? {
                    1 => Some(GeometryKind::Point),
                    2 => Some(GeometryKind::Line),
                    3 => Some(GeometryKind::Polygon),
                    _ => None,
                }
            }
            (4, 2) => {
                let mut packed = Reader::new(reader.bytes()?);
                while !packed.done() {
                    geometry.push(packed.varint()? as u32);
                }
            }
            _ => reader.skip(wire)?,
        }
    }
    let Some(kind) = kind else {
        return Ok(None);
    };
    Ok(Some(Feature {
        kind,
        paths: decode_geometry(&geometry, kind),
        tags: tags
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect(),
    }))
}

fn decode_value(data: &[u8]) -> Result<TagValue, String> {
    let mut reader = Reader::new(data);
    let mut value = TagValue::String(String::new());
    while let Some((field, wire)) = reader.field()? {
        value = match (field, wire) {
            (1, 2) => TagValue::String(String::from_utf8_lossy(reader.bytes()?).into_owned()),
            (2, 5) => TagValue::Float(f32::from_le_bytes(reader.fixed::<4>()?) as f64),
            (3, 1) => TagValue::Float(f64::from_le_bytes(reader.fixed::<8>()?)),
            (4, 0) => TagValue::Int(reader.varint()? as i64),
            (5, 0) => TagValue::Int(reader.varint()? as i64),
            (6, 0) => TagValue::Int(zigzag64(reader.varint()?)),
            (7, 0) => TagValue::Bool(reader.varint()? != 0),
            _ => {
                reader.skip(wire)?;
                value
            }
        };
    }
    Ok(value)
}

fn decode_geometry(geometry: &[u32], kind: GeometryKind) -> Vec<Vec<[f32; 2]>> {
    let mut paths = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    let mut x = 0i64;
    let mut y = 0i64;
    let mut index = 0;
    while index < geometry.len() {
        let command = geometry[index];
        index += 1;
        let operation = command & 0x7;
        let count = (command >> 3) as usize;
        match operation {
            1 => {
                // MoveTo starts a new path per point.
                for _ in 0..count {
                    if index + 1 >= geometry.len() {
                        return finish_geometry(paths, current);
                    }
                    x += zigzag32(geometry[index]);
                    y += zigzag32(geometry[index + 1]);
                    index += 2;
                    if !current.is_empty() {
                        paths.push(std::mem::take(&mut current));
                    }
                    current.push([x as f32, y as f32]);
                    if kind == GeometryKind::Point {
                        paths.push(std::mem::take(&mut current));
                    }
                }
            }
            2 => {
                for _ in 0..count {
                    if index + 1 >= geometry.len() {
                        return finish_geometry(paths, current);
                    }
                    x += zigzag32(geometry[index]);
                    y += zigzag32(geometry[index + 1]);
                    index += 2;
                    current.push([x as f32, y as f32]);
                }
            }
            // ClosePath: rings are closed implicitly when filled.
            7 => {}
            _ => return finish_geometry(paths, current),
        }
    }
    finish_geometry(paths, current)
}

fn finish_geometry(mut paths: Vec<Vec<[f32; 2]>>, current: Vec<[f32; 2]>) -> Vec<Vec<[f32; 2]>> {
    if !current.is_empty() {
        paths.push(current);
    }
    paths
}

fn zigzag32(value: u32) -> i64 {
    (((value >> 1) as i32) ^ -((value & 1) as i32)) as i64
}

fn zigzag64(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn done(&self) -> bool {
        self.position >= self.data.len()
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let Some(&byte) = self.data.get(self.position) else {
                return Err("truncated varint".into());
            };
            self.position += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("varint too long".into())
    }

    fn field(&mut self) -> Result<Option<(u32, u32)>, String> {
        if self.done() {
            return Ok(None);
        }
        let header = self.varint()?;
        Ok(Some(((header >> 3) as u32, (header & 0x7) as u32)))
    }

    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let length = self.varint()? as usize;
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.data.len())
            .ok_or("truncated length-delimited field")?;
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .position
            .checked_add(N)
            .filter(|end| *end <= self.data.len())
            .ok_or("truncated fixed-width field")?;
        let mut bytes = [0u8; N];
        bytes.copy_from_slice(&self.data[self.position..end]);
        self.position = end;
        Ok(bytes)
    }

    fn skip(&mut self, wire: u32) -> Result<(), String> {
        match wire {
            0 => self.varint().map(|_| ()),
            1 => self.fixed::<8>().map(|_| ()),
            2 => self.bytes().map(|_| ()),
            5 => self.fixed::<4>().map(|_| ()),
            _ => Err(format!("unsupported wire type {wire}")),
        }
    }
}

#[cfg(test)]
mod tests;

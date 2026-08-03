use super::*;

mod decode_parses_layers_features_and_tags;
mod decode_splits_polygons_and_multipoints_into_paths;

fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            bytes.push(byte);
            return bytes;
        }
        bytes.push(byte | 0x80);
    }
}

fn field(number: u32, wire: u32) -> Vec<u8> {
    varint(u64::from(number << 3 | wire))
}

fn length_delimited(number: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = field(number, 2);
    bytes.extend(varint(payload.len() as u64));
    bytes.extend(payload);
    bytes
}

fn packed(number: u32, values: &[u32]) -> Vec<u8> {
    let payload: Vec<u8> = values
        .iter()
        .flat_map(|value| varint(u64::from(*value)))
        .collect();
    length_delimited(number, &payload)
}

fn zigzag(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

/// Encodes a feature message wrapped as a layer `features` field.
fn feature(kind: u32, tags: &[u32], geometry: &[u32]) -> Vec<u8> {
    let mut bytes = packed(2, tags);
    bytes.extend(field(3, 0));
    bytes.extend(varint(u64::from(kind)));
    bytes.extend(packed(4, geometry));
    length_delimited(2, &bytes)
}

fn tile_with_layer(layer: &[u8]) -> Vec<u8> {
    length_delimited(3, layer)
}

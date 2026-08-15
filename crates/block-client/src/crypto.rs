use std::collections::BTreeMap;
use std::io::{Read, Write};

use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use uuid::Uuid;

use crate::fatal;

const STATIC_KEY: [u8; 32] = [
    0xb8, 0xbe, 0x0b, 0x14, 0x30, 0xd8, 0x89, 0x83, 0xe5, 0xa5, 0xed, 0x11, 0x37, 0x81, 0x86, 0x33,
    0xc7, 0x73, 0x9c, 0x29, 0xa3, 0xae, 0x91, 0xa6, 0xf4, 0x14, 0x40, 0xdb, 0x45, 0xd5, 0x45, 0xcd,
];

fn cipher() -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(&STATIC_KEY))
}

fn compress(plain: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(plain)
        .unwrap_or_else(|error| fatal(format!("failed to compress block data: {error}")));
    encoder
        .finish()
        .unwrap_or_else(|error| fatal(format!("failed to compress block data: {error}")))
}

fn decompress(compressed: &[u8]) -> Vec<u8> {
    let mut plain = Vec::new();
    ZlibDecoder::new(compressed)
        .read_to_end(&mut plain)
        .unwrap_or_else(|error| fatal(format!("failed to decompress block data: {error}")));
    plain
}

pub(crate) fn encode(plain: &[u8]) -> Vec<u8> {
    let compressed = compress(plain);
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher()
        .encrypt(&nonce, compressed.as_slice())
        .unwrap_or_else(|error| fatal(format!("failed to encrypt block data: {error}")));
    let mut encoded = Vec::with_capacity(nonce.len() + ciphertext.len());
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    encoded
}

pub(crate) fn decode(encoded: &[u8]) -> Vec<u8> {
    let Some((nonce_bytes, ciphertext)) = encoded.split_at_checked(size_of::<Nonce>()) else {
        fatal("encrypted block data is shorter than a nonce");
    };
    let compressed = cipher()
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .unwrap_or_else(|error| fatal(format!("failed to decrypt block data: {error}")));
    decompress(&compressed)
}

pub(crate) fn encode_properties(properties: &BTreeMap<Uuid, Vec<u8>>) -> BTreeMap<Uuid, Vec<u8>> {
    properties
        .iter()
        .map(|(key, value)| (*key, encode(value)))
        .collect()
}

pub(crate) fn decode_properties(properties: BTreeMap<Uuid, Vec<u8>>) -> BTreeMap<Uuid, Vec<u8>> {
    properties
        .into_iter()
        .map(|(key, value)| (key, decode(&value)))
        .collect()
}

#[cfg(test)]
mod tests;

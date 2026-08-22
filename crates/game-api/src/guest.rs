use std::alloc::{alloc, dealloc, Layout};
use std::convert::Infallible;

use crate::{GameHelper, GameRequest, GameScreen};

/// The buffer the host writes a request into. The host is trusted with
/// exactly one thing here - handing the same pointer and length straight
/// back to `show`, which frees it.
pub fn allocate(length: u32) -> u32 {
    let Ok(layout) = layout(length) else {
        return 0;
    };
    unsafe { alloc(layout) as u32 }
}

pub fn text(value: &str) -> u64 {
    hand_back(value.as_bytes().to_vec())
}

pub fn show(
    pointer: u32,
    length: u32,
    play: fn(GameHelper<'_>) -> Result<Infallible, GameScreen>,
) -> u64 {
    let request = take(pointer, length);
    let request: GameRequest =
        bincode::deserialize(&request).expect("the host encodes the request it asks about");
    let screen = match play(GameHelper::new(&request.actions, request.player)) {
        Ok(never) => match never {},
        Err(screen) => screen,
    };
    hand_back(bincode::serialize(&screen).expect("a screen is always encodable"))
}

fn layout(length: u32) -> Result<Layout, std::alloc::LayoutError> {
    Layout::from_size_align(length.max(1) as usize, 1)
}

fn take(pointer: u32, length: u32) -> Vec<u8> {
    let layout = layout(length).expect("the host asked for this allocation itself");
    let bytes =
        unsafe { std::slice::from_raw_parts(pointer as *const u8, length as usize) }.to_vec();
    unsafe { dealloc(pointer as *mut u8, layout) };
    bytes
}

/// Leaks a buffer for the host to read, packed into the pointer and length
/// a single `i64` result can carry. Nothing frees it: the host throws the
/// whole instance away once it has read the answer.
fn hand_back(bytes: Vec<u8>) -> u64 {
    let length = bytes.len() as u64;
    let pointer = Box::into_raw(bytes.into_boxed_slice()) as *mut u8 as u64;
    (pointer << 32) | length
}

/// Declares the module's exports: its display name, and the two functions
/// the host calls to ask what one player currently sees.
#[macro_export]
macro_rules! game {
    ($name:expr, $play:path) => {
        #[no_mangle]
        pub extern "C" fn name() -> u64 {
            $crate::guest::text($name)
        }

        #[no_mangle]
        pub extern "C" fn allocate(length: u32) -> u32 {
            $crate::guest::allocate(length)
        }

        #[no_mangle]
        pub extern "C" fn show(pointer: u32, length: u32) -> u64 {
            $crate::guest::show(pointer, length, $play)
        }
    };
}

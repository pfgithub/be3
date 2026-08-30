use std::{collections::VecDeque, sync::Arc, time::Instant};

use block_gpu_host::Gpu;
use wasmtime::SharedMemory;
use wasmtime_wasi::p1::WasiP1Ctx;

use crate::threads::{Spawner, Spawns};

pub struct State {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) memory: SharedMemory,
    pub(crate) gpu: Gpu,
    pub(crate) inbox: VecDeque<Vec<u8>>,
    pub(crate) outbox: Vec<Vec<u8>>,
    pub(crate) started: Instant,
    pub(crate) threads: Arc<Spawner>,
}

pub(crate) struct Threaded {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) threads: Arc<Spawner>,
}

impl Spawns for State {
    fn spawner(&self) -> &Arc<Spawner> {
        &self.threads
    }
}

impl Spawns for Threaded {
    fn spawner(&self) -> &Arc<Spawner> {
        &self.threads
    }
}

impl State {
    pub(crate) fn read(&self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let data = self.memory.data();
        let start = pointer as usize;
        let end = start
            .checked_add(length as usize)
            .ok_or_else(|| "a plugin pointer overflowed".to_owned())?;
        if end > data.len() {
            return Err(format!(
                "a plugin pointer ran past its memory: {start}..{end} of {}",
                data.len()
            ));
        }
        let mut bytes = vec![0u8; length as usize];
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = unsafe { *data[start + offset].get() };
        }
        Ok(bytes)
    }

    pub(crate) fn read_words(&self, pointer: u32, count: u32) -> Result<Vec<u32>, String> {
        let bytes = self.read(pointer, count.saturating_mul(4))?;
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(u32::from_le_bytes)
            .collect())
    }

    pub(crate) fn write(&self, pointer: u32, capacity: u32, bytes: &[u8]) -> u32 {
        let needed = bytes.len() as u32;
        if needed > capacity {
            return needed;
        }
        let data = self.memory.data();
        let start = pointer as usize;
        if start.saturating_add(bytes.len()) > data.len() {
            return 0;
        }
        for (offset, byte) in bytes.iter().enumerate() {
            unsafe { *data[start + offset].get() = *byte };
        }
        needed
    }

    pub(crate) fn report(&mut self, message: String) {
        self.gpu.report(message);
    }
}

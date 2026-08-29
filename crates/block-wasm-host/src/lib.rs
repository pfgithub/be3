mod gpu;
mod state;
mod transport;

#[cfg(test)]
mod tests;

use std::{path::Path, time::Instant};

use block_gpu_host::Gpu;
use wasmtime::{Config, Engine, Instance, Linker, Module, SharedMemory, Store, TypedFunc};
use wasmtime_wasi::{p1, WasiCtxBuilder};

pub use state::State;

pub struct Plugin {
    store: Store<State>,
    start: TypedFunc<(), ()>,
    step: TypedFunc<(), ()>,
    shutdown: TypedFunc<(), ()>,
    stopped: bool,
}

impl Plugin {
    pub fn from_file(
        path: &Path,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
        Self::from_bytes(&bytes, device, queue)
    }

    pub fn from_bytes(
        bytes: &[u8],
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, String> {
        let engine = engine()?;
        let module = Module::new(&engine, bytes)
            .map_err(|error| format!("the plugin module could not be compiled: {error}"))?;
        let memory = shared_memory(&engine, &module)?;
        let wasi = WasiCtxBuilder::new().inherit_stderr().build_p1();
        let state = State {
            wasi,
            memory: memory.clone(),
            gpu: Gpu::new(device, queue),
            inbox: Default::default(),
            outbox: Vec::new(),
            started: Instant::now(),
        };
        let mut store = Store::new(&engine, state);
        let mut linker: Linker<State> = Linker::new(&engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked: {error}"))?;
        linker
            .define(&store, "env", "memory", memory)
            .map_err(|error| format!("the plugin memory could not be linked: {error}"))?;
        gpu::link(&mut linker)?;
        transport::link(&mut linker)?;
        threads::link(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(&module)
            .map_err(|error| format!("the plugin imports could not be stubbed: {error}"))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| format!("the plugin could not be instantiated: {error}"))?;
        initialize_storage(&instance, &mut store)?;
        let plugin = Self {
            start: typed(&instance, &mut store, "plugin_start")?,
            step: typed(&instance, &mut store, "plugin_step")?,
            shutdown: typed(&instance, &mut store, "plugin_shutdown")?,
            store,
            stopped: false,
        };
        Ok(plugin)
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.call(self.start.clone())
    }

    pub fn step(&mut self) -> Result<(), String> {
        self.call(self.step.clone())
    }

    fn call(&mut self, function: TypedFunc<(), ()>) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        let outcome = function
            .call(&mut self.store, ())
            .map_err(|error| format!("the plugin trapped: {error}"));
        if let Err(error) = outcome {
            self.stopped = true;
            return Err(error);
        }
        match self.store.data_mut().gpu.take_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn send(&mut self, frame: Vec<u8>) {
        self.store.data_mut().inbox.push_back(frame);
    }

    pub fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.store.data_mut().outbox)
    }

    pub fn take_presented(&mut self) -> Vec<u32> {
        self.store.data_mut().gpu.take_presented()
    }

    pub fn surface(&self, surface: u32) -> Option<(wgpu::Texture, u64)> {
        self.store
            .data()
            .gpu
            .surface(surface)
            .map(|(texture, generation)| (texture.clone(), generation))
    }

    pub fn attach_surface(&mut self, surface: u32, texture: wgpu::Texture) {
        self.store.data_mut().gpu.attach_surface(surface, texture);
    }

    pub fn detach_surface(&mut self, surface: u32) {
        self.store.data_mut().gpu.detach_surface(surface);
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        let _ = self.shutdown.call(&mut self.store, ());
    }
}

fn initialize_storage(instance: &Instance, store: &mut Store<State>) -> Result<(), String> {
    let size = global(instance, store, "__tls_size")?;
    let align = global(instance, store, "__tls_align")?;
    let initialize: TypedFunc<(u32, u32), ()> = typed(instance, store, "plugin_initialize_tls")?;
    initialize
        .call(store, (size, align))
        .map_err(|error| format!("the plugin could not set up thread storage: {error}"))
}

fn global(instance: &Instance, store: &mut Store<State>, name: &str) -> Result<u32, String> {
    instance
        .get_global(&mut *store, name)
        .and_then(|global| global.get(store).i32())
        .map(|value| value as u32)
        .ok_or_else(|| format!("the plugin does not export {name}"))
}

fn engine() -> Result<Engine, String> {
    let mut config = Config::new();
    config.wasm_threads(true);
    config.shared_memory(true);
    config.wasm_bulk_memory(true);
    Engine::new(&config).map_err(|error| format!("the wasm engine could not start: {error}"))
}

fn shared_memory(engine: &Engine, module: &Module) -> Result<SharedMemory, String> {
    let declared = module
        .imports()
        .find(|import| import.module() == "env" && import.name() == "memory")
        .and_then(|import| import.ty().memory().cloned())
        .ok_or_else(|| "the plugin does not import env.memory".to_owned())?;
    if !declared.is_shared() {
        return Err("the plugin memory is not shared".to_owned());
    }
    SharedMemory::new(engine, declared)
        .map_err(|error| format!("the plugin memory could not be created: {error}"))
}

fn typed<Parameters, Results>(
    instance: &Instance,
    store: &mut Store<State>,
    name: &str,
) -> Result<TypedFunc<Parameters, Results>, String>
where
    Parameters: wasmtime::WasmParams,
    Results: wasmtime::WasmResults,
{
    instance
        .get_typed_func(store, name)
        .map_err(|error| format!("the plugin has no usable {name} export: {error}"))
}

mod threads {
    use wasmtime::Linker;

    use crate::State;

    pub(super) fn link(linker: &mut Linker<State>) -> Result<(), String> {
        linker
            .func_wrap("wasi", "thread-spawn", |_: i32| -> i32 { -1 })
            .map_err(|error| format!("thread spawn could not be linked: {error}"))?;
        Ok(())
    }
}

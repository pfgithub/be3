use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc, Mutex,
};

use block_gpu_abi as abi;
use wasmtime::{Caller, Engine, Error, Linker, Module, SharedMemory, Store, TypedFunc};
use wasmtime_wasi::{p1, WasiCtxBuilder};

use crate::state::Threaded;

const MODULE: &str = "wasi";
const SPAWN: &str = "thread-spawn";
const START: &str = "wasi_thread_start";
const REFUSED: i32 = -1;

pub(crate) trait Spawns {
    fn spawner(&self) -> &Arc<Spawner>;
}

pub(crate) struct Spawner {
    engine: Engine,
    module: Module,
    memory: SharedMemory,
    next: AtomicI32,
    stopped: AtomicBool,
    failures: Mutex<Vec<String>>,
}

impl Spawner {
    pub(crate) fn new(engine: Engine, module: Module, memory: SharedMemory) -> Arc<Self> {
        Arc::new(Self {
            engine,
            module,
            memory,
            next: AtomicI32::new(1),
            stopped: AtomicBool::new(false),
            failures: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn take_failure(&self) -> Option<String> {
        let mut failures = self.failures.lock().ok()?;
        match failures.is_empty() {
            true => None,
            false => Some(failures.remove(0)),
        }
    }

    fn spawn(self: &Arc<Self>, argument: i32) -> i32 {
        if self.stopped.load(Ordering::Acquire) {
            return REFUSED;
        }
        let thread = self.next.fetch_add(1, Ordering::Relaxed);
        let spawner = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name(format!("plugin thread {thread}"))
            .spawn(move || {
                if let Err(failure) = spawner.enter(thread, argument) {
                    spawner.fail(failure);
                }
            });
        match spawned {
            Ok(_) => thread,
            Err(error) => {
                self.fail(format!("a plugin thread could not start: {error}"));
                REFUSED
            }
        }
    }

    fn enter(self: &Arc<Self>, thread: i32, argument: i32) -> Result<(), String> {
        let wasi = WasiCtxBuilder::new().inherit_stderr().build_p1();
        let state = Threaded {
            wasi,
            threads: Arc::clone(self),
        };
        let mut store = Store::new(&self.engine, state);
        let mut linker: Linker<Threaded> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut Threaded| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked for a plugin thread: {error}"))?;
        linker
            .define(&store, "env", "memory", self.memory.clone())
            .map_err(|error| format!("a plugin thread could not share memory: {error}"))?;
        link(&mut linker)?;
        self.refuse_host_calls(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(&self.module)
            .map_err(|error| format!("a plugin thread could not stub its imports: {error}"))?;
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|error| format!("a plugin thread could not be instantiated: {error}"))?;
        let start: TypedFunc<(i32, i32), ()> = instance
            .get_typed_func(&mut store, START)
            .map_err(|error| format!("the plugin has no usable {START} export: {error}"))?;
        start
            .call(&mut store, (thread, argument))
            .map_err(|error| format!("a plugin thread trapped: {error:?}"))
    }

    fn refuse_host_calls(&self, linker: &mut Linker<Threaded>) -> Result<(), String> {
        for import in self.module.imports() {
            if !matches!(import.module(), abi::GPU_MODULE | abi::HOST_MODULE) {
                continue;
            }
            let Some(signature) = import.ty().func().cloned() else {
                continue;
            };
            let refusal = format!(
                "a plugin thread called {}::{}, which only its main thread may do",
                import.module(),
                import.name()
            );
            linker
                .func_new(import.module(), import.name(), signature, move |_, _, _| {
                    Err(Error::msg(refusal.clone()))
                })
                .map_err(|error| format!("a host call could not be refused: {error}"))?;
        }
        Ok(())
    }

    fn fail(&self, failure: String) {
        eprintln!("{failure}");
        if let Ok(mut failures) = self.failures.lock() {
            failures.push(failure);
        }
    }
}

pub(crate) fn link<T: Spawns + 'static>(linker: &mut Linker<T>) -> Result<(), String> {
    linker
        .func_wrap(
            MODULE,
            SPAWN,
            |caller: Caller<'_, T>, argument: i32| -> i32 {
                caller.data().spawner().spawn(argument)
            },
        )
        .map(|_| ())
        .map_err(|error| format!("{SPAWN} could not be linked: {error}"))
}

#[cfg(test)]
mod tests;

use wasmi::{Caller, Engine, Instance, Linker, Module, Store, TypedFunc};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Vertex {
    pub(super) position: [f32; 2],
    pub(super) color: [f32; 3],
}

#[derive(Debug, Default, Clone)]
pub(super) struct DrawFrame {
    pub(super) clear_color: [f32; 3],
    pub(super) vertices: Vec<Vertex>,
}

#[derive(Default)]
struct HostState {
    frame: DrawFrame,
}

/// A loaded `wasm-demo` module, run one frame at a time through its `frame`
/// export. The module has no access to anything but the drawing calls it is
/// given through `env`, so it cannot see or affect the rest of the app.
pub(super) struct GuestModule {
    store: Store<HostState>,
    frame_fn: TypedFunc<f32, ()>,
    _instance: Instance,
}

impl GuestModule {
    pub(super) fn load(bytes: &[u8]) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).map_err(|error| error.to_string())?;
        let mut store = Store::new(&engine, HostState::default());

        let mut linker = Linker::new(&engine);
        linker
            .func_wrap(
                "env",
                "clear",
                |mut caller: Caller<'_, HostState>, r: f32, g: f32, b: f32| {
                    caller.data_mut().frame.clear_color = [r, g, b];
                },
            )
            .expect("\"clear\" is only defined once");
        linker
            .func_wrap(
                "env",
                "draw_triangle",
                |mut caller: Caller<'_, HostState>,
                 x0: f32,
                 y0: f32,
                 x1: f32,
                 y1: f32,
                 x2: f32,
                 y2: f32,
                 r: f32,
                 g: f32,
                 b: f32| {
                    let color = [r, g, b];
                    let vertices = &mut caller.data_mut().frame.vertices;
                    for position in [[x0, y0], [x1, y1], [x2, y2]] {
                        vertices.push(Vertex { position, color });
                    }
                },
            )
            .expect("\"draw_triangle\" is only defined once");

        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|error| error.to_string())?;
        let frame_fn = instance
            .get_typed_func::<f32, ()>(&store, "frame")
            .map_err(|error| error.to_string())?;

        Ok(Self {
            store,
            frame_fn,
            _instance: instance,
        })
    }

    /// Runs the module's `frame` export and returns what it drew.
    pub(super) fn run_frame(&mut self, time_seconds: f32) -> Result<DrawFrame, String> {
        self.store.data_mut().frame = DrawFrame::default();
        self.frame_fn
            .call(&mut self.store, time_seconds)
            .map_err(|error| error.to_string())?;
        Ok(std::mem::take(&mut self.store.data_mut().frame))
    }
}

use block_gpu_abi as abi;
use wasmtime::{Caller, Linker};

use crate::State;

pub(super) fn link(linker: &mut Linker<State>) -> Result<(), String> {
    wrap(
        linker,
        "host_send",
        |mut caller: Caller<'_, State>, pointer: u32, length: u32| {
            let state = caller.data_mut();
            match state.read(pointer, length) {
                Ok(bytes) => state.outbox.push(bytes),
                Err(message) => state.report(message),
            }
        },
    )?;
    wrap(
        linker,
        "host_receive",
        |mut caller: Caller<'_, State>, pointer: u32, capacity: u32| -> i64 {
            let state = caller.data_mut();
            let Some(frame) = state.inbox.front() else {
                return abi::NO_MESSAGE;
            };
            let needed = frame.len() as u32;
            if needed > capacity {
                return needed as i64;
            }
            let frame = state.inbox.pop_front().unwrap_or_default();
            state.write(pointer, capacity, &frame);
            needed as i64
        },
    )?;
    wrap(linker, "host_now", |caller: Caller<'_, State>| -> f64 {
        caller.data().started.elapsed().as_secs_f64()
    })?;
    Ok(())
}

fn wrap<Parameters, Results>(
    linker: &mut Linker<State>,
    name: &str,
    function: impl wasmtime::IntoFunc<State, Parameters, Results>,
) -> Result<(), String> {
    linker
        .func_wrap(abi::HOST_MODULE, name, function)
        .map(|_| ())
        .map_err(|error| format!("{name} could not be linked: {error}"))
}

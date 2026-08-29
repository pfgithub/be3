// Instantiating a plugin in the browser.
//
// A plugin is the same bindgen-free wasm that wasmtime runs on every other
// platform: its only imports are WASI and the be3 gpu abi. In a browser the
// abi is answered by block-gpu-shim, a second module that does have bindgen
// and does hold a real wgpu device on the worker's OffscreenCanvas. The two
// modules have separate memories, so a call that only passes numbers is bound
// straight to the shim's export, and a call that passes a pointer goes through
// a wrapper here that copies the bytes into a scratch block the shim owns.

import * as wasi from "./wasi.js";
import * as threads from "./threads.js";

// Where each pointer sits in the argument list, mirroring the declarations in
// block-gpu-guest/src/imports.rs. A pointer is always followed by its length,
// so only the pointer's position and which way the bytes travel is recorded. A
// pair says the length counts something wider than a byte.
const reads = {
    create_buffer: [0],
    create_texture: [0],
    create_texture_view: [0],
    create_sampler: [0],
    create_bind_group_layout: [0],
    create_bind_group: [0],
    create_pipeline_layout: [0],
    create_shader_module: [0],
    create_render_pipeline: [0],
    create_command_encoder: [0],
    encoder_begin_render_pass: [0],
    queue_submit: [[0, 4]],
    queue_write_texture: [0, 2],
    queue_write_buffer: [2],
    buffer_write_mapped: [2],
    pass_set_bind_group: [[3, 4]],
    surface_configure: [1],
    host_send: [0],
};

const writes = {
    device_limits: 0,
    texture_describe: 1,
    error_take: 0,
    host_receive: 0,
};

function stub(module, name) {
    return () => {
        throw new Error(`a plugin called ${module}.${name}, which its host does not answer`);
    };
}

function forward(shim, memory, name) {
    const call = shim[name];
    if (typeof call !== "function") {
        return stub("be3", name);
    }
    const inbound = reads[name];
    if (inbound !== undefined) {
        const buffers = inbound.map((entry) =>
            Array.isArray(entry) ? entry : [entry, 1],
        );
        return (...args) => {
            let total = 0;
            for (const [at, width] of buffers) {
                total += args[at + 1] * width;
            }
            let address = shim.be3_scratch(total);
            const host = new Uint8Array(shim.memory.buffer);
            const guest = new Uint8Array(memory.buffer);
            for (const [at, width] of buffers) {
                const length = args[at + 1] * width;
                host.set(guest.subarray(args[at], args[at] + length), address);
                args[at] = address;
                address += length;
            }
            return call(...args);
        };
    }
    const at = writes[name];
    if (at !== undefined) {
        return (...args) => {
            const capacity = args[at + 1];
            const wanted = args[at];
            const address = shim.be3_scratch(capacity);
            args[at] = address;
            const answer = call(...args);
            const needed = Number(answer);
            if (needed > 0 && needed <= capacity) {
                const host = new Uint8Array(shim.memory.buffer);
                new Uint8Array(memory.buffer).set(
                    host.subarray(address, address + needed),
                    wanted,
                );
            }
            return answer;
        };
    }
    return call;
}

function resolve(module, memory, shim) {
    const resolved = { __proto__: null };
    for (const wanted of WebAssembly.Module.imports(module)) {
        const group = (resolved[wanted.module] ??= { __proto__: null });
        if (wanted.kind === "memory") {
            group[wanted.name] = memory;
        } else if (wanted.module === "wasi_snapshot_preview1") {
            group[wanted.name] = wasi[wanted.name];
        } else if (wanted.module === "wasi") {
            group[wanted.name] = threads[wanted.name];
        } else if (wanted.module === "be3_gpu" || wanted.module === "be3_host") {
            group[wanted.name] = forward(shim, memory, wanted.name);
        } else {
            group[wanted.name] = stub(wanted.module, wanted.name);
        }
    }
    return resolved;
}

// WebAssembly.Module.imports does not report a memory's limits, and a shared
// memory has to be created with limits the module accepts, so the import
// section is read out of the bytes the worker already fetched.
function memoryLimits(bytes) {
    const view = new DataView(bytes);
    let at = 8;
    const byte = () => view.getUint8(at++);
    const leb = () => {
        let value = 0;
        let shift = 0;
        for (;;) {
            const part = byte();
            value += (part & 0x7f) * 2 ** shift;
            if ((part & 0x80) === 0) {
                return value;
            }
            shift += 7;
        }
    };
    const name = () => {
        const length = leb();
        at += length;
    };
    while (at < view.byteLength) {
        const section = byte();
        const size = leb();
        const end = at + size;
        if (section !== 2) {
            at = end;
            continue;
        }
        const count = leb();
        for (let index = 0; index < count; index += 1) {
            name();
            name();
            const kind = byte();
            if (kind === 0) {
                leb();
            } else if (kind === 1) {
                byte();
                const flags = byte();
                leb();
                if (flags & 1) {
                    leb();
                }
            } else if (kind === 2) {
                const flags = byte();
                const initial = leb();
                const maximum = flags & 1 ? leb() : undefined;
                return { initial, maximum };
            } else if (kind === 3) {
                byte();
                byte();
            } else if (kind === 4) {
                byte();
                byte();
                leb();
            }
        }
        at = end;
    }
    throw new Error("a plugin does not import the memory it runs in");
}

export async function boot(shimUrl, moduleUrl, canvas) {
    const shimModule = await import(shimUrl);
    const shim = await shimModule.default();
    await shimModule.start(canvas);
    const bytes = await (await fetch(moduleUrl)).arrayBuffer();
    const limits = memoryLimits(bytes);
    const memory = new WebAssembly.Memory({ ...limits, shared: true });
    const module = await WebAssembly.compile(bytes);
    wasi.bindMemory(memory);
    threads.bindThreads(module, memory);
    const instance = new WebAssembly.Instance(module, resolve(module, memory, shim));
    const exports = instance.exports;
    exports.plugin_initialize_tls(exports.__tls_size.value, exports.__tls_align.value);
    exports.plugin_start();
    return {
        deliver: (frame) => shimModule.deliver(frame),
        collect: () => shimModule.collect(),
        failure: () => shimModule.failure(),
        step: () => exports.plugin_step(),
        shutdown: () => exports.plugin_shutdown(),
    };
}

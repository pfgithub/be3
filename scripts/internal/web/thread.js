// The Worker one wasm thread runs on.
//
// It is handed the compiled module and the shared memory of the instance that
// spawned it, instantiates the module again against that memory, and calls
// wasi_thread_start, which is where wasi-libc picks up the stack and the
// thread-local block the spawning thread already laid out for it. The module's
// start function is deliberately not run: memory, and everything in it, is
// already initialised by the thread that got there first.

import * as wasi from "./wasi.js";
import * as threads from "./threads.js";

function stub(module, name) {
    return () => {
        throw new Error(
            `a wasm thread called ${module}.${name}; a thread may only compute, ` +
                "because the JavaScript beside the module belongs to the thread that made it",
        );
    };
}

function imports(module, memory) {
    const resolved = { __proto__: null };
    for (const wanted of WebAssembly.Module.imports(module)) {
        const group = (resolved[wanted.module] ??= { __proto__: null });
        if (wanted.kind === "memory") {
            group[wanted.name] = memory;
        } else if (wanted.module === "wasi_snapshot_preview1") {
            group[wanted.name] = wasi[wanted.name];
        } else if (wanted.module === "wasi") {
            group[wanted.name] = threads[wanted.name];
        } else {
            group[wanted.name] = stub(wanted.module, wanted.name);
        }
    }
    return resolved;
}

self.onmessage = (event) => {
    const { module, memory, id, startArgument } = event.data;
    wasi.bindMemory(memory);
    threads.bindThreads(module, memory);
    const instance = new WebAssembly.Instance(module, imports(module, memory));
    try {
        instance.exports.wasi_thread_start(id, startArgument);
    } catch (error) {
        console.error(`thread ${id} stopped`, error);
    } finally {
        self.close();
    }
};

// The wasi thread-spawn call the module makes.
//
// block-app and every plugin are built for wasm32-wasip1-threads, so a
// std::thread::spawn reaches wasi-libc's pthread implementation, which asks the
// host for a thread through the wasi-threads import "wasi"."thread-spawn". A
// browser is not a wasi-threads host, so a thread is a Worker that instantiates
// the very same module against the very same shared memory and calls the
// module's wasi_thread_start with the id and argument agreed here.
//
// Only the module and its memory are shared. Every other import the thread's
// instance is given is a stub that throws, because the JavaScript state that
// wasm-bindgen keeps beside a module — its object table, its cached views —
// belongs to the thread that made it. A thread is for computing, and reaching
// out of the module from one is a mistake worth hearing about.

/** @type {{module: WebAssembly.Module, memory: WebAssembly.Memory} | null} */
let context = null;

// The id wasi-libc stores as the thread's own. Ids start above the main
// thread's so that no live thread is ever mistaken for another.
let nextId = 2;

/**
 * Hands the shim what a thread has to be started from: the compiled module and
 * the shared memory the running instance was given. Until this is called there
 * is nothing to start a thread from and spawning fails.
 */
export function bindThreads(module, memory) {
    context = { module, memory };
}

function threadSpawn(startArgument) {
    if (context === null) {
        console.error("a thread was spawned before bindThreads was called");
        return -1;
    }
    const id = nextId++;
    const worker = new Worker(new URL("./thread.js", import.meta.url), { type: "module" });
    worker.onerror = (event) => {
        console.error(`thread ${id} failed`, event.message ?? event);
    };
    worker.postMessage({
        module: context.module,
        memory: context.memory,
        id,
        startArgument,
    });
    return id;
}

export { threadSpawn as "thread-spawn" };

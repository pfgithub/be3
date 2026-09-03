// The one bare import libghostty-vt makes.
//
// The terminal emulator behind the debug terminal window is compiled for
// freestanding WebAssembly, where Zig's standard library has no stderr to log
// to and calls an imported "env"."log" instead. Nothing else in the module
// imports from "env", and the emulator only logs when it is fed input it
// cannot make sense of.
//
// The memory is shared, so it is a SharedArrayBuffer, which TextDecoder
// refuses to look at: the message is copied through an unshared buffer the
// same way the WASI shim does it.

/** @type {WebAssembly.Memory | null} */
let memory = null;

/**
 * Hands the shim the module's memory, the same way the WASI shim is given it.
 */
export function bindMemory(exported) {
    memory = exported;
}

const decoder = new TextDecoder();

export function log(pointer, length) {
    if (memory === null) {
        return;
    }
    console.log(decoder.decode(new Uint8Array(memory.buffer, pointer, length).slice()));
}

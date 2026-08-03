// The wasi_snapshot_preview1 calls the module makes.
//
// block-app is built for wasm32-wasip1, so Rust's standard library and the C in
// FreeType and HarfBuzz both reach the host through WASI. wasm-bindgen emits
// those as imports from the bare specifier "wasi_snapshot_preview1", which
// index.html points at this file with an import map.
//
// A browser is not a WASI host: there is no filesystem, no environment, and no
// process to exit. Only the calls the module actually makes are implemented,
// and the rest report that they are unsupported rather than pretending to
// succeed, so a wrong assumption surfaces as a clear error instead of silently
// corrupting state.

/** @type {WebAssembly.Memory | null} */
let memory = null;

/**
 * Hands the shim the module's memory. The wasm-bindgen glue runs the module's
 * start function during initialisation, before it returns the exports, so this
 * cannot be called any earlier than immediately after `init()` resolves.
 */
export function bindMemory(exported) {
    memory = exported;
}

// WASI errno values.
const SUCCESS = 0;
const EBADF = 8;
const EINVAL = 28;
const ENOENT = 44;
const ENOTSUP = 58;

// WASI filetype values.
const FILETYPE_CHARACTER_DEVICE = 2;

const STDOUT = 1;
const STDERR = 2;

function view() {
    if (memory === null) {
        throw new Error(
            "a WASI call was made before the module's memory was bound; " +
                "bindMemory must be called as soon as init() resolves",
        );
    }
    return new DataView(memory.buffer);
}

function bytes(pointer, length) {
    if (memory === null) {
        throw new Error("a WASI call was made before the module's memory was bound");
    }
    return new Uint8Array(memory.buffer, pointer, length);
}

// Text written to a stream is buffered until a newline, so a line logged in
// pieces arrives in the console as one message.
const pending = new Map();
const decoder = new TextDecoder();

function writeStream(fd, text) {
    const buffered = (pending.get(fd) ?? "") + text;
    const lines = buffered.split("\n");
    pending.set(fd, lines.pop() ?? "");
    for (const line of lines) {
        if (fd === STDERR) {
            console.error(line);
        } else {
            console.log(line);
        }
    }
}

export function fd_write(fd, iovsPointer, iovsLength, writtenPointer) {
    if (fd !== STDOUT && fd !== STDERR) {
        return EBADF;
    }
    const data = view();
    let written = 0;
    let text = "";
    for (let index = 0; index < iovsLength; index++) {
        const entry = iovsPointer + index * 8;
        const bufferPointer = data.getUint32(entry, true);
        const bufferLength = data.getUint32(entry + 4, true);
        text += decoder.decode(bytes(bufferPointer, bufferLength));
        written += bufferLength;
    }
    writeStream(fd, text);
    view().setUint32(writtenPointer, written, true);
    return SUCCESS;
}

export function fd_read(fd, iovsPointer, iovsLength, readPointer) {
    // Nothing can be read: report end of file rather than an error, which is
    // what a closed stdin looks like.
    view().setUint32(readPointer, 0, true);
    return SUCCESS;
}

export function fd_close() {
    return SUCCESS;
}

export function fd_seek() {
    // The standard streams are not seekable.
    return ENOTSUP;
}

export function fd_fdstat_get(fd, resultPointer) {
    if (fd !== STDOUT && fd !== STDERR && fd !== 0) {
        return EBADF;
    }
    // fdstat is { filetype: u8, flags: u16 at offset 2, rights: u64 at 8,
    // rights_inheriting: u64 at 16 }. Reporting a character device keeps the
    // standard library from treating stdout as a seekable file.
    const data = view();
    data.setUint8(resultPointer, FILETYPE_CHARACTER_DEVICE);
    data.setUint16(resultPointer + 2, 0, true);
    data.setBigUint64(resultPointer + 8, 0n, true);
    data.setBigUint64(resultPointer + 16, 0n, true);
    return SUCCESS;
}

export function fd_fdstat_set_flags() {
    return SUCCESS;
}

export function fd_filestat_get() {
    return EBADF;
}

export function fd_prestat_get() {
    // No preopened directories. Reporting EBADF ends the standard library's
    // scan for them, which is what leaves the module with no filesystem.
    return EBADF;
}

export function fd_prestat_dir_name() {
    return EBADF;
}

export function path_open() {
    // There is no filesystem, so every path is missing. The app only reaches
    // here through code paths that already tolerate a failed open, such as the
    // text editor's search for system fonts, which the web build answers with
    // fonts held in the binary instead.
    return ENOENT;
}

export function environ_sizes_get(countPointer, sizePointer) {
    const data = view();
    data.setUint32(countPointer, 0, true);
    data.setUint32(sizePointer, 0, true);
    return SUCCESS;
}

export function environ_get() {
    // Nothing to write: the environment is empty.
    return SUCCESS;
}

export function clock_time_get(clockId, precision, resultPointer) {
    // Clock ids: 0 realtime, 1 monotonic. Both are reported in nanoseconds.
    const milliseconds = clockId === 0 ? Date.now() : performance.now();
    view().setBigUint64(resultPointer, BigInt(Math.round(milliseconds * 1e6)), true);
    return SUCCESS;
}

export function random_get(pointer, length) {
    crypto.getRandomValues(bytes(pointer, length));
    return SUCCESS;
}

export function sched_yield() {
    // Yielding cannot be honoured on the browser's single thread, and the
    // caller only needs it to not fail.
    return SUCCESS;
}

export function proc_exit(code) {
    throw new Error(`Block exited with status ${code}`);
}

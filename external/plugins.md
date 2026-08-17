# Plugin proposal

Framework-neutral plugins rendered inside BE3, with native plugins isolated in
separate processes and web plugins isolated as separate WebAssembly modules.
High-level design only — no code.

## Goals

- Replace the current `wasm-demo` debug window and `wasm-demo` crate with a
  `plugin-demo` that works on macOS, Linux, Windows, Android, and web.
- A plugin owns its UI and may use any GUI framework. It does not link to BE3,
  share BE3's `egui` instance, or need the same Rust toolchain or dependency
  versions as the host.
- BE3 gives the plugin a viewport and sends it normalized pointer, wheel,
  keyboard, text, focus, resize, and scale-factor input. The plugin renders a
  frame into a negotiated platform-native image surface, which BE3 composites
  into the debug window.
- Image transport uses an optimized GPU/compositor mechanism on every
  platform. There is no CPU pixel-buffer fallback in v1. If the active host and
  plugin graphics backends have no compatible mechanism, the window reports
  that the configuration is unsupported.
- Native plugin crashes, hangs, and malformed messages do not corrupt or block
  the BE3 process. Process isolation is not treated as a security sandbox;
  permissions and sandbox policy are separate future work.

## Execution model

On macOS, Linux, and Windows, `plugin-demo` is a standalone executable shipped
beside or inside the application package. BE3 creates a private IPC endpoint,
launches the executable with only the information needed to connect, verifies
the peer, and performs a versioned handshake. All blocking process and IPC work
runs away from the UI thread.

On Android, the plugin runs in a separately named application process through
a packaged Android service. Binder provides connection establishment, peer
death notification, ordinary messages, and native handle transfer. The plugin
is bundled with the application in v1; downloading and executing new Android
plugin code is out of scope.

On web, the browser cannot launch a native subprocess. The separately built
and instantiated `plugin-demo` WebAssembly module remains the isolation
boundary. It follows the same conceptual lifecycle and input protocol, while
JavaScript adapts those messages to the module and its canvas.

Closing the window requests an orderly shutdown. Native plugins get a bounded
interval to exit before BE3 terminates and reaps them. Disconnects, crashes,
timeouts, and startup failures become visible plugin errors, and reopening the
window can start a clean process without retaining stale endpoints, handles,
or surfaces.

## IPC protocol

`block-plugin-api` defines shared Rust message types and a versioned binary wire
protocol. Both BE3 and native plugins depend on this crate, and its message
model derives `serde` serialization. Messages use a bounded binary encoding
inside length-prefixed frames. Protocol versions still change explicitly when
message representations or semantics change. The protocol covers:

- protocol negotiation, plugin identity, and supported capabilities;
- viewport creation, logical and pixel size, scale factor, and resizing;
- batched pointer, button, wheel, keyboard, text, modifier, and focus input;
- surface capability negotiation and native handle transfer;
- frame generation, damage, synchronization, and presentation completion;
- structured errors, liveness, graceful shutdown, and acknowledgement.

The protocol specifies message ordering, request identifiers, maximum frame and
queue sizes, collection limits, backpressure, timeout behavior, and how unknown
message kinds, unsupported protocol versions, truncated frames, and malformed
payloads are handled. A slow plugin can cause its own input or frames to be
coalesced or dropped, but cannot grow host memory without bound or make the UI
thread wait on IPC.

Native surface handles are never serialized as process-local integer values.
Shared Rust attachment types associate their metadata and ownership with a
protocol frame, while narrow platform adapters carry the actual resources:
ancillary data on Unix-domain sockets, duplicated handles associated with
named-pipe messages on Windows, and Binder file-descriptor or native-object
transfer on Android. Every surface capability defines who creates, duplicates,
owns, signals, waits on, and closes each handle, including behavior on resize,
disconnect, and device loss.

The initial desktop transports are Unix-domain sockets on macOS and Linux and
named pipes on Windows. The endpoint is private to the launched plugin, the
handshake authenticates the expected peer where the operating system permits
it, and accepting arbitrary network clients is never part of the protocol.

## Rendering and input

BE3 allocates the Plugin Demo viewport in `egui`, converts its logical size to
physical pixels, and sends input without exposing `egui` types. Pointer capture
continues outside the viewport after a press. Keyboard messages distinguish
physical keys from text input. Focus changes, modifier state, wheel units,
zero-sized viewports, and scale-factor changes are explicit.

The host and plugin advertise surface mechanisms supported by their active
graphics backends and choose the first compatible option. A surface descriptor
contains its mechanism, dimensions, color format and color space, alpha mode,
native handles, synchronization objects, frame generation, and damage bounds.
The plugin renders and signals readiness; BE3 waits through GPU-compatible
synchronization and samples or copies the surface in an `egui_wgpu` paint
callback. Neither side performs CPU readback or CPU texture upload.

The intended initial mechanisms are:

- **Web:** the existing hidden plugin canvas followed by
  `GPUQueue.copyExternalImageToTexture` into a BE3-owned wgpu texture. The two
  WebGPU devices do not share a wgpu resource, and pixels do not pass through
  Rust or WebAssembly memory.
- **macOS:** an IOSurface-backed Metal texture plus cross-process-compatible
  GPU synchronization.
- **Windows:** a shared DXGI/D3D texture plus duplicated synchronization
  handles.
- **Linux:** dma-buf plus explicit synchronization, imported through the
  compatible Vulkan or EGL backend.
- **Android:** AHardwareBuffer plus compatible GPU synchronization, with the
  handles transferred through Binder.

The exact descriptor for a mechanism is fixed only after confirming what the
project's `wgpu` version can import on that platform. Backend-specific and
unsafe handle import stays inside small platform modules. Dimensions, formats,
handle types, peer identity, ownership, and synchronization state are validated
before import. If import or synchronization is unavailable, BE3 shows a precise
unsupported-backend error instead of falling back to a CPU image.

The last safely presentable frame may remain visible while waiting for a new
one. A synchronization timeout, invalid descriptor, device loss, or plugin exit
stops further presentation and changes the window to an error state. Resize
creates a newly negotiated surface and retires the old one only after neither
process can still use it.

## Packaging

`scripts/build-plugin-demo.sh` builds the native plugin executable for an
explicit target and profile and stages it where the corresponding application
package expects it. BE3 launches a resolved absolute path rather than searching
the working directory or `PATH`.

- macOS places the executable inside the application bundle and signs it with
  the rest of the bundle.
- Windows packages the executable and its dependencies in a dedicated
  directory and restricts its DLL search path to that directory.
- Linux packages the executable in a deterministic application-relative
  directory.
- Android declares and packages the separate-process service in the APK.
- `scripts/build-block-web.sh` builds and stages the renamed WebAssembly plugin
  and its JavaScript bindings alongside the host module.

The Plugin Demo reports missing artifacts, launch failures, handshake errors,
unsupported graphics backends, plugin exits, and protocol violations
separately. Diagnostics include the plugin version, process or module state,
selected transport, and negotiated surface mechanism.

## Suggested units of work

Roughly in dependency order; each should be independently reviewable and
verifiable. Every lifecycle or platform unit includes its own failure-path
handling and cleanup rather than deferring correctness work to a final pass.

1. **Rename the existing demo** — rename `wasm-demo` and
   `debug/wasm_demo` to `plugin-demo` and `debug/plugin_demo`, including
   workspace entries, build scripts, generated assets, shader identifiers,
   menu labels, and user-facing text. Preserve current web-only behavior.
2. **Shared protocol model and framing** — add `block-plugin-api` with shared
   Rust types for handshake, lifecycle, input, capability, opaque surface,
   frame, error, and shutdown messages. Implement a bounded, length-prefixed
   binary encoding, frame and collection limits, explicit version-evolution
   rules, and malformed-message tests. Keep platform-native surface descriptors
   opaque until their corresponding surface paths define them.
3. **Session and lifecycle core** — implement the transport-independent plugin
   state machine, request tracking, timeouts, bounded queues, and event
   coalescing rules. Only pointer motion, wheel accumulation, redundant
   modifier state, and superseded resize events may be coalesced; button, key,
   text, focus, and lifecycle transitions preserve their ordering. Test it with
   an in-process protocol peer, including malformed payloads, timeout,
   disconnect, repeated start and shutdown, and queue saturation.
4. **Host viewport and input adapter** — translate `egui` viewport state and
   pointer, wheel, keyboard, text, modifier, focus, resize, and scale-factor
   input into protocol messages without exposing `egui` types. Implement
   pointer capture and zero-sized viewport behavior and test the non-GUI input
   normalization separately where practical.
5. **Host presenter integration** — define the platform surface-presenter
   interface and connect it to an `egui_wgpu` paint callback. Support retaining
   the last safely presentable frame, transitioning to precise unsupported or
   error states, and releasing presenter resources on close or replacement.
6. **Web protocol adapter** — make the existing hidden-canvas and
   `copyExternalImageToTexture` path the first complete protocol-driven vertical
   slice. Adapt module startup, lifecycle, full input, resize, errors, repeated
   open and close, and shutdown to the shared session core, then remove the old
   WebAssembly-specific host facade.
7. **Native demo executable** — build `plugin-demo` as a standalone desktop
   executable with the shared protocol client, lifecycle state machine, and a
   transport test mode. It must exit cleanly on shutdown or host disconnect and
   reject malformed or out-of-order host messages.
8. **Desktop process transport** — implement one desktop process lifecycle and
   IPC driver for macOS, Linux, and Windows using cross-platform process and
   local-socket APIs where practical. Narrow platform adapters provide private
   Unix-domain sockets or Windows named pipes, peer verification, forced
   termination, and process-tree reaping. Perform the handshake and implement
   disconnect handling and bounded shutdown once in the shared driver. A
   missing surface presenter may still produce an unsupported error. Cover
   startup failure, crash, hang, malformed payloads, and repeated open and
   close on every platform.
9. **Desktop handle transfer** — define one attachment model that associates
   native resources strictly with protocol frames and shares descriptor-count,
   type, ownership, disconnect, and malformed-message validation. Implement
   narrow Unix ancillary-descriptor and Windows duplicated-handle carriers,
   including close-on-exec, process identity, duplication rights, and cleanup.
10. **macOS surface path** — define the macOS surface descriptor, render the
    demo into an IOSurface-backed Metal texture, transfer its handles and
    synchronization over IPC, import it into BE3's active wgpu backend, and
    handle unsupported backends, resize, crash, timeout, shutdown, malformed
    descriptors, synchronization failure, and device loss.
11. **Windows surface path** — define the Windows surface descriptor and
    implement the equivalent shared DXGI/D3D texture, duplicated-handle,
    synchronization, import, resize, failure handling, and teardown flow.
12. **Linux surface path** — define the Linux surface descriptor and implement
    the equivalent dma-buf, explicit-sync, Vulkan/EGL import, resize, failure
    handling, and teardown flow, returning an unsupported error for incompatible
    active backends.
13. **Desktop build and packaging** — add `scripts/build-plugin-demo.sh`, stage
    each executable and runtime dependency in its application layout, integrate
    macOS signing and Windows DLL lookup constraints, and document the developer
    build commands and platform-specific failure diagnostics.
14. **Android service and Binder transport** — add the separately named service
    process, shared protocol adapter, Binder connection and peer lifecycle,
    death handling, bounded queues, orderly shutdown, and recovery after service
    death or activity recreation. Native graphics handles remain unsupported in
    this unit.
15. **Android handle transfer** — define how Binder carries file descriptors or
    native objects associated with protocol frames, including validation,
    ownership, duplication, disconnect, and malformed-message cleanup.
16. **Android surface path** — define the Android surface descriptor, render to
    AHardwareBuffer, transfer and import the buffer and synchronization handles,
    and handle unsupported backends, resize, crash, timeout, shutdown,
    synchronization failure, activity recreation, and device loss.
17. **Android packaging** — declare and package the separate-process service in
    the APK, stage the bundled plugin code, and report missing or invalid
    artifacts distinctly.
18. **Cross-platform diagnostics and documentation** — display detailed process
    or module state, selected transport, protocol version, queue state, and
    negotiated capabilities. Consolidate the platform support matrix and manual
    test matrix after each adapter already owns its lifecycle and cleanup tests.

Units 1–6 establish and validate the shared protocol through the web vertical
slice. Units 7–9 establish the native process boundary and resource-transfer
model with shared desktop orchestration and narrow platform adapters. Units
10–13 make and package desktop rendering one platform at a time because native
GPU import and synchronization cannot be usefully abstracted by cross-platform
APIs. Units 14–17 add Android in separate transport, handle-transfer, rendering,
and packaging stages. Unit 18 consolidates diagnostics and documentation
without postponing lifecycle correctness from earlier units.

## Out of scope

V1 does not include plugin discovery, installation, automatic updates, hot
reload, downloaded Android executables, permissions, sandbox policy, non-Rust
native plugins, CPU image fallbacks, audio, accessibility-tree integration,
clipboard integration, drag and drop, input methods beyond the defined text
protocol, or a general plugin
SDK for third-party distribution. The process boundary contains memory faults
but does not by itself restrict filesystem, network, GPU, or child-process
access. Those capabilities need a separate security and product design.

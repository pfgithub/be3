# Plugin deduplication

The web and native plugin transports should remain separate, but the app-side host duplicated substantial platform-neutral orchestration.

## Done

The app-side host is now shared. `crates/block-app/src/plugin_host/runtime.rs` owns the host, every runtime's state and all of `editor_ui`, `creation`, `artifact`, `preview`, instance queries, pass and screen synchronisation, slot allocation and eviction, and error and restart handling. Each platform implements only the `Backend` trait in it: `native.rs` (child process), `web.rs` (browser adapter and repaint scheduling) and `unavailable.rs` (no plugins at all). Presentation stays in `linux.rs`, `windows.rs` and `web/renderer.rs` behind `SurfacePresenter`.

## Remaining sharing opportunities

### 1. Shared protocol session naming

The protocol lifecycle state machine in `crates/block-editor-plugin/src/native.rs` is already used by the web runtime in `crates/block-editor-plugin/src/web.rs`. It is not native-specific. Moving it to a shared module such as `session.rs` would make the intended architecture clearer and prevent future platform-specific behavior from leaking into it.

### 2. Linux and Windows plugin runners

The Linux and Windows endpoint loops in `crates/block-editor-plugin/src/runner.rs` are closely duplicated. The following behavior is common:

- Handshake
- Message batching
- Screen processing
- Layout generations
- Surface resizing
- Rendering
- Outbound flushing
- Repaint timing

Socket readiness, attachment carriers, surface types, and platform cleanup differ. This is the largest remaining sharing opportunity.

### 3. Web and Android discovery

`crates/block-app/src/editors/plugin/discovery/web.rs` and `crates/block-app/src/editors/plugin/discovery/android.rs` duplicate parsing `plugins/index.json`, constructing manifest paths, recording errors, and installing results. Their file-fetch mechanisms differ. A shared indexed-manifest loader is possible, though the asynchronous web and synchronous Android readers limit the payoff.

## Code that should remain separate

- Native child-process management and IPC
- Browser wasm loading and canvas lifecycle
- Web repaint scheduling
- DMA-BUF, DXGI, and external-image presentation

## Recommended order

1. Rename and extract the shared client session.
2. Consolidate the Linux and Windows runner loops.

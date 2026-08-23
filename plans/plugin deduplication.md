# Plugin deduplication

The web and native plugin transports should remain separate, but the app-side host currently duplicates substantial platform-neutral orchestration.

## Sharing opportunities

### 1. Common host and runtime orchestration

`crates/block-app/src/plugin_host/native.rs` and `crates/block-app/src/plugin_host/web.rs` independently implement nearly identical logic for:

- `editor_ui`
- `creation`
- `artifact`
- `preview`
- Instance input, drag, cursor, and open-request handling
- Artifact drafts and outcomes
- Creation readiness and completion
- View changes, aspect ratio, and intrinsic size
- Closing instances and runtime status reporting

The platform-specific portion is primarily runtime startup, message transport, frame presentation, error recovery, and repaint scheduling. A shared host core operating through a small web/native runtime interface would remove the largest duplication while retaining distinct implementations.

### 2. Screen and pass synchronization

Both host implementations independently perform:

- Pass deduplication
- `Instances::next_screens`
- Screen-set change detection
- Pending message collection
- Client-response pumping

The transport send and poll portions differ, but construction of the outbound batch can be shared.

### 3. Runtime slot allocation and eviction

The `surface_for` logic is effectively duplicated between the native and web hosts. Selecting an existing surface, finding a free one, and evicting the least-recently-used idle runtime are platform-neutral. Only shutdown and release are platform-specific.

### 4. Shared runtime state

Both runtime structures carry the same conceptual fields:

- Surface number
- Presenter status
- `Instances`
- `ScreenLayout`
- Last sent screens
- Pass number

These could form a shared core embedded inside separate native and web runtime wrappers.

### 5. Shared protocol session naming

The protocol lifecycle state machine in `crates/block-editor-plugin/src/native.rs` is already used by the web runtime in `crates/block-editor-plugin/src/web.rs`. It is not native-specific. Moving it to a shared module such as `session.rs` would make the intended architecture clearer and prevent future platform-specific behavior from leaking into it.

### 6. Linux and Windows plugin runners

The Linux and Windows endpoint loops in `crates/block-editor-plugin/src/runner.rs` are closely duplicated. The following behavior is common:

- Handshake
- Message batching
- Screen processing
- Layout generations
- Surface resizing
- Rendering
- Outbound flushing
- Repaint timing

Socket readiness, attachment carriers, surface types, and platform cleanup differ. This is likely the next-largest sharing opportunity after the app-side host.

### 7. Web and Android discovery

`crates/block-app/src/editors/plugin/discovery/web.rs` and `crates/block-app/src/editors/plugin/discovery/android.rs` duplicate parsing `plugins/index.json`, constructing manifest paths, recording errors, and installing results. Their file-fetch mechanisms differ. A shared indexed-manifest loader is possible, though the asynchronous web and synchronous Android readers limit the payoff.

## Code that should remain separate

- Native child-process management and IPC
- Browser wasm loading and canvas lifecycle
- Web repaint scheduling
- DMA-BUF, DXGI, and external-image presentation
- Native and browser error and retry behavior where lifecycle semantics differ

## Recommended order

1. Rename and extract the shared client session.
2. Extract common host query and mutation functions and outbound screen-batch construction.
3. Extract runtime slot allocation.
4. Introduce a shared host core with separate web and native backend wrappers.
5. Independently consolidate the Linux and Windows runner loops.

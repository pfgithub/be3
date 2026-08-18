# Plugin-based block editors

## Proposal

### Goals

- Make the plugin boundary the only supported way to implement a block editor. `block-app` should be an editor host, while every first-party editor is built, packaged, discovered, and run through the same public API available to third-party editors.
- Let editor code keep using the normal typed `BlockClient` and `BlockHandle` APIs. A child-process client uses a delegated IPC backend connected to the parent process's existing client instead of opening another server connection.
- Preserve the behavior of the current `BlockEditor` surface: creation, tabs, embedded live editors, passive previews, top and side regions, intrinsic sizing, pan and zoom, access control, child blocks, navigation actions, presence, and dynamic artifacts.
- Isolate editor failures. A malformed message, panic, timeout, or crash should close only the affected editor instance and show a recoverable error in its place.
- Support desktop, web, and Android before the in-process editor path is removed. Every supported target must use its optimized GPU texture-sharing mechanism; pixel buffers and software presentation are not part of the architecture.
- Turn `crates/editors/counter` into a small reference editor and conformance fixture built on the production SDK, not a parallel debug-only implementation.

### Architectural boundary

The parent owns the server websocket and its existing authoritative `BlockClient`. Each plugin process constructs a delegated `BlockClient` whose backend sends client requests over the plugin IPC connection. The parent bridge executes those requests through the existing client and streams block updates, operation acknowledgements, reference-list changes, presence, properties, and history state back to the child. From editor code, `get_block::<B>`, `BlockHandle::read`, `operate`, undo/redo, creation, relationships, references, and presence continue to work as they do in process.

This first implementation does not sandbox plugins or assign per-plugin capability scopes. A child executable is already arbitrary code running as the user, and its delegated client may use the same workspace operations available to the parent client. Normal account, workspace, and block permissions still apply at the existing client/server boundary, but the parent bridge is not a new security boundary. Sandboxing, permission prompts, and restricted delegated clients are explicitly future work and must not complicate the MVP protocol.

Refactor `block-client` so its public handle and subscription behavior can run over either the current direct backend or a delegated backend. Keep encryption keys, websocket ownership, and operation sequencing in the parent's direct client; do not create a transparent socket tunnel or a second server session. The delegated wire protocol represents the semantic requests and events needed by `BlockClient`, allowing the child to reuse typed blocks without knowing server transport or encryption details.

Move the type-erased UI parts of `BlockEditor`, `EditorKind`, `CreationOptions`, and `DynamicArtifactSupport` out of `block-app` and express them as a versioned contract in `block-plugin-api`. The UI wire representation should use UUIDs, bounded byte payloads, and plain geometry rather than Rust trait objects or `egui` types. The host retains all application chrome and translates plugin results into existing app actions.

Each plugin package has a manifest containing a stable plugin ID and version, the supported block type UUIDs, display names and Material icon names, creation mode, editor capabilities, required protocol capabilities, and target-specific entry points. One package may register multiple block types, but registration conflicts are errors and first-party packages are selected explicitly rather than by load order. The workspace build produces a catalog consumed by native, web, and Android packaging; runtime directory scanning is an additional desktop source, not the source of truth for built-in editors.

### Protocol changes

Replace the demo-specific lifecycle with an instance-oriented protocol. After the existing version and capability handshake, the host can create and destroy many editor instances in one plugin runtime. Every instance is keyed by an opaque instance ID and has a role such as direct editor, preview, creation options, or dynamic-artifact settings. Messages for an unknown or closed instance are rejected.

Keep two logical message families on the framed plugin connection. The editor protocol controls plugin instances, render regions, input, and app-level actions. The delegated-client protocol backs `BlockClient` and is independent of editor UI lifecycle, so the same delegated client can serve multiple editor instances in one plugin runtime.

Add bounded editor messages for:

- editor discovery and registration metadata, checked against the package manifest;
- opening an instance with its block ID, block type, editor role, and a handle to the runtime's delegated client;
- requesting that another block be opened, embedded, or previewed by the host;
- creation requests and results, child-block actions, navigation actions, intrinsic-size changes, viewport commands, and dynamic-artifact generation results;
- named render regions for main content, top bar, left sidebar, right sidebar, preview, creation options, and artifact settings.

The delegated-client protocol mirrors the internal semantic boundary between `BlockClient` handles and its worker: fetch and watch blocks, create blocks, submit typed serialized operations, manage history, read and write properties, watch reference lists, update relationships and dynamic artifacts, and exchange presence. It also carries request completion, errors, confirmed block updates, and operation acknowledgements in the same order observed by the parent client. Reuse the existing block codecs and optimistic-operation machinery on the child side rather than defining a second editor-specific state/revision system.

The UI protocol must expose capabilities rather than assume every editor implements every feature. Unsupported optional regions and actions retain the defaults currently provided by `BlockEditor`. Protocol limits should be defined per payload class: delegated block data, media-backed blocks, operation batches, presence, and surface descriptors cannot all share the demo's current one-megabyte frame limit.

### Host runtime

Promote the reusable code under `block-app/src/debug/counter` into a platform-neutral plugin host. Separate transport, session state, process/WASM lifecycle, input translation, surface presentation, and editor adaptation. `PluginEditor` implements the existing host-side `BlockEditor` during migration and maps its methods to plugin instances and named render regions. This adapter lets plugin and in-process editors coexist until the final cutover.

The host owns a plugin manager shared by all tabs and embedded editors. It discovers packages, validates manifests, starts at most one runtime per package when practical, routes instance messages, restarts crashed first-party runtimes, and shuts runtimes down when their last instance closes. Requests must be asynchronous: no editor draw call may block on process I/O. Each region displays its most recent frame, queues coalescible input and resize events, and requests repaint when a new frame or host event arrives.

The delegated client exposes the parent client's normal workspace behavior without an additional plugin permission layer. The host still owns editor composition concerns such as cycle prevention, tab navigation, native surface handles, and telling an editor when its UI should be read-only. Treat child processes as trusted first-party executables for this phase. A later sandboxing proposal can add restricted client backends and explicit grants without changing editor code that already targets the `BlockClient` API.

Replace the fixed `counter` path and global debug window with package lookup and ordinary `EditorRegistry` registrations. Registry entries become data from validated plugin manifests plus host callbacks for opening and creating `PluginEditor` instances. Unsupported, missing, incompatible, and crashed plugins use distinct user-facing states so installation errors are not confused with block permissions or loading.

### Rendering and platform support

Require external GPU surfaces on every target. Windows uses DXGI shared textures, Linux uses DMA-BUF, macOS uses IOSurface, Android uses `AHardwareBuffer`, and web uses `WebExternalImage` backed by the plugin's off-screen canvas. The handshake rejects a plugin when the host and guest cannot negotiate the platform's required surface mechanism. There is no software or pixel-buffer presentation path.

Named regions are independent viewports with their own size, scale factor, input focus, and frame generation. This preserves host-owned panel layout and allows previews and creation dialogs to exist without opening a full tab. Regions that do not need continuous animation render on state, input, theme, access, or size changes. The host sends theme and font-scale data so first-party plugins remain visually consistent, while the protocol does not expose internal `egui` objects.

Linux DMA-BUF, macOS IOSurface, and Android hardware-buffer import/export are not currently completed by the demo, so they are explicit platform-enablement milestones. Surface loss recreates the optimized surface and its region without losing the editor instance; repeated recreation failure puts that region into an error state rather than changing presentation mechanisms.

### SDK and editor layout

Split the current demo into three responsibilities:

- `block-plugin-api`: serialization-only protocol types, validation, host and guest session state machines, and attachment descriptors, with no UI framework dependency;
- a new `block-editor-plugin` guest SDK: instance routing, delegated-client setup, host UI requests, `egui` input adaptation, region rendering, and target entry points;
- `counter`: a minimal reference package that declares one test block editor and exercises every stable capability needed by conformance tests.

First-party editor packages depend on the guest SDK and `block-client`, but not on `block-app`. They open their typed handles from the delegated client and retain the same read/operate/subscription patterns used today. The delegated build of `block-client` must exclude direct websocket, account-management, and platform transport dependencies that a plugin does not use, keeping plugin artifacts smaller without creating a second client API. Editor-only reusable cores such as `logicgame`, canvas geometry, database layouts, and renderers move with or below their plugin packages rather than remaining reachable through `block-app`.

Split the current text editor into two reusable layers before migrating it. `text-editor-core` remains the UI-independent editing engine for diffing, syntax highlighting, and cursor data. A new `text-editor-view` package owns the editor/view behavior currently in `block-app`, including `egui` interaction, layout, font handling, embeds, selection, cursor presentation, and the adapter that drives `text-editor-core`. The text plugin depends on `text-editor-view` and supplies its delegated `BlockHandle<Text>` and presence access; neither reusable text package depends on `block-app` or plugin transport.

The guest SDK supplies the delegated `BlockClient`, local UI state, named-region callbacks, and declarative editor capabilities. Block state, operations, references, history, and presence use the real `block-client` API rather than SDK-specific imitations. Provide a package template, manifest validation command, build command, and a revised editor guide based on `counter`.

### Migration and completion criteria

Migrate editors in increasing order of dependency complexity. Begin with read-only and simple single-block editors, continue with ordinary editable blocks, then move media and configurable creation, nested/reference-heavy editors, GPU editors, and finally text, logic grid, database views, and dynamic-artifact producers. During migration, `EditorRegistry` may contain both native and plugin registrations, but a block type has exactly one selected implementation.

An editor is considered migrated only when its creation flow, direct tab, embedded mode, preview, access changes, undo/redo, presence, references, and failure recovery match the capabilities it had in process. Deterministic non-GUI editor logic keeps ordinary Rust tests in its model or core crate. Protocol, SDK, lifecycle, and host adapter behavior receive automated tests; visual and platform-surface behavior receives a documented manual test matrix.

The migration is complete when `block-app/src/editors` contains only the plugin adapter, host-owned editor layout and common chrome, unsupported/error UI, registry/catalog integration, and truly app-wide helpers. There are no first-party block-type editor implementations linked into `block-app`; deleting or disabling a plugin package makes that editor unavailable without recompiling the host; and the same package artifacts and protocol are used by the reference demo and production editors.

## Suggested units of work

The first milestone is deliberately a narrow vertical slice: a real synchronized counter block opened through a delegated `BlockClient` and rendered by a plugin in an ordinary tab. It uses the existing WebExternalImage and Windows DXGI work, supports one plugin instance and one main region, and omits creation UI, previews, presence, references, and third-party discovery. That proves the delegated client and texture path before generalizing either one.

1. **Add the counter block.** Add counter state plus increment/decrement operations under `block-client::blocks`, with separate serialization, operation, and history tests.
2. **Separate `BlockClient` from its direct transport.** Introduce an internal backend request/event boundary while preserving the existing public `BlockClient` and `BlockHandle` APIs and direct-client behavior.
3. **Define the delegated-client protocol.** Add bounded request, completion, error, block-update, and operation-acknowledgement messages that serialize the backend boundary over the existing framed IPC transport.
4. **Implement the parent client bridge.** Receive delegated requests, execute them through the parent's existing `BlockClient`, and stream resulting events back in order without opening another server connection.
5. **Implement the child delegated backend.** Construct a `BlockClient` over IPC, populate typed handles from parent events, and preserve the existing optimistic operation and history behavior.
6. **Define the minimum editor manifest.** Add bounded manifest types for one block type, display metadata, entry points, and a required surface mechanism; give `counter` a valid manifest.
7. **Define the minimum UI protocol.** Add instance IDs plus open, resize, input, close, and error messages for one main render region, with codec-limit and ordering tests.
8. **Extract the minimum host runtime.** Move handshake, process/WASM lifecycle, IPC multiplexing, input queues, and existing surface presentation out of the debug window.
9. **Extract the minimum guest runtime.** Move handshake, delegated-client setup, single-instance routing, `egui` input, and frame production into `block-editor-plugin`.
10. **Add the counter plugin editor.** Replace the local demo state with a typed `BlockHandle<Counter>` and increment/decrement controls driven through `operate`.
11. **Open the counter through `EditorRegistry`.** Add a minimal `PluginEditor`, register the counter manifest, and open it as a normal tab through the production host. This commit is the MVP.
12. **Support multiple instances per runtime.** Route instance-scoped UI messages while sharing one delegated client, stop a runtime after its last instance, and reject stale instance IDs.
13. **Complete delegated block watching.** Support concurrent typed handles, initial fetches, watch lifetimes, confirmed updates, disconnection, and resubscription after runtime restart.
14. **Complete delegated operations and history.** Cover pending operation ordering, acknowledgements, remote operations, history grouping, undo, redo, and authoritative replacement using existing client semantics.
15. **Delegate block creation and properties.** Support typed creation, duplication, replacement, naming, parent changes, access reads, and dynamic-artifact descriptors through the child client.
16. **Delegate references and presence.** Support reference-list watches, cached reference metadata, relationship helpers, presence reads/writes, and cleanup when the delegated client disconnects.
17. **Add delegated-client conformance tests.** Run the existing client behavior suite against direct and in-memory delegated backends and add transport interruption and reconnection cases.
18. **Add package failure recovery.** Distinguish missing, incompatible, crashed, timed-out, and surface-failed packages; restart first-party runtimes and rebuild delegated handles from parent state.
19. **Generalize Windows texture presentation.** Move DXGI sharing behind the region presenter, support surface recreation, and put repeated failures into a region error state.
20. **Generalize web texture presentation.** Replace the fixed demo canvas with per-package, per-region external images and deterministic GPU and DOM resource teardown.
21. **Implement Linux texture presentation.** Add DMA-BUF export, attachment transfer, import, synchronization, surface recreation, and lifecycle tests.
22. **Implement macOS texture presentation.** Add IOSurface export/import, synchronization, signing-safe attachment handling, surface recreation, and lifecycle tests.
23. **Implement Android texture presentation.** Add the Android service transport plus `AHardwareBuffer` transfer, import, synchronization, surface recreation, and lifecycle tests.
24. **Make optimized surfaces mandatory.** Require the platform surface capability during negotiation and test that a handshake with no common mechanism fails explicitly.
25. **Add named editor regions.** Generalize instances to main, top bar, left sidebar, right sidebar, preview, creation-options, and artifact-settings regions with independent input and frame state.
26. **Complete the `PluginEditor` adapter.** Map every current `BlockEditor` region, intrinsic-size query, pan/zoom capability, viewport command, and navigation result onto UI messages.
27. **Add editor creation flows.** Extend manifests and the UI protocol for immediate and configured creation while editor code creates blocks through its delegated client.
28. **Add nested editors and previews.** Let plugins request host-rendered referenced previews or live regions while the host retains composition and cycle prevention.
29. **Add child and artifact UI actions.** Route child placement plus dynamic-artifact settings and regeneration UI between plugin regions and host chrome; perform block changes through delegated handles.
30. **Complete the guest authoring API.** Expose delegated-client access, declarative regions, theme/font-scale data, host UI actions, and a headless conformance harness.
31. **Generate the first-party catalog.** Validate manifests during workspace builds, generate deterministic registrations, reject duplicate block types, and load catalog registrations beside native editors.
32. **Add external discovery and packaging.** Discover desktop packages, resolve explicit overrides, and teach native, web, and Android builds to stage entry points, manifests, and assets.
33. **Turn `counter` into the conformance package.** Keep the counter as its reference editor and exercise creation, preview, history, presence, references, multiple instances, and intentional failures.
34. **Migrate compiled logic and calendar.** Package their existing typed-handle editors and preview regions, register their manifests, and remove their in-process modules.
35. **Migrate pixel art and workspace index.** Package painting, image generation, listings, and child navigation, then remove their native registrations.
36. **Migrate presentation.** Package slide creation, reference watching, embedded previews, presenter playback, and child actions, then remove its native editor.
37. **Add host platform integrations.** Add UI requests for file pickers, clipboard images, media playback, and native webviews where those cannot live naturally inside a child-rendered texture.
38. **Migrate image, audio, and browser tab.** Use delegated handles plus the host platform integrations for imports, playback, and native webviews.
39. **Migrate video.** Package its timeline, player, and effects using delegated handles and host media playback while preserving frame-accurate operations.
40. **Add map and GPU integrations.** Define tile-cache coordination and editor GPU-resource initialization needed alongside the platform texture-sharing path.
41. **Migrate map and pixel ray tracer.** Package their rendering cores and UI using delegated handles and the new integrations, including previews and intrinsic sizing.
42. **Migrate scene 3D.** Package its renderer, shaders, camera, and scene UI, then remove its block-app render-resource installation.
43. **Migrate settings, UI settings, and hotbar.** Package fallback and per-client settings resolution plus the shared nested hotbar, then remove their native editors.
44. **Migrate logic game and version control.** Package challenge, quiz, version-control data, and worktree UI using the delegated client and ordinary child-process OS access.
45. **Migrate infinite canvas.** Package its core, interaction, painting, inspector, geometry, nested live editors, previews, and clipboard integration.
46. **Migrate GUI builder.** Package its surface, inspector, nested block picker, and dynamic Rust artifact settings and regeneration.
47. **Extract database editor cores.** Move schema, spreadsheet, kanban, and scatter logic below a plugin package without depending on block-app.
48. **Migrate database editors.** Package database, database schema, and database view together, preserving typed handles, references, sorting, and embedded behavior.
49. **Create `text-editor-view`.** Move `egui` interaction, layout, fonts, embeds, selection, and cursor presentation out of block-app into a package layered on `text-editor-core`.
50. **Migrate text.** Build the text plugin around `text-editor-view` and a delegated `BlockHandle<Text>`, including cursor presence, embeds, and intrinsic sizing.
51. **Extract logic-grid editor cores.** Move canvas geometry, simulation, challenges, hotbar logic, renderer, shaders, and compiled-artifact generation below a plugin package.
52. **Migrate logic grid.** Package direct editing, GPU rendering, presence, challenge playback, nested components, and compiled-logic artifacts using delegated handles.
53. **Remove the in-process editor path.** Delete remaining block-type registrations and modules, reduce block-app to host chrome and the plugin adapter, and remove debug demo paths.
54. **Ship the authoring workflow.** Replace the editor guides, add package scaffold and build commands, document the trusted-executable model, and add CI plus the platform matrix.
